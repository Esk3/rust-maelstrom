#![allow(dead_code)]

use std::collections::HashMap;

use crate::message::Message;

#[derive(Debug)]
pub enum Event<I, T>
where
    Message<I>: IntoMessageId,
    Message<T>: IntoMessageId,
{
    MessageRecived(Message<I>),
    MessageSent(Message<T>),
    // maybe wrap in message, or seperate ErrorMessage variant
    Error(crate::error::Error),
}

enum MessageType<I> {
    NodeMessage(I),
    BuiltinMessage(BuiltInMessage),
}
enum BuiltInMessage {}

enum SubscribeOption<I, T>
where
    Message<I>: IntoMessageId,
    Message<T>: IntoMessageId,
{
    None(tokio::sync::mpsc::UnboundedSender<Event<I, T>>),
    MessageRecived(tokio::sync::mpsc::UnboundedSender<I>),
    MessageSent(tokio::sync::mpsc::UnboundedSender<T>),
    Error(tokio::sync::mpsc::UnboundedSender<crate::error::Error>),
}

pub struct EventHandler<I, T>
where
    Message<I>: IntoMessageId,
    Message<T>: IntoMessageId,
{
    new_subscriber_tx: tokio::sync::mpsc::UnboundedSender<(MessageId, SubscribeOption<I, T>)>,
    new_event_tx: tokio::sync::mpsc::UnboundedSender<Event<I, T>>,
}

impl<I, T> EventHandler<I, T>
where
    Message<I>: IntoMessageId,
    Message<T>: IntoMessageId,
    T: Send + 'static,
    I: Send + 'static,
{
    pub fn new() -> Self {
        let (new_subscriber_tx, new_subscriber_rx) = tokio::sync::mpsc::unbounded_channel();
        let (new_event_tx, new_event_rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = EventWorker::new(new_subscriber_rx, new_event_rx);
        tokio::spawn(async move { worker.run().await });
        Self {
            new_subscriber_tx,
            new_event_tx,
        }
    }
    pub fn publish_event(&self, event: Event<I, T>) {
        self.new_event_tx.send(event).unwrap();
    }
    pub fn subscribe(
        &self,
        message_id: &impl IntoMessageId,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Event<I, T>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx
            .send((message_id.into_message_id(), SubscribeOption::None(tx)));
        rx
    }
    pub fn subscribe_once(&self) -> Event<I, T> {
        todo!()
    }
    pub fn subscribe_message_recived(
        &self,
        message_id: &impl IntoMessageId,
    ) -> tokio::sync::mpsc::UnboundedReceiver<I> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx.send((
            message_id.into_message_id(),
            SubscribeOption::MessageRecived(tx),
        ));
        rx
    }
    pub fn subscribe_message_sent(
        &self,
        message_id: &impl IntoMessageId,
    ) -> tokio::sync::mpsc::UnboundedReceiver<T> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx.send((
            message_id.into_message_id(),
            SubscribeOption::MessageSent(tx),
        ));
        rx
    }
    pub fn subscribe_error(
        &self,
        message_id: &impl IntoMessageId,
    ) -> tokio::sync::mpsc::UnboundedReceiver<crate::error::Error> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx
            .send((message_id.into_message_id(), SubscribeOption::Error(tx)));
        rx
    }
}
struct EventWorker<I, T>
where
    Message<I>: IntoMessageId,
    Message<T>: IntoMessageId,
{
    new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(MessageId, SubscribeOption<I, T>)>,
    new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>,
    subscriptions: HashMap<MessageId, SubscribeOption<I, T>>,
}

impl<I, T> EventWorker<I, T>
where
    Message<I>: IntoMessageId,
    Message<T>: IntoMessageId,
{
    fn new(
        new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(MessageId, SubscribeOption<I, T>)>,
        new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>,
    ) -> Self {
        Self {
            new_subscriber_rx,
            new_event_rx,
            subscriptions: HashMap::new(),
        }
    }
    async fn run(mut self) {
        loop {
            tokio::select! {
                Some(new_subscriber) = self.new_subscriber_rx.recv() => {
                    let (message_id, subscriber_option) = new_subscriber;
                    self.new_subscriber(message_id, subscriber_option);
                }
                Some(new_event) = self.new_event_rx.recv() => {
                    self.new_event(new_event);
                }
                else => panic!("am i closed now?")

            }
        }
    }
    fn new_subscriber(&mut self, message_id: MessageId, subscriber_option: SubscribeOption<I, T>) {
        self.subscriptions.insert(message_id, subscriber_option);
    }
    fn new_event(&mut self, event: Event<I, T>) {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.into_message_id(),
            Event::MessageSent(ref msg) => msg.into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.subscriptions.remove(&message_id) {
            match (sub, event) {
                (
                    SubscribeOption::MessageRecived(tx),
                    Event::MessageRecived(Message { body, .. }),
                ) => {
                    tx.send(body).unwrap();
                }
                (SubscribeOption::MessageRecived(_), _) => todo!(),
                (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body, .. })) => {
                    tx.send(body).unwrap();
                }
                (SubscribeOption::MessageSent(_), _) => todo!(),
                (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
                (SubscribeOption::Error(_), _) => todo!(),
                (SubscribeOption::None(tx), event) => {
                    tx.send(event).unwrap();
                }
            }
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct MessageId {
    src: String,
    dest: String,
    id: usize,
}

pub trait IntoMessageId: Send + Sync {
    fn into_message_id(&self) -> MessageId;
}

#[derive(Debug)]
enum MockMsgBody {
    DoSomething,
    DoNothing,
}

impl IntoMessageId for Message<MockMsgBody> {
    fn into_message_id(&self) -> MessageId {
        let id = match self.body {
            MockMsgBody::DoSomething => 1,
            MockMsgBody::DoNothing => 2,
        };
        MessageId {
            src: self.src.clone(),
            dest: self.dest.clone(),
            id,
        }
    }
}

#[tokio::test]
async fn test() {
    let handler = EventHandler::new();
    let message = Message {
        src: "testsrc".to_string(),
        dest: "testdest".to_string(),
        body: MockMsgBody::DoSomething,
    };
    let mut listner = handler.subscribe(&message);
    let event: Event<MockMsgBody, MockMsgBody> = Event::MessageRecived(message);
    handler.publish_event(event);
    dbg!(listner.recv().await);
    panic!();
}

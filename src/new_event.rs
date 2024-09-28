#![allow(dead_code)]

use std::{collections::HashMap, fmt::Debug, hash::Hash};

use crate::message::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<I, T, Id>
where
    Message<I>: IntoMessageId<Id>,
    Message<T>: IntoMessageId<Id>,
{
    MessageRecived(Message<I>),
    MessageSent(Message<T>),
    // maybe wrap in message, or add seperate ErrorMessage variant
    Error(crate::error::Error),
    A(Id),
}

enum MessageType<I> {
    NodeMessage(I),
    BuiltinMessage(BuiltInMessage),
}

enum BuiltInMessage {}

#[derive(Debug)]
enum SubscribeOption<I, T, Id>
where
    Message<I>: IntoMessageId<Id>,
    Message<T>: IntoMessageId<Id>,
{
    None(tokio::sync::mpsc::UnboundedSender<Event<I, T, Id>>),
    MessageRecived(tokio::sync::mpsc::UnboundedSender<I>),
    MessageSent(tokio::sync::mpsc::UnboundedSender<T>),
    Error(tokio::sync::mpsc::UnboundedSender<crate::error::Error>),
}

pub struct EventHandler<I, T, Id>
where
    Message<I>: IntoMessageId<Id>,
    Message<T>: IntoMessageId<Id>,
{
    new_subscriber_tx:
        tokio::sync::mpsc::UnboundedSender<(MessageId<Id>, SubscribeOption<I, T, Id>)>,
    new_event_tx: tokio::sync::mpsc::UnboundedSender<Event<I, T, Id>>,
}

impl<I, T, Id> EventHandler<I, T, Id>
where
    Message<I>: IntoMessageId<Id>,
    Message<T>: IntoMessageId<Id>,
    T: Send + 'static + Debug,
    I: Send + 'static + Debug,
    Id: Send + Sync + Debug + Clone + Eq + PartialEq + Hash + 'static,
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
    pub fn publish_event(&self, event: Event<I, T, Id>) {
        self.new_event_tx.send(event).unwrap();
    }
    pub fn subscribe(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Event<I, T, Id>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx.send((
            message_id.clone_into_message_id(),
            SubscribeOption::None(tx),
        ));
        rx
    }
    pub fn subscribe_once(&self) -> Event<I, T, Id> {
        todo!()
    }
    pub fn subscribe_message_recived(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<I> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx.send((
            message_id.clone_into_message_id(),
            SubscribeOption::MessageRecived(tx),
        ));
        rx
    }
    pub fn subscribe_message_sent(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<T> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx.send((
            message_id.clone_into_message_id(),
            SubscribeOption::MessageSent(tx),
        ));
        rx
    }
    pub fn subscribe_error(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<crate::error::Error> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx.send((
            message_id.clone_into_message_id(),
            SubscribeOption::Error(tx),
        ));
        rx
    }
}
struct EventWorker<I, T, Id>
where
    Message<I>: IntoMessageId<Id>,
    Message<T>: IntoMessageId<Id>,
{
    new_subscriber_rx:
        tokio::sync::mpsc::UnboundedReceiver<(MessageId<Id>, SubscribeOption<I, T, Id>)>,
    new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T, Id>>,
    subscriptions: HashMap<MessageId<Id>, SubscribeOption<I, T, Id>>,
}

impl<I, T, Id> EventWorker<I, T, Id>
where
    Message<I>: IntoMessageId<Id> + Debug,
    Message<T>: IntoMessageId<Id> + Debug,
    I: Debug,
    T: Debug,
    Id: Hash + PartialEq + Eq + Clone,
{
    fn new(
        new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(
            MessageId<Id>,
            SubscribeOption<I, T, Id>,
        )>,
        new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T, Id>>,
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
                else => {
                    dbg!("her");
                    panic!("am i closed now?")
                }

            }
        }
    }
    fn new_subscriber(
        &mut self,
        message_id: MessageId<Id>,
        subscriber_option: SubscribeOption<I, T, Id>,
    ) {
        self.subscriptions.insert(message_id, subscriber_option);
    }
    fn new_event(&mut self, event: Event<I, T, Id>) {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
            _ => todo!(),
        };
        if let Some(sub) = self.subscriptions.get(&message_id) {
            match (sub, event) {
                (
                    SubscribeOption::MessageRecived(tx),
                    Event::MessageRecived(Message { body, .. }),
                ) => {
                    tx.send(body).unwrap();
                    let _ = self.subscriptions.remove(&message_id);
                }
                (SubscribeOption::MessageRecived(_), _) => {
                    dbg!("here");
                }
                (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body, .. })) => {
                    tx.send(body).unwrap();
                    let _ = self.subscriptions.remove(&message_id);
                }
                (SubscribeOption::MessageSent(_), _) => todo!(),
                (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
                (SubscribeOption::Error(_), _) => todo!(),
                (SubscribeOption::None(tx), event) => {
                    tx.send(event).unwrap();
                    let _ = self.subscriptions.remove(&message_id);
                }
            }
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct BodyId<T>(T);

pub trait IntoBodyId<T>: Send + Sync + Debug + PartialEq + Eq + Hash
where
    T: Send + Sync + Debug + PartialEq + Eq + Hash,
{
    fn into_body_id(self) -> BodyId<T>;
    fn clone_into_body_id(&self) -> BodyId<T>;
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct MessageId<T> {
    src: String,
    dest: String,
    id: BodyId<T>,
}

pub trait IntoMessageId<T>: Send + Sync {
    fn into_message_id(self) -> MessageId<T>;
    fn clone_into_message_id(&self) -> MessageId<T>
    where
        T: Clone;
}

impl<T> IntoMessageId<T> for MessageId<T>
where
    T: Send + Sync + Debug,
{
    fn into_message_id(self) -> MessageId<T> {
        self
    }

    fn clone_into_message_id(&self) -> MessageId<T>
    where
        T: Clone,
    {
        self.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MockMsgBody {
    DoSomething,
    DoNothing,
}

impl IntoBodyId<usize> for MockMsgBody {
    fn into_body_id(self) -> BodyId<usize> {
        match self {
            MockMsgBody::DoSomething => BodyId(1),
            MockMsgBody::DoNothing => BodyId(2),
        }
    }

    fn clone_into_body_id(&self) -> BodyId<usize> {
        match self {
            MockMsgBody::DoSomething => BodyId(1),
            MockMsgBody::DoNothing => BodyId(2),
        }
    }
}

impl<T, ID> IntoMessageId<T> for Message<ID>
where
    ID: IntoBodyId<T>,
    T: Send + Sync + Debug + Hash + PartialEq + Eq,
{
    fn clone_into_message_id(&self) -> MessageId<T> {
        let id = self.body.clone_into_body_id();
        MessageId {
            src: self.src.clone(),
            dest: self.dest.clone(),
            id,
        }
    }

    fn into_message_id(self) -> MessageId<T> {
        let id = self.body.into_body_id();
        MessageId {
            src: self.src,
            dest: self.dest,
            id,
        }
    }
}

#[tokio::test]
async fn event_test() {
    let res = tokio::time::timeout(std::time::Duration::from_secs(1), async move {
        let handler = EventHandler::new();
        let message = Message {
            src: "testsrc".to_string(),
            dest: "testdest".to_string(),
            body: MockMsgBody::DoSomething,
        };
        let mut listner = handler.subscribe(&message);
        let event: Event<MockMsgBody, MockMsgBody, usize> = Event::MessageRecived(message.clone());
        handler.publish_event(event);
        let recv = dbg!(listner.recv().await).unwrap();
        assert_eq!(recv, Event::MessageRecived(message));
    })
    .await;
    dbg!(res.unwrap());
}

#[tokio::test]
async fn specific_event_test() {
    let res = tokio::time::timeout(std::time::Duration::from_secs(1), async move {
        let handler = EventHandler::new();
        let message = Message {
            src: "testsrc".to_string(),
            dest: "testdest".to_string(),
            body: MockMsgBody::DoSomething,
        };

        let mut listner = handler.subscribe_message_recived(&message);
        handler.publish_event(Event::MessageSent(message.clone()));

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let x = listner.try_recv();

        match x {
            Ok(_) => panic!(),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => panic!(),
            _ => (),
        };

        handler.publish_event(Event::MessageRecived(message.clone()));

        let y = listner.recv().await;

        assert!(matches!(y, Some(MockMsgBody::DoSomething)));
    })
    .await;
    dbg!(res).unwrap();
}

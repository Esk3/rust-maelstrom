#![allow(dead_code)]

use std::{collections::{HashMap, VecDeque}, fmt::Debug, hash::Hash, usize};

use anyhow::Context;

use crate::message::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<I, T> {
    MessageRecived(Message<I>),
    MessageSent(Message<T>),
    // maybe wrap in message, or add seperate ErrorMessage variant
    Error(crate::error::Error),
}

enum MessageType<I> {
    NodeMessage(I),
    BuiltinMessage(BuiltInMessage),
}

enum BuiltInMessage {}

#[derive(Debug)]
enum SubscribeOption<I, T> {
    None(tokio::sync::mpsc::UnboundedSender<Event<I, T>>),
    MessageRecived(tokio::sync::mpsc::UnboundedSender<I>),
    MessageSent(tokio::sync::mpsc::UnboundedSender<T>),
    Error(tokio::sync::mpsc::UnboundedSender<crate::error::Error>),
}

impl<I, T> SubscribeOption<I, T> {
    pub fn send(&self, event: Event<I, T>) -> anyhow::Result<()> {
        match (self, event) {
            (SubscribeOption::MessageRecived(tx), Event::MessageRecived(Message { body, .. })) => {
                tx.send(body);
                Ok(())
            }
            (SubscribeOption::MessageRecived(_), _) => {
                dbg!("here");
                Ok(())
            }
            (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body, .. })) => {
                tx.send(body);
                Ok(())
            }
            (SubscribeOption::MessageSent(_), _) => todo!(),
            (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
            (SubscribeOption::Error(_), _) => todo!(),
            (SubscribeOption::None(tx), event) => {
                tx.send(event);
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventHandler<I, T, Id> {
    new_subscriber_tx: tokio::sync::mpsc::UnboundedSender<(MessageId<Id>, SubscribeOption<I, T>)>,
    new_event_tx: tokio::sync::mpsc::UnboundedSender<Event<I, T>>,
}

impl<I, T, Id> EventHandler<I, T, Id>
where
    I: IntoBodyId<Id> + Send + 'static + Debug,
    T: IntoBodyId<Id> + Send + 'static + Debug,
    Id: Send + Sync + Debug + Clone + Eq + PartialEq + Hash + 'static,
{
    #[must_use]
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

    pub fn publish_event(&self, event: Event<I, T>) -> anyhow::Result<()> {
        self.new_event_tx
            .send(event)
            .context("publish event failed: event worker is closed")
    }

    pub fn subscribe(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx
            .send((
                message_id.clone_into_message_id(),
                SubscribeOption::None(tx),
            ))
            .context("subscribe failed: event worker closed")?;
        Ok(rx)
    }
    pub fn subscribe_once(&self) -> Event<I, T> {
        todo!()
    }
    pub fn subscribe_message_recived(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<I>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx
            .send((
                message_id.clone_into_message_id(),
                SubscribeOption::MessageRecived(tx),
            ))
            .context("subscribe to message recived failed: event worker closed")?;
        Ok(rx)
    }
    pub fn subscribe_message_sent(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<T>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx
            .send((
                message_id.clone_into_message_id(),
                SubscribeOption::MessageSent(tx),
            ))
            .context("subscribe to message sent failed: event worker closed")?;
        Ok(rx)
    }
    pub fn subscribe_error(
        &self,
        message_id: &impl IntoMessageId<Id>,
    ) -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<crate::error::Error>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.new_subscriber_tx
            .send((
                message_id.clone_into_message_id(),
                SubscribeOption::Error(tx),
            ))
            .context("subscribe to error failed: event worker closed")?;
        Ok(rx)
    }
}

impl<I, T, Id> Default for EventHandler<I, T, Id>
where
    I: IntoBodyId<Id> + Send + 'static + Debug,
    T: IntoBodyId<Id> + Send + 'static + Debug,
    Id: Send + Sync + Debug + Clone + Eq + PartialEq + Hash + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// TODO timeout for long events?
struct EventWorker<I, T, Id> {
    new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(MessageId<Id>, SubscribeOption<I, T>)>,
    new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>,
    queue: VecDeque<Event<I, T>>,
    static_subscriptions: HashMap<MessageId<Id>, SubscribeOption<I, T>>,
    dynamic_subscriptions: Vec<(MessageId<Id>, SubscribeOption<I, T>)>,
}

impl<I, T, Id> EventWorker<I, T, Id>
where
    I: IntoBodyId<Id>,
    T: IntoBodyId<Id>,
    Id: Hash + Eq + PartialEq + Debug + Send + Sync,
    I: Debug,
    T: Debug,
    Id: Hash + PartialEq + Eq + Clone,
{
    fn new(
        new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(
            MessageId<Id>,
            SubscribeOption<I, T>,
        )>,
        new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>,
    ) -> Self {
        Self {
            new_subscriber_rx,
            new_event_rx,
            queue: VecDeque::new(),
            static_subscriptions: HashMap::new(),
            dynamic_subscriptions: Vec::new(),
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
                    self.push_event(new_event);
                }
                else => {
                    panic!("am i closed now?")
                }

            }
            while let Some(event) = self.queue.pop_front() {
                if let Some(option) = self.find_consumable_event(&event) {
                    option.send(event).unwrap();
                } else {
                    self.queue.push_front(event);
                    break;
                }
            }
        }
    }

    fn new_subscriber(
        &mut self,
        message_id: MessageId<Id>,
        subscriber_option: SubscribeOption<I, T>,
    ) {
        if message_id.src.contains('*') || message_id.dest.contains('*') {
            self.dynamic_subscriptions
                .push((message_id, subscriber_option));
        } else {
            self.static_subscriptions
                .insert(message_id, subscriber_option);
        }
    }

    fn push_event(&mut self, event: Event<I, T>) {
        self.queue.push_back(event);
    }

    fn can_consume_event(&self, event: &Event<I, T>) -> bool {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.static_subscriptions.get(&message_id) {
            match (sub, event) {
                (
                    SubscribeOption::MessageRecived(tx),
                    Event::MessageRecived(Message { body: _, .. }),
                ) => !tx.is_closed(),
                (SubscribeOption::MessageRecived(_), _) => {
                    dbg!("here");
                    false
                }
                (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body: _, .. })) => {
                    !tx.is_closed()
                }
                (SubscribeOption::MessageSent(_), _) => todo!(),
                (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
                (SubscribeOption::Error(_), _) => todo!(),
                (SubscribeOption::None(tx), _event) => !tx.is_closed(),
            }
        } else {
            self.match_dynamic_subcribers(event).is_some()
        }
    }

    fn find_consumable_event(&self, event: &Event<I, T>) -> Option<&SubscribeOption<I, T>> {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.static_subscriptions.get(&message_id) {
            Some(sub)
        } else {
            self.match_dynamic_subcribers(event).map(|(id, sub)| sub)
        }
    }

    /// TODO always return first (0th) if available
    fn match_dynamic_subscribers_position(&self, event: &Event<I, T>) -> Option<usize> {
        if self.dynamic_subscriptions.is_empty() {
            None
        } else {
            Some(0)
        }
    }

    fn match_dynamic_subcribers(
        &self,
        event: &Event<I, T>,
    ) -> Option<&(MessageId<Id>, SubscribeOption<I, T>)> {
        let i = self.match_dynamic_subscribers_position(event)?;
        self.dynamic_subscriptions.get(i)
    }

    fn new_event(&mut self, event: Event<I, T>) {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.static_subscriptions.get(&message_id) {
            match (sub, event) {
                (
                    SubscribeOption::MessageRecived(tx),
                    Event::MessageRecived(Message { body, .. }),
                ) => {
                    // errors if event listner dropped
                    let _ = tx.send(body);
                    let _ = self.static_subscriptions.remove(&message_id);
                }
                (SubscribeOption::MessageRecived(_), _) => {
                    dbg!("here");
                }
                (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body, .. })) => {
                    // errors if event listner dropped
                    let _ = tx.send(body);
                    let _ = self.static_subscriptions.remove(&message_id);
                }
                (SubscribeOption::MessageSent(_), _) => todo!(),
                (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
                (SubscribeOption::Error(_), _) => todo!(),
                (SubscribeOption::None(tx), event) => {
                    // errors if event listner dropped
                    let _ = tx.send(event);
                    let _ = self.static_subscriptions.remove(&message_id);
                }
            }
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct BodyId<T>(pub T);

impl From<usize> for BodyId<usize> {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<&usize> for BodyId<usize> {
    fn from(value: &usize) -> Self {
        Self(*value)
    }
}

pub trait IntoBodyId<T>: Send + Sync + Debug + PartialEq + Eq
where
    T: Send + Sync + Debug + PartialEq + Eq + Hash,
{
    fn into_body_id(self) -> BodyId<T>;
    fn clone_into_body_id(&self) -> BodyId<T>;
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct MessageId<T> {
    pub src: String,
    pub dest: String,
    pub id: BodyId<T>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn event_test() {
        let res = tokio::time::timeout(std::time::Duration::from_millis(200), async move {
            let handler = EventHandler::new();
            let message = Message {
                src: "testsrc".to_string(),
                dest: "testdest".to_string(),
                body: MockMsgBody::DoSomething,
            };
            let mut listner = handler.subscribe(&message).unwrap();
            let event: Event<MockMsgBody, MockMsgBody> = Event::MessageRecived(message.clone());
            handler.publish_event(event).unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let recv = dbg!(listner.recv().await).unwrap();
            assert_eq!(recv, Event::MessageRecived(message));
        })
        .await;
        dbg!(res.unwrap());
        panic!();
    }

    #[tokio::test]
    async fn specific_event_test() {
        let res = tokio::time::timeout(std::time::Duration::from_millis(20), async move {
            let handler = EventHandler::new();
            let message = Message {
                src: "testsrc".to_string(),
                dest: "testdest".to_string(),
                body: MockMsgBody::DoSomething,
            };

            let mut listner = handler.subscribe_message_recived(&message).unwrap();
            handler
                .publish_event(Event::MessageSent(message.clone()))
                .unwrap();

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;

            let x = listner.try_recv();

            match x {
                Ok(_) => panic!(),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => panic!(),
                _ => (),
            };

            handler
                .publish_event(Event::MessageRecived(message.clone()))
                .unwrap();

            let y = listner.recv().await;

            assert!(matches!(y, Some(MockMsgBody::DoSomething)));
        })
        .await;
        dbg!(res).unwrap();
    }

    pub struct Subscriber {
        message_id: MessageId<usize>,
        callback: Box<dyn Fn()>,
    }

    impl Debug for Subscriber {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Subscriber")
                .field("message_id", &self.message_id)
                .finish_non_exhaustive()
        }
    }

    #[derive(Debug, PartialEq, Eq, Clone)]
    enum Mock {
        First,
    }

    impl IntoBodyId<usize> for Mock {
        fn into_body_id(self) -> BodyId<usize> {
            1.into()
        }

        fn clone_into_body_id(&self) -> BodyId<usize> {
            1.into()
        }
    }

    #[tokio::test]
    async fn wait_test() {
        let handler = EventHandler::<Mock, Mock, _>::new();
        let message = Message {
            src: "1".to_string(),
            dest: 1.to_string(),
            body: Mock::First,
        };
        let event = Event::MessageRecived(message.clone());
        handler.publish_event(event).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let mut recv = handler.subscribe(&message).unwrap();

        dbg!(tokio::time::timeout(
            std::time::Duration::from_millis(10),
            async move {
                dbg!(recv.recv().await);
            }
        ).await).unwrap();
    }

    #[tokio::test]
    async fn two_events_test() {
        let handler = EventHandler::<Mock, Mock, _>::new();
        let message_1 = Message {
            src: 1.to_string(),
            dest: 1.to_string(),
            body: Mock::First,
        };

        let message_2 = Message {
            src: 2.to_string(),
            dest: 2.to_string(),
            body: Mock::First,
        };

        let event_1 = Event::MessageRecived(message_1.clone());
        let event_2 = Event::MessageRecived(message_2.clone());

        handler.publish_event(event_1.clone()).unwrap();
        handler.publish_event(event_2.clone()).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let mut msg_2 = handler.subscribe(&message_2).unwrap();
        let mut msg_1 = handler.subscribe(&message_1).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(0)).await;

        assert_eq!(msg_1.try_recv(), Ok(event_1));
        assert_eq!(msg_2.try_recv(), Ok(event_2));
    }
}

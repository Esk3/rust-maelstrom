use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use crate::{message::Message, Fut};

use super::Service;

#[derive(Debug, Clone)]
pub struct EventLayer<S, Req> {
    inner: S,
    event_broker: EventBroker<Req>,
}

impl<S, Req> EventLayer<S, Req> {
    pub fn new(inner: S, event_broker: EventBroker<Req>) -> Self {
        Self {
            inner,
            event_broker,
        }
    }
}

impl<S, Req, Res> Service<Message<Req>> for EventLayer<S, Req>
where
    S: Service<Message<Req>, Response = Option<Res>> + Clone + Send + 'static,
    Req: Send + Clone + AsBodyId + 'static + Debug,
{
    type Response = Option<Res>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Req>) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            let id = MessageId {
                this: request.dest.to_string(),
                other: request.src.to_string(),
                id: request.body.as_body_id(),
            };
            match this.event_broker.publish_message_id(&id, request) {
                Ok(()) => Ok(None),
                Err(request) => this.inner.call(request).await,
            }
        })
    }
}

#[derive(Debug, Default)]
pub struct EventStore<Key, T> {
    subscribers: HashMap<Key, tokio::sync::oneshot::Sender<T>>,
    events: HashMap<Key, T>,
}

impl<Key, T> EventStore<Key, T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            events: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventBroker<T>(Arc<Mutex<EventStore<MessageId, Message<T>>>>);

impl<T> EventBroker<T>
where
    T: AsBodyId + Debug,
{
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(EventStore::new())))
    }

    pub fn publish_message_id(
        &self,
        id: &MessageId,
        message: Message<T>,
    ) -> Result<(), Message<T>> {
        match self.0.lock().unwrap().subscribers.remove(id) {
            Some(tx) => {
                tx.send(message)?;
                Ok(())
            }
            None => Err(message),
        }
    }

    pub fn publish_or_store_id(&self, id: MessageId, message: Message<T>) {
        if let Err(message) = self.publish_message_id(&id, message) {
            self.0.lock().unwrap().events.insert(id, message);
        }
    }

    #[must_use]
    pub fn subsribe_to_id(&self, id: MessageId) -> tokio::sync::oneshot::Receiver<Message<T>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut lock = self.0.lock().unwrap();
        if let Some(message) = lock.events.remove(&id) {
            tx.send(message).unwrap();
        } else {
            lock.subscribers.insert(id, tx);
        }
        rx
    }
}

impl<T> Default for EventBroker<T>
where
    T: AsBodyId + Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MessageId {
    pub this: String,
    pub other: String,
    pub id: BodyId,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BodyId(pub String);

pub trait AsBodyId {
    fn as_raw_id(&self) -> String;
    fn as_body_id(&self) -> BodyId {
        BodyId(self.as_raw_id())
    }
}

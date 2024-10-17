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
impl<S, Req> Service<Message<Req>> for EventLayer<S, Req>
where
    S: Service<Message<Req>> + Clone + Send + 'static,
    Req: Send + Clone + AsBodyId + 'static + Debug,
{
    type Response = Option<S::Response>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Req>) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            match this.event_broker.publish_message_recived(request) {
                Ok(()) => Ok(None),
                Err(request) => this.inner.call(request).await.map(Some),
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct EventBroker<T>(Arc<Mutex<HashMap<MessageId, tokio::sync::oneshot::Sender<Message<T>>>>>);

impl<T> EventBroker<T>
where
    T: AsBodyId + Debug,
{
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }

    pub fn publish_message_recived(&self, message: Message<T>) -> Result<(), Message<T>> {
        let id = MessageId {
            this: message.dest.to_string(),
            other: message.src.to_string(),
            id: message.body.as_body_id(),
        };
        match self.0.lock().unwrap().remove(&id) {
            Some(tx) => {
                tx.send(message)?;
                Ok(())
            }
            None => Err(message),
        }
    }
    pub fn publish_message_from_self(&self, message: Message<T>) -> Result<(), Message<T>> {
        let id = MessageId {
            this: message.src.to_string(),
            other: message.dest.to_string(),
            id: message.body.as_body_id(),
        };
        match self.0.lock().unwrap().remove(&id) {
            Some(tx) => {
                tx.send(message)?;
                Ok(())
            }
            None => Err(message),
        }
    }

    pub fn subscribe_to_response(
        &self,
        message: &Message<T>,
    ) -> tokio::sync::oneshot::Receiver<Message<T>> {
        let id = MessageId {
            this: message.src.to_string(),
            other: message.dest.to_string(),
            id: message.body.as_body_id(),
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0.lock().unwrap().insert(id, tx);
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
    this: String,
    other: String,
    id: BodyId,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BodyId(String);

pub trait AsBodyId {
    fn as_raw_id(&self) -> String;
    fn as_body_id(&self) -> BodyId {
        BodyId(self.as_raw_id())
    }
}

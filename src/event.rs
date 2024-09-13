use std::collections::HashMap;
use std::fmt::Debug;

use crate::message::Message;

pub struct EventHandler<T> {
    subscribers: HashMap<usize, tokio::sync::oneshot::Sender<Message<T>>>,
}

impl<T> EventHandler<T>
where
    T: Debug,
{
    pub fn new() -> Self {
        todo!()
    }
    pub fn subscribe(&mut self) {}
    pub fn unsubscribe(&mut self) {}
    pub fn handle_event(&mut self, event: Event<T>) -> Option<Event<T>> {
        match event {
            Event::NetworkMessage(_) => Some(event),
            Event::Internal { id, message } => {
                if let Some(consumer) = self.subscribers.remove(&id) {
                    consumer.send(message).unwrap();
                    None
                } else {
                    todo!(
                        "return event, problem is checking if id exsist in subs without moving msg"
                    )
                }
            }
        }
    }
}

pub enum Event<T> {
    NetworkMessage(Message<T>),
    Internal { id: usize, message: Message<T> },
}

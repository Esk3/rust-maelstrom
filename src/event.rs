use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};

use crate::message::Message;

#[derive(Debug, Deserialize)]
pub enum Event<T: EventId> {
    Maelstrom(Message<T>),
    Injected(Message<T>),
    BuiltIn(BuiltInEvent),
}

impl<T: EventId> Event<T> {
    pub fn id(&self) -> usize {
        match self {
            Event::Maelstrom(msg) | Event::Injected(msg) => msg.body.get_event_id(),
            Event::BuiltIn(_) => todo!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BuiltInEvent {
    Read
}

pub trait EventId {
    fn get_event_id(&self) -> usize;
}

#[derive(Debug, Clone)]
pub struct EventBroker<T>
where
    T: EventId + Clone,
{
    new_ids_tx: tokio::sync::mpsc::UnboundedSender<(usize, tokio::sync::oneshot::Sender<Event<T>>)>,
    events_tx: tokio::sync::mpsc::UnboundedSender<Event<T>>,
}

impl<T> EventBroker<T>
where
    T: Debug + Send + 'static + EventId + Clone,
{
    #[must_use]
    pub fn new() -> Self {
        let (new_ids_tx, mut new_ids_rx) = tokio::sync::mpsc::unbounded_channel();
        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut subscribers = HashMap::<usize, tokio::sync::oneshot::Sender<Event<T>>>::new();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = new_ids_rx.recv() => {
                        let (id, tx) = msg.unwrap();
                        subscribers.insert(id, tx);
                    },
                    event = events_rx.recv() => {
                        let event: Event<T> = event.unwrap();
                        let id = event.id();
                        // let (id, body) = match event.unwrap() {
                        //     Event::Maelstrom { dest, body } | Event::Injected { dest, body } => (dest, body),
                        // };
                        let Some(tx) = subscribers.remove(&id) else {
                            return;
                        };
                        tx.send(event).unwrap();
                    }
                }
            }
        });
        Self {
            new_ids_tx,
            events_tx,
        }
    }

    #[must_use]
    pub fn subscribe(&self, id: usize) -> tokio::sync::oneshot::Receiver<Event<T>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.new_ids_tx.send((id, tx)).unwrap();
        rx
    }

    pub fn publish_event(&self, event: Event<T>) -> anyhow::Result<()> {
        if let Err(e) = self.events_tx.send(event) {
            bail!("{e}: channel error on publishing event")
        }
        Ok(())
    }
}

impl<T> Default for EventBroker<T>
where
    T: Debug + Send + 'static + EventId + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::error;
use crate::event::{BuiltInEvent, EventBroker, EventId};
use crate::id_counter::SeqIdCounter;
use crate::message::Message;

#[derive(Debug)]
pub struct LinKv<T: Clone + EventId> {
    node_id: String,
    event_broker: EventBroker<T>,
}

impl<T> LinKv<T>
where
    T: Debug + Send + Clone + EventId + 'static,
{
    #[must_use]
    pub fn new(node_id: String, ids: SeqIdCounter, event_broker: EventBroker<T>) -> Self {
        Self {
            node_id,
            event_broker,
        }
    }
    pub async fn read(&self, key: serde_json::Value, id: usize) -> Result<serde_json::Value, error::Error>
    {
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: Output::<serde_json::Value, serde_json::Value>::Read { key, msg_id: id }, 
        };
        let listner = self.event_broker.subscribe(id);
        message
            .send(std::io::stdout())
            .map_err(|_| error::Error::crash(id))?;
        let response = listner.await.unwrap();

        match response {
            crate::event::Event::Maelstrom(_) => todo!(),
            crate::event::Event::Injected(_) => todo!(),
            crate::event::Event::BuiltIn(message) => match message.body {
                BuiltInEvent::ReadOk { value, msg_id, in_reply_to } => Ok(value),
                BuiltInEvent::WriteOk { msg_id, in_reply_to } => todo!(),
                BuiltInEvent::CasOk { msg_id, in_reply_to } => todo!(),
                BuiltInEvent::Error { in_reply_to, code, text } => todo!(),
            },
        }
    }
    pub async fn write<K, V>(&self, key: K, value: V, msg_id: usize) -> Result<(), error::Error>
    where
        K: Serialize,
        V: Serialize,
    {
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: Output::Write { key, value, msg_id },
        };
        let listner = self.event_broker.subscribe(msg_id);
        message
            .send(std::io::stdout())
            .map_err(|_| error::Error::crash(0 /* TODO msg id*/))?;
        listner.await.unwrap();
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Input<T> {
    ReadOk {
        value: T,
        in_reply_to: usize,
    },
    Error {
        code: usize,
        text: String,
        in_reply_to: usize,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Output<K, V> {
    Read { key: K, msg_id: usize },
    Write { key: K, value: V, msg_id: usize },
}

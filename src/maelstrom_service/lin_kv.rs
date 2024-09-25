use core::panic;
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
    pub async fn read<K, V>(&self, key: K) -> Result<V, error::Error>
    where
        K: Serialize,
        V: Serialize,
        T: ExtractInput<ReturnValue = V>,
    {
        let msg_id = self.event_broker.get_id_counter().next_id();
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: LinKvOutput::<K, V>::Read { key, msg_id },
        };
        let listner = self.event_broker.subscribe(msg_id);
        message
            .send(std::io::stdout())
            .map_err(|_| error::Error::crash(msg_id))?;
        let response = listner.await.unwrap();

        match response {
            crate::event::Event::Injected(msg) => match msg.body.extract_input() {
                LinKvInput::ReadOk { value, in_reply_to } => Ok(value),
                LinKvInput::WriteOk { in_reply_to } => panic!(),
                // TODO fix error code.
                LinKvInput::Error {
                    code,
                    text,
                    in_reply_to,
                } => Err(error::Error::new(
                    error::ErrorCode::KeyDoesNotExist,
                    text,
                    in_reply_to,
                )),
            },
            _ => todo!(),
        }
    }

    pub async fn read_or_default<K, V>(&self, key: K) -> Result<V, error::Error>
    where
        K: Serialize + Clone,
        V: Serialize + Default,
        T: ExtractInput<ReturnValue = V>,
    {
        match self.read(key.clone()).await {
            Ok(value) => Ok(value),
            Err(err) if matches!(err.code(), error::ErrorCode::KeyDoesNotExist) => {
                self.write(key, V::default()).await.map(|_| V::default())
            }
            Err(_) => todo!(),
        }
    }

    pub async fn write<K, V>(&self, key: K, value: V) -> Result<(), error::Error>
    where
        K: Serialize,
        V: Serialize,
        T: ExtractInput<ReturnValue = V>,
    {
        let msg_id = self.event_broker.get_id_counter().next_id();
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: LinKvOutput::Write { key, value, msg_id },
        };
        let listner = self.event_broker.subscribe(msg_id);
        message
            .send(std::io::stdout())
            .map_err(|_| error::Error::crash(0 /* TODO msg id*/))?;
        match listner.await {
            Ok(crate::event::Event::Injected(message)) => match message.body.extract_input() {
                LinKvInput::WriteOk { in_reply_to: _ } => Ok(()),
                LinKvInput::ReadOk { .. } | LinKvInput::Error { .. } => todo!(),
            },
            _ => todo!(),
        }
    }
}

pub trait ExtractInput {
    type ReturnValue;
    fn extract_input(self) -> LinKvInput<Self::ReturnValue>;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinKvInput<T> {
    ReadOk {
        value: T,
        in_reply_to: usize,
    },
    Error {
        code: usize,
        text: String,
        in_reply_to: usize,
    },
    WriteOk {
        in_reply_to: usize,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinKvOutput<K, V> {
    Read { key: K, msg_id: usize },
    Write { key: K, value: V, msg_id: usize },
}

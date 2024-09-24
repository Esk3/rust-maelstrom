use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::error;
use crate::event::{BuiltInEvent, EventBroker, EventId};
use crate::id_counter::Ids;
use crate::message::Message;

#[derive(Debug)]
pub struct LinKv<T: Clone + EventId> {
    node_id: String,
    ids: Ids,
    event_broker: EventBroker<T>,
}

impl<T> LinKv<T>
where
    T: Debug + Send + Clone + EventId + 'static,
{
    pub fn new(node_id: String, ids: Ids, event_broker: EventBroker<T>) -> Self {
        Self {
            node_id,
            ids,
            event_broker,
        }
    }
    pub async fn read<K, V>(&self, key: K, id: usize) -> Result<V, error::Error>
    where
        K: Serialize,
        V: Serialize,
    {
        let msg_id = self.ids.next_id();
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: Output::<K, V>::Read { key, msg_id: id }, //BuiltInEvent::Read, // body: Output::Read {
                                                            //     key: key.clone(),
                                                            //     msg_id,
                                                            // },
        };
        let listner = self.event_broker.subscribe(msg_id);
        message
            .send(std::io::stdout())
            .map_err(|_| error::Error::crash(msg_id))?;
        let response = listner.await.unwrap();

        match response {
            crate::event::Event::Maelstrom(_) => todo!(),
            crate::event::Event::Injected(_) => todo!(),
            crate::event::Event::BuiltIn(_) => todo!(),
        }

        // match response.body {
        //     Input::ReadOk {
        //         value,
        //         in_reply_to: _,
        //     } => Ok(value),
        //     Input::Error { code: 20, .. } => {
        //         self.write(key, V::default()).await.unwrap();
        //         Ok(V::default())
        //     }
        //     Input::Error {
        //         code,
        //         text,
        //         in_reply_to: _,
        //     } => panic!("error creating new key: [{code}]: {text}"),
        //     Input::Txn { .. } | Input::WriteOk { .. } => panic!(),
        // }
    }
    pub async fn write<K, V>(&self, key: K, value: V) -> Result<(), error::Error>
    where
        K: Serialize,
        V: Serialize,
    {
        let msg_id = self.ids.next_id();
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

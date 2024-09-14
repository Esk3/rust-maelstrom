use rust_maelstrom::{
    error::{self, Error},
    event::EventBroker,
    id_counter::Ids,
    message::Message,
    server::{HandlerInput, HandlerResponse, Server},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Server::new(Handler);
    server.run().await?;
    Ok(())
}

#[derive(Debug)]
struct Node {
    id: String,
    ids: Ids,
}

impl Node {}

impl rust_maelstrom::Node for Node {
    fn init(node_id: String, _node_ids: Vec<String>) -> Self {
        Self {
            id: node_id,
            ids: Ids::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Handler;
impl rust_maelstrom::service::Service<HandlerInput<Input, Node>> for Handler {
    type Response = HandlerResponse<Message<Output>, Input>;

    type Future = rust_maelstrom::Fut<Self::Response>;

    fn call(
        &mut self,
        HandlerInput {
            message,
            node,
            event_broker,
        }: rust_maelstrom::server::HandlerInput<Input, Node>,
    ) -> Self::Future {
        let (reply, body) = message.into_reply();
        match body {
            Input::Txn { msg_id, txn } => Box::pin(async move {
                let txn = handle_txn(node, txn, event_broker).await;
                Ok(HandlerResponse::Response(reply.with_body(Output::TxnOk {
                    in_reply_to: msg_id,
                    txn,
                })))
            }),
            Input::ReadOk { in_reply_to, .. }
            | Input::WriteOk { in_reply_to }
            | Input::Error { in_reply_to, .. } => Box::pin(async move {
                Ok(HandlerResponse::Event(
                    rust_maelstrom::server::Event::Injected {
                        dest: in_reply_to,
                        body: reply.into_reply().0.with_body(body),
                    },
                ))
            }),
        }
    }
}

async fn handle_txn(
    node: Arc<Mutex<Node>>,
    txn: Vec<Txn>,
    event_broker: EventBroker<Input>,
) -> Vec<Txn> {
    let node_id;
    let ids;
    {
        let node = node.lock().unwrap();
        node_id = node.id.clone();
        ids = node.ids.clone();
    }
    let mut result = Vec::new();
    let lin_kv_client = LinKv::new(node_id.clone(), ids.clone(), event_broker.clone());
    for op in txn {
        match op {
            Txn::Read(r, key, _) => {
                let value = lin_kv_client.read(key.clone()).await.unwrap();
                result.push(Txn::Read(r, key, Some(value)));
            }
            Txn::Append(a, key, new_value) => {
                let mut values = lin_kv_client.read(key.clone()).await.unwrap();
                values.push(new_value.clone());
                lin_kv_client.write(key.clone(), values).await.unwrap();
                result.push(Txn::Append(a, key, new_value));
            }
        }
    }
    result
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Input {
    Txn {
        msg_id: usize,
        txn: Vec<Txn>,
    },
    /// lin-kv read ok
    ReadOk {
        value: Vec<serde_json::Value>,
        in_reply_to: usize,
    },
    /// lin-kv write
    WriteOk {
        in_reply_to: usize,
    },
    Error {
        code: usize,
        text: String,
        in_reply_to: usize,
    },
}

impl rust_maelstrom::message::MessageId for Input {
    fn get_id(&self) -> usize {
        match self {
            Input::Txn { msg_id, .. } => *msg_id,
            Input::ReadOk { in_reply_to, .. }
            | Input::WriteOk { in_reply_to }
            | Input::Error { in_reply_to, .. } => *in_reply_to,
        }
    }
}

#[derive(Debug)]
struct LinKv {
    node_id: String,
    ids: Ids,
    event_broker: EventBroker<Input>,
}

impl LinKv {
    pub fn new(node_id: String, ids: Ids, event_broker: EventBroker<Input>) -> Self {
        Self {
            node_id,
            ids,
            event_broker,
        }
    }
    async fn read(&self, key: serde_json::Value) -> Result<Vec<serde_json::Value>, error::Error> {
        let msg_id = self.ids.next_id();
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: Output::Read {
                key: key.clone(),
                msg_id,
            },
        };
        let listner = self.event_broker.subscribe(msg_id);
        message
            .send(std::io::stdout())
            .map_err(|_| Error::crash(0 /* TODO msg id */))?;
        let response = listner.await.unwrap();

        match response.body {
            Input::ReadOk {
                value,
                in_reply_to: _,
            } => Ok(value),
            Input::Error { code: 20, .. } => {
                self.write(key, Vec::new()).await.unwrap();
                Ok(Vec::new())
            }
            Input::Error {
                code,
                text,
                in_reply_to: _,
            } => panic!("error creating new key: [{code}]: {text}"),
            Input::Txn { .. } | Input::WriteOk { .. } => panic!(),
        }
    }
    async fn write(
        &self,
        key: serde_json::Value,
        values: Vec<serde_json::Value>,
    ) -> Result<(), Error> {
        let msg_id = self.ids.next_id();
        let message = Message {
            src: self.node_id.clone(),
            dest: "lin-kv".to_string(),
            body: Output::Write {
                key,
                value: serde_json::Value::Array(values),
                msg_id,
            },
        };
        let listner = self.event_broker.subscribe(msg_id);
        message
            .send(std::io::stdout())
            .map_err(|_| Error::crash(0 /* TODO msg id*/))?;
        listner.await.unwrap();
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Output {
    TxnOk {
        in_reply_to: usize,
        txn: Vec<Txn>,
    },
    /// lin-kv read
    Read {
        key: serde_json::Value,
        msg_id: usize,
    },
    /// lin_kv write
    Write {
        key: serde_json::Value,
        value: serde_json::Value,
        msg_id: usize,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum Txn {
    Read(R, serde_json::Value, Option<Vec<serde_json::Value>>),
    Append(Append, serde_json::Value, serde_json::Value),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum R {
    R,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum Append {
    Append,
}

use rust_maelstrom::{
    message::Message,
    server::{EventBroker, HandlerInput, HandlerResponse, Server},
    service,
};
use serde::{de::value, Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[tokio::main]
async fn main() {
    let server = Server::new(Handler);
    server.run().await;
}

#[derive(Debug)]
struct Node {
    id: String,
    state: HashMap<serde_json::Value, Vec<serde_json::Value>>,
}

impl Node {
    pub fn read(&self, key: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
        self.state.get(key)
    }
    pub fn append(&mut self, key: serde_json::Value, value: serde_json::Value) {
        self.state.entry(key).or_default().push(value);
    }
}

impl rust_maelstrom::Node for Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self {
        Self {
            id: node_id,
            state: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Handler;
impl rust_maelstrom::service::Service<HandlerInput<Request, Node>> for Handler {
    type Response = HandlerResponse<Message<Response>, Request>;

    type Future = rust_maelstrom::Fut<Self::Response>;

    fn call(
        &mut self,
        HandlerInput {
            message,
            node,
            event_broker,
        }: rust_maelstrom::server::HandlerInput<Request, Node>,
    ) -> Self::Future {
        let (reply, body) = message.into_reply();
        Box::pin(async move {
            match body {
                Request::Txn { msg_id, txn } => handle_txn(node, txn, msg_id, event_broker).await,
                Request::ReadOk { in_reply_to } => todo!(),
                Request::WriteOk { in_reply_to } => todo!(),
            }
        });
        todo!()
    }
}

async fn handle_txn(
    node: Arc<Mutex<Node>>,
    txn: Vec<Txn>,
    msg_id: usize,
    event_broker: EventBroker<Request>,
) {
    let mut node = node.lock().unwrap();
    let result = Vec::new();
    for op in txn {
        match op {
            Txn::Read(r, key, _) => {
                let value = LinKv::read(key, todo!(), todo!(), event_broker).await;
                result.push(Txn::Read(r, key, Some(value)));
            },
            Txn::Append(a, key, new_value) => {
                let values = LinKv::read(key, todo!(), todo!(), event_broker).await;
                values.push(new_value);
                LinKv::write(key, values, todo!(), todo!(), event_broker).await;
                result.push(Txn::Append(a, key, new_value));
            }
        }
    }
    // Box::pin(async move {
    //     Ok(Response::TxnOk {
    //         in_reply_to: msg_id,
    //         txn,
    //     })
    // });
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
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
}

impl rust_maelstrom::message::MessageId for Request {
    fn get_id(&self) -> usize {
        match self {
            Request::Txn { msg_id, .. } => *msg_id,
            Request::ReadOk { in_reply_to, .. } | Request::WriteOk { in_reply_to } => *in_reply_to,
        }
    }
}

struct LinKv;

impl LinKv {
    async fn read(
        key: serde_json::Value,
        src: String,
        msg_id: usize,
        event_broker: EventBroker<Request>,
    ) -> Vec<serde_json::Value> {
        let message = Message {
            src,
            dest: "lin-kv".to_string(),
            body: Response::Read { key, msg_id },
        };
        let listner = event_broker.subscribe(msg_id);
        message.send(std::io::stdout());
        let Message {
            body: Request::ReadOk { value, .. },
            ..
        } = listner.await.unwrap()
        else {
            panic!();
        };
        value
    }
    async fn write(
        key: serde_json::Value,
        values: Vec<serde_json::Value>,
        src: String,
        msg_id: usize,
        event_broker: EventBroker<Request>,
    ) -> Option<()> {
        let message = Message {
            src,
            dest: "lin-kv".to_string(),
            body: Response::Write { key, value: values, msg_id },
        };
        let listner = event_broker.subscribe(msg_id);
        message.send(std::io::stdout());
        listner.await.unwrap();
        Some(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
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

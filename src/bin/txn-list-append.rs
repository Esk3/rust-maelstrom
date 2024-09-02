use std::{collections::HashMap, future::Future, pin::Pin};

use rust_maelstrom::{main_loop, message::Message, Fut};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    main_loop(rust_maelstrom::handler::Handler::new(Handler)).await;
}

struct Middleware<T> {
    inner: T,
}

impl<T, Req> rust_maelstrom::service::Service<Req> for Middleware<T> {
    type Response = ();

    type Future = Fut<()>;

    fn call(&mut self, request: Req) -> Self::Future {
        todo!()
    }
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
impl
    rust_maelstrom::service::Service<
        rust_maelstrom::handler::RequestArgs<Message<Request>, Response, Node>,
    > for Handler
{
    type Response = Response;

    type Future = rust_maelstrom::Fut<Self::Response>;

    fn call(
        &mut self,
        rust_maelstrom::handler::RequestArgs {
            request,
            node,
            id: _,
            input: _,
        }: rust_maelstrom::handler::RequestArgs<Message<Request>, Response, Node>,
    ) -> Self::Future {
        let mut node = node.lock().unwrap();
        let (txn, msg_id) = match request.body {
            Request::Txn { msg_id, txn } => (txn, msg_id),
        };
        let msg = Message {
            src: node.id.clone(),
            dest: "lin-kv".to_string(),
            body: todo!(),
        };
        let txn = txn
            .into_iter()
            .map(|op| match op {
                Txn::Read(r, key, _) => {
                    let value = node.read(&key).cloned().unwrap_or(Vec::new());
                    Txn::Read(r, key, Some(value))
                }
                Txn::Append(a, key, value) => {
                    node.append(key.clone(), value.clone());
                    Txn::Append(a, key, value)
                }
            })
            .collect();
        Box::pin(async move {
            Ok(Response::TxnOk {
                in_reply_to: msg_id,
                txn,
            })
        })
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Txn { msg_id: usize, txn: Vec<Txn> },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    TxnOk { in_reply_to: usize, txn: Vec<Txn> },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Txn {
    Read(R, serde_json::Value, Option<Vec<serde_json::Value>>),
    Append(Append, serde_json::Value, serde_json::Value),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum R {
    R,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Append {
    Append,
}

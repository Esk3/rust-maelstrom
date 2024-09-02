use std::{future::Future, pin::Pin};

use rust_maelstrom::message::Message;
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {}

struct Node {}

impl rust_maelstrom::Node for Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self {
        dbg!("todo, init node");
        Self {}
    }
}

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
        request: rust_maelstrom::handler::RequestArgs<Message<Request>, Response, Node>,
    ) -> Self::Future {
        Box::pin(async move {
            Ok(Response::TxnOk {
                in_reply_to: 0,
                txn: vec![Txn::Read(
                    R::R,
                    serde_json::Number::from_f64(1.0).into(),
                    None,
                )],
            })
        })
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Txn { msg_id: usize, txn: Vec<Txn> },
}
#[derive(Debug, Serialize)]
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

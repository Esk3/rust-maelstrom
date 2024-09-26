use rust_maelstrom::{
    error,
    event::{self, Event, EventBroker},
    maelstrom_service::lin_kv::{ExtractInput, LinKv, LinKvInput},
    message::Message,
    server::{HandlerInput, HandlerResponse, Server},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = Server::new(Handler);
    server.run().await?;
    Ok(())
}

#[derive(Debug)]
struct Node {
    id: String,
}

impl Node {}

impl rust_maelstrom::Node for Node {
    fn init(node_id: String, _node_ids: Vec<String>) -> Self {
        Self { id: node_id }
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
            event,
            node,
            event_broker,
        }: rust_maelstrom::server::HandlerInput<Input, Node>,
    ) -> Self::Future {
        match event {
            Event::Maelstrom(msg) => {
                Box::pin(async move { handle_message(node, msg, event_broker).await })
            }
            Event::Injected(msg) => panic!("handler got unexpected message: {msg:?}"),
        }
    }
}

async fn handle_message(
    node: Arc<Mutex<Node>>,
    message: Message<Input>,
    event_broker: EventBroker<Input>,
) -> anyhow::Result<HandlerResponse<Message<Output>, Input>> {
    let (reply, body) = message.into_reply();
    match body {
        Input::Txn { msg_id, txn } => {
            let txn = handle_txn(node, txn, event_broker).await;
            match txn {
                Ok(txn) => Ok(HandlerResponse::Response(reply.with_body(Output::TxnOk {
                    in_reply_to: msg_id,
                    txn,
                }))),
                Err(error) => Ok(HandlerResponse::Error(
                    reply.with_body(error.set_in_reply_to(msg_id)),
                )),
            }
        }
        Input::LinKv(_) => Ok(HandlerResponse::Event(Event::Injected(
            reply.into_reply().0.with_body(body),
        ))),
    }
}

async fn handle_txn(
    node: Arc<Mutex<Node>>,
    txn: Vec<Txn>,
    event_broker: EventBroker<Input>,
) -> Result<Vec<Txn>, error::Error> {
    let node_id;
    {
        let node = node.lock().unwrap();
        node_id = node.id.clone();
    }
    let lin_kv_client = LinKv::new(node_id.clone(), event_broker.clone());
    let old_tree = lin_kv_client.read_or_default(0).await.unwrap();
    let mut new_tree = old_tree.clone();
    let txn = txn
        .into_iter()
        .map(|op| match op {
            Txn::Read(r, key, _) => {
                // to string because serde_json::value::String(x) != serde_json::value::Number(x).
                // but to maelstrom "1" == 1
                let values = new_tree.entry(key.to_string()).or_default().clone();
                Txn::Read(r, key, Some(values))
            }
            Txn::Append(a, key, new_value) => {
                dbg!(&new_tree, &new_value);
                new_tree
                    .entry(key.to_string())
                    .or_default()
                    .push(new_value.clone());
                dbg!(&new_tree, &new_value);
                Txn::Append(a, key, new_value)
            }
        })
        .collect();
    dbg!(&old_tree, &new_tree);
    lin_kv_client.cas(0, old_tree, new_tree).await.map(|()| txn)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Input {
    Txn {
        msg_id: usize,
        txn: Vec<Txn>,
    },
    #[serde(untagged)]
    LinKv(LinKvInput<HashMap<String, Vec<serde_json::Value>>>),
}

impl ExtractInput for Input {
    type ReturnValue = HashMap<String, Vec<serde_json::Value>>;
    fn extract_input(self) -> Option<LinKvInput<Self::ReturnValue>> {
        match self {
            Input::Txn { .. } => None,
            Input::LinKv(lin_kv) => Some(lin_kv),
        }
    }
}

impl event::EventId for Input {
    fn get_event_id(&self) -> usize {
        match &self {
            Input::Txn { msg_id, txn: _ } => *msg_id,
            Input::LinKv(lin_kv) => match lin_kv {
                LinKvInput::ReadOk { in_reply_to, .. }
                | LinKvInput::WriteOk { in_reply_to }
                | LinKvInput::Error { in_reply_to, .. }
                | LinKvInput::CasOk { in_reply_to } => *in_reply_to,
            },
        }
    }
}

impl rust_maelstrom::message::MessageId for Input {
    fn get_id(&self) -> usize {
        match self {
            Input::Txn { msg_id, .. } => *msg_id,
            Input::LinKv(lin_kv) => match lin_kv {
                LinKvInput::ReadOk { in_reply_to, .. }
                | LinKvInput::WriteOk { in_reply_to }
                | LinKvInput::Error { in_reply_to, .. }
                | LinKvInput::CasOk { in_reply_to } => *in_reply_to,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Output {
    TxnOk { in_reply_to: usize, txn: Vec<Txn> },
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

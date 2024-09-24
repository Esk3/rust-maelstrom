use rust_maelstrom::{
    event::{self, BuiltInEvent, Event, EventBroker},
    id_counter::SeqIdCounter,
    maelstrom_service::lin_kv::LinKv,
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
    ids: SeqIdCounter,
}

impl Node {}

impl rust_maelstrom::Node for Node {
    fn init(node_id: String, _node_ids: Vec<String>) -> Self {
        Self {
            id: node_id,
            ids: SeqIdCounter::new(),
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
            event,
            node,
            event_broker,
        }: rust_maelstrom::server::HandlerInput<Input, Node>,
    ) -> Self::Future {
        match event {
            Event::Maelstrom(msg) => {
                Box::pin(async move { handle_message(node, msg, event_broker).await })
            }
            Event::BuiltIn(message) => {
                Box::pin(async move { handle_built_in_event(message, event_broker) })
            }
            Event::Injected(msg) => panic!("handler got unexpected message: {msg:?}"),
        }
    }
}

fn handle_built_in_event(
    message: Message<BuiltInEvent>,
    event_broker: EventBroker<Input>,
) -> anyhow::Result<HandlerResponse<Message<Output>, Input>> {
    event_broker.publish_event(Event::BuiltIn(message)).unwrap();
    Ok(HandlerResponse::None)
    // Ok(HandlerResponse::Event(Event::Injected(message)))
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
            Ok(HandlerResponse::Response(reply.with_body(Output::TxnOk {
                in_reply_to: msg_id,
                txn,
            })))
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
                let value = lin_kv_client
                    .read(key.clone(), event_broker.get_id_counter().next_id())
                    .await
                    .unwrap();
                result.push(Txn::Read(r, key, Some(value)));
            }
            Txn::Append(a, key, new_value) => {
                let mut values = lin_kv_client
                    .read(key.clone(), event_broker.get_id_counter().next_id())
                    .await
                    .unwrap();
                values.push(new_value.clone());
                lin_kv_client.write(key.clone(), values, event_broker.get_id_counter().next_id()).await.unwrap();
                result.push(Txn::Append(a, key, new_value));
            }
        }
    }
    result
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Input {
    Txn { msg_id: usize, txn: Vec<Txn> },
}

impl event::EventId for Input {
    fn get_event_id(&self) -> usize {
        match &self {
            Input::Txn { msg_id, txn: _ } => *msg_id,
        }
    }
}

impl rust_maelstrom::message::MessageId for Input {
    fn get_id(&self) -> usize {
        match self {
            Input::Txn { msg_id, .. } => *msg_id,
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

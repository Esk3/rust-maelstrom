use rust_maelstrom::{
    event::{self, Event, EventBroker},
    maelstrom_service::lin_kv::{ExtractInput, LinKv, LinKvInput},
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
            Ok(HandlerResponse::Response(reply.with_body(Output::TxnOk {
                in_reply_to: msg_id,
                txn,
            })))
        }
        // Input::LinKv(ref lin_kv) => Ok(match lin_kv {
        //     LinKvInput::ReadOk { value, in_reply_to } => HandlerResponse::Event(Event::Injected(reply.into_reply().0.with_body(body))),
        //     LinKvInput::Error { code, text, in_reply_to } => todo!(),
        // }),
        Input::LinKv(_) => Ok(HandlerResponse::Event(Event::Injected(
            reply.into_reply().0.with_body(body),
        ))),
    }
}

async fn handle_txn(
    node: Arc<Mutex<Node>>,
    txn: Vec<Txn>,
    event_broker: EventBroker<Input>,
) -> Vec<Txn> {
    let node_id;
    {
        let node = node.lock().unwrap();
        node_id = node.id.clone();
    }
    let mut result = Vec::new();
    let lin_kv_client = LinKv::new(node_id.clone(), event_broker.clone());
    for op in txn {
        match op {
            Txn::Read(r, key, _) => {
                let value = lin_kv_client.read_or_default(key.clone()).await.unwrap();
                result.push(Txn::Read(r, key, Some(value)));
            }
            Txn::Append(a, key, new_value) => {
                let mut values = lin_kv_client.read_or_default(key.clone()).await.unwrap();
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
    #[serde(untagged)]
    LinKv(LinKvInput<Vec<serde_json::Value>>),
}

impl ExtractInput for Input {
    type ReturnValue = Vec<serde_json::Value>;
    fn extract_input(self) -> Option<LinKvInput<Self::ReturnValue>> {
        match self {
            Input::Txn {..} => None,
            Input::LinKv(lin_kv) => Some(lin_kv),
        }
    }
}

#[test]
fn parsetest() {
    let s = r#"{"id":14,"src":"lin-kv","dest":"n1","body":{"type":"error","code":20,"text":"key does not exist","in_reply_to":1}}"#;
    dbg!(&s);
    let value = serde_json::from_str::<Message<Input>>(s);
    value.unwrap();
}

impl event::EventId for Input {
    fn get_event_id(&self) -> usize {
        match &self {
            Input::Txn { msg_id, txn: _ } => *msg_id,
            Input::LinKv(lin_kv) => match lin_kv {
                LinKvInput::ReadOk { in_reply_to, .. }
                | LinKvInput::WriteOk { in_reply_to }
                | LinKvInput::Error { in_reply_to, .. } => *in_reply_to,
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
                | LinKvInput::Error { in_reply_to, .. } => *in_reply_to,
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

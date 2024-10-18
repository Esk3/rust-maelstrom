use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use rust_maelstrom::{
    error,
    id_counter::SeqIdCounter,
    maelstrom_service::lin_kv::{LinKvClient, LinKvInput},
    message::Message,
    node_handler::NodeHandler,
    service::{
        event::{AsBodyId, EventBroker, EventLayer},
        json::JsonLayer,
        Service,
    },
    Fut, Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let w = Arc::new(Mutex::new(std::io::stdout()));
    let lin_kv_client = LinKvClient::new("empty", w.clone());
    let event_broker = EventBroker::<Input>::new();
    let node = NodeHandler::<()>::init_node::<TxnListAppendNode, _>((
        event_broker.clone(),
        lin_kv_client.clone(),
    ))
    .await;
    let event_layer = EventLayer::new(node, event_broker);
    let router = RoutingLayer {
        txn: event_layer,
        lin_kv: lin_kv_client,
    };
    let service = JsonLayer::new(router);
    let handler = NodeHandler::new(service);
    handler.run_with_io(tokio::io::stdin(), w).await;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Inputs {
    Txn(Input),
    LinKv(LinKvInput<HashMap<String, Vec<serde_json::Value>>>),
}

#[derive(Debug, Clone)]
pub struct RoutingLayer<T> {
    txn: T,
    lin_kv: LinKvClient<HashMap<String, Vec<serde_json::Value>>, std::io::Stdout>,
}

impl<T> Service<Message<Inputs>> for RoutingLayer<T>
where
    T: Service<Message<Input>, Response = Option<Message<Output>>> + Clone + Send + 'static,
{
    type Response = Option<Message<Output>>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Inputs>) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            let (msg, body) = request.split();
            match body {
                Inputs::Txn(body) => this.txn.call(msg.with_body(body)).await,
                Inputs::LinKv(body) => {
                    this.lin_kv.handle_message(msg.with_body(body));
                    Ok(None)
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TxnListAppendNode {
    node_id: String,
    event_broker: EventBroker<Input>,
    lin_kv_client: LinKvClient<HashMap<String, Vec<serde_json::Value>>, std::io::Stdout>,
    id_counter: SeqIdCounter,
}
impl
    Node<(
        EventBroker<Input>,
        LinKvClient<HashMap<String, Vec<serde_json::Value>>, std::io::Stdout>,
    )> for TxnListAppendNode
{
    fn init(
        node_id: String,
        _node_ids: Vec<String>,
        (event_broker, lin_kv_client): (
            EventBroker<Input>,
            LinKvClient<HashMap<String, Vec<serde_json::Value>>, std::io::Stdout>,
        ),
    ) -> Self {
        lin_kv_client.set_src(&node_id);
        Self {
            node_id,
            event_broker,
            lin_kv_client,
            id_counter: SeqIdCounter::new(),
        }
    }
}

impl Service<Message<Input>> for TxnListAppendNode {
    type Response = Option<Message<Output>>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Input>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { Ok(this.handle_message(request).await) })
    }
}

impl TxnListAppendNode {
    async fn handle_message(&self, message: Message<Input>) -> Option<Message<Output>> {
        let (reply, body) = message.into_reply();
        match body {
            Input::Txn { msg_id, txn } => {
                let txn = self.handle_txn(txn).await;
                Some(reply.with_body(Output::TxnOk {
                    in_reply_to: msg_id,
                    txn: txn.unwrap(),
                }))
            }
        }
    }

    async fn handle_txn(&self, txn: Vec<Txn>) -> Result<Vec<Txn>, error::Error> {
        let old_tree = self
            .lin_kv_client
            .read(0, self.id_counter.next_id())
            .await
            .unwrap();
        let old_tree = if let Some(old_tree) = old_tree {
            old_tree
        } else {
            self.lin_kv_client
                .send_write(0, HashMap::new(), self.id_counter.next_id())
                .await
                .unwrap();
            HashMap::new()
        };
        let mut new_tree = old_tree.clone();
        let txn = txn
            .into_iter()
            .map(|op| match op {
                Txn::Read(r, key, _) => {
                    let values = new_tree.entry(key.to_string()).or_default().clone();
                    Txn::Read(r, key, Some(values))
                }
                Txn::Append(a, key, new_value) => {
                    new_tree
                        .entry(key.to_string())
                        .or_default()
                        .push(new_value.clone());
                    Txn::Append(a, key, new_value)
                }
            })
            .collect();
        self.lin_kv_client
            .cas(0, old_tree, new_tree, self.id_counter.next_id())
            .await
            .map(|()| txn)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Input {
    Txn { msg_id: usize, txn: Vec<Txn> },
    //#[serde(untagged)]
    //LinKv(LinKvInput<HashMap<String, Vec<serde_json::Value>>>),
}

impl AsBodyId for Input {
    fn as_raw_id(&self) -> String {
        match self {
            Input::Txn { msg_id, txn: _ } => msg_id.to_string(),
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

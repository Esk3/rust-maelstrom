use rust_maelstrom::{
    error,
    message::Message,
    node_handler::NodeHandler,
    service::{
        event::{AsBodyId, EventBroker, EventLayer},
        json::JsonLayer,
        Service,
    },
    Fut,
    Node, //server::{HandlerInput, HandlerResponse, Server},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // router service. json deserializes input<Input, LinKvInput>. router input<Input, LinKvInput>
    // and gives to node or linkv client
    let event_broker = EventBroker::<Input>::new();
    let node = NodeHandler::<()>::init_node::<TxnListAppendNode, _>(event_broker.clone()).await;
    let service = JsonLayer::new(EventLayer::new(node, event_broker));
    let handler = NodeHandler::new(service);
    handler.run().await;
    Ok(())
}

#[derive(Debug, Clone)]
enum Route<A, B> {
    A(A),
    B(B),
}
struct Test;

#[derive(Debug, Clone)]
pub struct RoutingLayer<A, B> {
    a: A,
    b: B,
}

impl<A, B> Service<Message<Route<Input, Test>>> for RoutingLayer<A, B>
where
    A: Service<Message<Input>, Response = Option<Message<Output>>> + Clone + Send + 'static,
    B: Service<Message<Test>> + Clone + Send + 'static,
{
    type Response = Option<Message<Output>>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Route<Input, Test>>) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            let (msg, body) = request.split();
            match body {
                Route::A(input) => this.a.call(msg.with_body(input)).await,
                Route::B(test) => {
                    this.b.call(msg.with_body(test)).await?;
                    Ok(None)
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
struct TxnListAppendNode {}
impl Node<EventBroker<Input>> for TxnListAppendNode {
    fn init(node_id: String, node_ids: Vec<String>, state: EventBroker<Input>) -> Self {
        todo!()
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
        let (msg, body) = message.split();
        match body {
            Input::Txn { msg_id, txn } => todo!(),
        }
    }
}

//
//#[derive(Debug)]
//struct Node {
//    id: String,
//}
//
//impl Node {}
//
//impl rust_maelstrom::Node for Node {
//    fn init(node_id: String, _node_ids: Vec<String>, state: ()) -> Self {
//        Self { id: node_id }
//    }
//}
//
//#[derive(Debug, Clone)]
//struct Handler;
//impl rust_maelstrom::service::Service<HandlerInput<Input, Node>> for Handler {
//    type Response = HandlerResponse<Message<Output>, Input>;
//
//    type Future = rust_maelstrom::Fut<Self::Response>;
//
//    fn call(
//        &mut self,
//        HandlerInput {
//            event,
//            node,
//            event_broker,
//        }: rust_maelstrom::server::HandlerInput<Input, Node>,
//    ) -> Self::Future {
//        match event {
//            Event::Maelstrom(msg) => {
//                Box::pin(async move { handle_message(node, msg, event_broker).await })
//            }
//            Event::Injected(msg) => panic!("handler got unexpected message: {msg:?}"),
//        }
//    }
//}
//
//async fn handle_message(
//    node: Arc<Mutex<Node>>,
//    message: Message<Input>,
//    event_broker: EventBroker<Input>,
//) -> anyhow::Result<HandlerResponse<Message<Output>, Input>> {
//    let (reply, body) = message.into_reply();
//    match body {
//        Input::Txn { msg_id, txn } => {
//            let txn = handle_txn(node, txn, event_broker).await;
//            match txn {
//                Ok(txn) => Ok(HandlerResponse::Response(reply.with_body(Output::TxnOk {
//                    in_reply_to: msg_id,
//                    txn,
//                }))),
//                Err(error) => Ok(HandlerResponse::Error(
//                    reply.with_body(error.set_in_reply_to(msg_id)),
//                )),
//            }
//        }
//        Input::LinKv(_) => Ok(HandlerResponse::Event(Event::Injected(
//            reply.into_reply().0.with_body(body),
//        ))),
//    }
//}
//
//async fn handle_txn(
//    node: Arc<Mutex<Node>>,
//    txn: Vec<Txn>,
//    event_broker: EventBroker<Input>,
//) -> Result<Vec<Txn>, error::Error> {
//    let node_id;
//    {
//        let node = node.lock().unwrap();
//        node_id = node.id.clone();
//    }
//    let lin_kv_client = LinKv::new(node_id.clone(), event_broker.clone());
//    let old_tree = lin_kv_client.read_or_default(0).await.unwrap();
//    let mut new_tree = old_tree.clone();
//    let txn = txn
//        .into_iter()
//        .map(|op| match op {
//            Txn::Read(r, key, _) => {
//                // to string because serde_json::value::String(x) != serde_json::value::Number(x).
//                // but to maelstrom "1" == 1
//                let values = new_tree.entry(key.to_string()).or_default().clone();
//                Txn::Read(r, key, Some(values))
//            }
//            Txn::Append(a, key, new_value) => {
//                new_tree
//                    .entry(key.to_string())
//                    .or_default()
//                    .push(new_value.clone());
//                Txn::Append(a, key, new_value)
//            }
//        })
//        .collect();
//    lin_kv_client.cas(0, old_tree, new_tree).await.map(|()| txn)
//}
//
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
            Input::Txn { msg_id, txn } => msg_id.to_string(),
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

fn main() {}

//use std::sync::{Arc, Mutex};
//
//use rust_maelstrom::{
//    message::Message,
//    node_handler::NodeHandler,
//    service::{
//        event::{AsBodyId, EventBroker},
//        json::JsonLayer,
//        Service,
//    },
//    Fut, Node,
//};
//use serde::{Deserialize, Serialize};
//
//#[tokio::main]
//async fn main() {
//    let event_broker = EventBroker::new();
//    let node = NodeHandler::<()>::init_node::<GNode, _>(event_broker).await;
//    let service = JsonLayer::<_, Message<GRequest>>::new(node);
//    let handler = NodeHandler::new(service);
//    handler.run().await;
//}
//
//#[derive(Debug)]
//struct State {
//    count: usize,
//    seen_uuids: Vec<String>,
//}
//
//#[derive(Debug, Clone)]
//struct GNode {
//    id: String,
//    nodes: Vec<String>,
//    state: Arc<Mutex<State>>,
//    event_broker: EventBroker<GRequest>,
//}
//
//impl Node<EventBroker<GRequest>> for GNode {
//    fn init(node_id: String, node_ids: Vec<String>, event_broker: EventBroker<GRequest>) -> Self {
//        Self {
//            id: node_id,
//            nodes: node_ids,
//            state: Arc::new(Mutex::new(State {
//                count: 0,
//                seen_uuids: Vec::new(),
//            })),
//            event_broker,
//        }
//    }
//}
//impl Service<Message<GRequest>> for GNode {
//    type Response = Option<Message<GResponse>>;
//
//    type Future = Fut<Self::Response>;
//
//    fn call(&mut self, request: Message<GRequest>) -> Self::Future {
//        let this = self.clone();
//        Box::pin(async move {
//            let (reply, body) = request.into_reply();
//            let body = this.handle_message(body).await;
//            Ok(Some(reply.with_body(body)))
//        })
//    }
//}
//
//impl GNode {
//    async fn handle_message(&self, body: GRequest) -> GResponse {
//        match body {
//            GRequest::Add {
//                delta,
//                msg_id,
//                uuid,
//            } => {
//                let uuid = format!("{}{}", self.id, msg_id);
//                {
//                    let mut state = self.state.lock().unwrap();
//                    if state.seen_uuids.contains(&uuid) {};
//                    state.count += delta;
//                    state.seen_uuids.push(uuid);
//                    drop(state);
//                }
//                self.send_to_neigbors().await;
//                todo!();
//            }
//            GRequest::Read { msg_id } => todo!(),
//            GRequest::AddOk { in_reply_to } => todo!(),
//        }
//    }
//    async fn send_to_neigbors(&self) {
//        // event broker
//        // writer
//        todo!()
//    }
//}
//
//#[derive(Debug, Serialize, Deserialize)]
//#[serde(rename_all = "snake_case", tag = "type")]
//enum GRequest {
//    Add {
//        delta: usize,
//        msg_id: usize,
//        uuid: Option<String>,
//    },
//    Read {
//        msg_id: usize,
//    },
//    AddOk {
//        in_reply_to: usize,
//    },
//}
//
//impl AsBodyId for GRequest {
//    fn as_raw_id(&self) -> String {
//        match self {
//            GRequest::Add {
//                delta: _,
//                msg_id: id,
//                uuid: _,
//            }
//            | GRequest::Read { msg_id: id }
//            | GRequest::AddOk { in_reply_to: id } => id.to_string(),
//        }
//    }
//}
//
//#[derive(Debug, Serialize, Deserialize)]
//#[serde(rename_all = "snake_case", tag = "type")]
//enum GResponse {
//    AddOk { in_reply_to: usize },
//    ReadOk { value: usize, in_reply_to: usize },
//}

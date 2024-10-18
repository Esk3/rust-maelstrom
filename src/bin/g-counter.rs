use std::{future::Future, pin::Pin};

use rust_maelstrom::{
    //handler::RequestArgs,
    message::{Message, PeerMessage},
    service::Service,
    Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    // let main_loop = MainLoop::new(Handler::new(MaelstromHandler));
    // main_loop.run().await;
}
//
//#[derive(Debug)]
//struct GNode {
//    id: String,
//    nodes: Vec<String>,
//    count: usize,
//    seen_uuids: Vec<String>,
//}
//
//impl Node for GNode {
//    fn init(node_id: String, node_ids: Vec<String>) -> Self {
//        Self {
//            id: node_id,
//            nodes: node_ids,
//            count: 0,
//            seen_uuids: Vec::new(),
//        }
//    }
//}
//
//impl GNode {
//    fn add(&mut self, value: usize) {
//        self.count += value;
//    }
//    fn read(&self) -> usize {
//        self.count
//    }
//}
//
//#[derive(Clone)]
//struct MaelstromHandler;
//impl Service<rust_maelstrom::handler::RequestArgs<Message<GRequest>, GResponse, GNode>>
//    for MaelstromHandler
//{
//    type Response = GResponse;
//
//    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>> + Send>>;
//    fn call(&mut self, request: RequestArgs<Message<GRequest>, GResponse, GNode>) -> Self::Future {
//        dbg!(&request);
//        match request.request.body {
//            GRequest::Add {
//                delta,
//                msg_id,
//                uuid,
//            } => Box::pin(async move {
//                dbg!("add");
//                let mut node = request.node.lock().unwrap();
//                let uuid = uuid.unwrap_or_else(|| format!("{}{msg_id}", node.id));
//                if !node.seen_uuids.contains(&uuid) {
//                    dbg!("new uuid", &uuid);
//                    node.add(delta);
//                    node.seen_uuids.push(uuid.clone());
//                    let mut out = std::io::stdout().lock();
//                    dbg!(&out);
//                    for nei in node
//                        .nodes
//                        .iter()
//                        .filter(|n| n.as_str() != node.id)
//                        .filter(|n| n.as_str() != request.request.src)
//                    {
//                        Message {
//                            src: node.id.clone(),
//                            dest: nei.clone(),
//                            body: PeerMessage {
//                                src: request.id,
//                                dest: None,
//                                id: request.id,
//                                body: GRequest::Add {
//                                    delta,
//                                    msg_id,
//                                    uuid: Some(uuid.clone()),
//                                },
//                            },
//                        }
//                        .send(&mut out)?;
//                    }
//                }
//                Ok(GResponse::AddOk {
//                    in_reply_to: msg_id,
//                })
//            }),
//            GRequest::Read { msg_id } => Box::pin(async move {
//                dbg!("read");
//                let node = request.node.lock().unwrap();
//                let value = node.read();
//                Ok(GResponse::ReadOk {
//                    value,
//                    in_reply_to: msg_id,
//                })
//            }),
//        }
//    }
//}
//
//#[tokio::test]
//async fn handler_test() {
//    use rust_maelstrom::handler::Handler;
//    use std::sync::{Arc, Mutex};
//    let mut handler = Handler::new(MaelstromHandler);
//    let node = GNode {
//        id: "test_node".to_string(),
//        nodes: Vec::new(),
//        count: 0,
//        seen_uuids: Vec::new(),
//    };
//    let node = Arc::new(Mutex::new(node));
//    let request = rust_maelstrom::handler::HandlerRequest {
//        request: rust_maelstrom::handler::RequestType::Maelstrom(Message {
//            src: "test_src".to_string(),
//            dest: "test dest".to_string(),
//            body: GRequest::Read { msg_id: 1 },
//        }),
//        node: node.clone(),
//        id: 1,
//        input: tokio::sync::mpsc::unbounded_channel().1,
//    };
//    let response = handler.call(request).await;
//    assert!(matches!(response, Ok(Message { body: _my_res, .. })));
//}
//
//#[tokio::test]
//async fn add_test() {
//    use rust_maelstrom::handler::Handler;
//    use std::sync::{Arc, Mutex};
//    let mut handler = Handler::new(MaelstromHandler);
//    let node = GNode {
//        id: "Test node".to_string(),
//        nodes: Vec::new(),
//        count: 0,
//        seen_uuids: Vec::new(),
//    };
//    let node = Arc::new(Mutex::new(node));
//    let num = 2;
//    let request = rust_maelstrom::handler::HandlerRequest {
//        request: rust_maelstrom::handler::RequestType::Maelstrom(Message {
//            src: "testing src".to_string(),
//            dest: "testing dest".to_string(),
//            body: GRequest::Add {
//                delta: num,
//                msg_id: 1,
//                uuid: None,
//            },
//        }),
//        node: node.clone(),
//        id: 1,
//        input: tokio::sync::mpsc::unbounded_channel().1,
//    };
//    let response = handler.call(request).await;
//    assert!(response.is_ok());
//    dbg!(response.unwrap());
//
//    let request = rust_maelstrom::handler::HandlerRequest {
//        request: rust_maelstrom::handler::RequestType::Maelstrom(Message {
//            src: "testing src".to_string(),
//            dest: "testing destl".to_string(),
//            body: GRequest::Read { msg_id: 2 },
//        }),
//        node: node.clone(),
//        id: 2,
//        input: tokio::sync::mpsc::unbounded_channel().1,
//    };
//    let response = handler.call(request).await;
//    dbg!(response.unwrap());
//    dbg!(&node);
//    assert_eq!(node.lock().unwrap().count, num);
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
//}
//#[derive(Debug, Serialize, Deserialize)]
//#[serde(rename_all = "snake_case", tag = "type")]
//enum GResponse {
//    AddOk { in_reply_to: usize },
//    ReadOk { value: usize, in_reply_to: usize },
//}

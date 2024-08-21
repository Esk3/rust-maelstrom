use std::{
    future::Future,
    io::stdin,
    pin::Pin,
    sync::{Arc, Mutex},
};

use rust_maelstrom::{
    message::Message, Handler, HandlerRequest, HandlerResponse, MaelstromRequest,
    MaelstromResponse, Node, PeerResponse, RequestArgs, RequestType, Service,
};
use serde::{Deserialize, Serialize};

fn main() {}

// #[tokio::test]
// async fn test() {
//     rust_maelstrom::main_service_loop(Handler::new(MaelstromHandler)).await;
//     todo!()
// }

#[derive(Debug)]
struct GNode {
    id: String,
    count: usize,
}

impl Node for GNode {
    fn init(node_id: String, node_ids: Vec<String>) -> Self {
        Self {
            id: node_id,
            count: 0,
        }
    }
}

impl GNode {
    fn add(&mut self, value: usize) {
        self.count += value;
    }
    fn read(&self) -> usize {
        self.count
    }
}

#[derive(Clone)]
struct MaelstromHandler;
impl Service<rust_maelstrom::RequestArgs<Message<GRequest>, GNode>> for MaelstromHandler {
    type Response = GResponse;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;
    fn call(&mut self, request: RequestArgs<Message<GRequest>, GNode>) -> Self::Future {
        match request.request.body {
            GRequest::Add { delta, msg_id } => Box::pin(async move {
                let mut node = request.node.lock().unwrap();
                node.add(delta);
                Ok(GResponse::AddOk {
                    in_reply_to: msg_id,
                })
            }),
            GRequest::Read { msg_id } => Box::pin(async move {
                let node = request.node.lock().unwrap();
                let value = node.read();
                Ok(GResponse::ReadOk {
                    value,
                    in_reply_to: msg_id,
                })
            }),
        }
    }
}

#[tokio::test]
async fn handler_test() {
    let mut handler = Handler::new(MaelstromHandler);
    let node = GNode {
        id: "test_node".to_string(),
        count: 0,
    };
    let node = Arc::new(Mutex::new(node));
    let request = HandlerRequest {
        request: RequestType::Maelstrom(Message {
            src: "test_src".to_string(),
            dest: "test dest".to_string(),
            body: GRequest::Read { msg_id: 1 },
        }),
        node: node.clone(),
        id: 1,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let response = handler.call(request).await;
    assert!(matches!(response, Ok(Message { body: MyRes, .. })));
}

#[tokio::test]
async fn add_test() {
    let mut handler = Handler::new(MaelstromHandler);
    let node = GNode {
        id: "Test node".to_string(),
        count: 0,
    };
    let node = Arc::new(Mutex::new(node));
    let num = 2;
    let request = HandlerRequest {
        request: RequestType::Maelstrom(Message {
            src: "testing src".to_string(),
            dest: "testing dest".to_string(),
            body: GRequest::Add {
                delta: num,
                msg_id: 1,
            },
        }),
        node: node.clone(),
        id: 1,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let response = handler.call(request).await;
    assert!(response.is_ok());
    dbg!(response);

    let request = HandlerRequest {
        request: RequestType::Maelstrom(Message {
            src: "testing src".to_string(),
            dest: "testing destl".to_string(),
            body: GRequest::Read { msg_id: 2 },
        }),
        node: node.clone(),
        id: 2,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let response = handler.call(request).await;
    dbg!(response);
    dbg!(&node);
    assert_eq!(node.lock().unwrap().count, num);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum GRequest {
    Add { delta: usize, msg_id: usize },
    Read { msg_id: usize },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum GResponse {
    AddOk { in_reply_to: usize },
    ReadOk { value: usize, in_reply_to: usize },
}

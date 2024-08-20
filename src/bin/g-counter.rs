use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use rust_maelstrom::{message::Message, Handler, HandlerRequest, HandlerResponse, MaelstromRequest, MaelstromResponse, Node, PeerResponse, RequestArgs, RequestType, Service};

fn main() {}

#[tokio::test]
async fn test() {
    rust_maelstrom::main_service_loop(MaelstromHandler, PeerHandler).await;
    todo!()
}

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
impl Service<rust_maelstrom::RequestArgs<GNode>> for MaelstromHandler {
    type Response = MaelstromResponse;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;
    fn call(&mut self, request: RequestArgs<GNode>) -> Self::Future {
        match request.request {
            MaelstromRequest::Add(value) => Box::pin(async move {
                let mut node = request.node.lock().unwrap();
                node.add(value);
                Ok(MaelstromResponse::AddOk)
            }),
            MaelstromRequest::Read => Box::pin(async move {
                let node = request.node.lock().unwrap();
                Ok(MaelstromResponse::ReadOk(node.read()))
            }),
        }
    }
}

#[derive(Clone)]
struct PeerHandler;
impl<N> Service<RequestArgs<N>> for PeerHandler {
    type Response = PeerResponse;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: RequestArgs<N>) -> Self::Future {
        match request.request {
            MaelstromRequest::Add(_) => todo!(),
            MaelstromRequest::Read => todo!(),
        }
    }
}


#[tokio::test]
async fn handler_test() {
    let mut handler = Handler {
        maelstrom_handler: MaelstromHandler,
        peer_handler: PeerHandler,
    };
    let node = GNode {
        id: "test_node".to_string(),
        count: 0,
    };
    let node = Arc::new(Mutex::new(node));
    let request = HandlerRequest {
        request: RequestType::MaelstromRequest(MaelstromRequest::Read),
        node: node.clone(),
        id: 1,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let response = handler.call(request).await;
    assert!(
        matches!(response, Ok(Message{body: HandlerResponse::Maelstrom(MaelstromResponse::ReadOk(value)), ..}) if value == 0)
    );
}

#[tokio::test]
async fn add_test() {
    let mut handler = Handler {
        maelstrom_handler: MaelstromHandler,
        peer_handler: PeerHandler,
    };
    let node = GNode {
        id: "Test node".to_string(),
        count: 0,
    };
    let node = Arc::new(Mutex::new(node));
    let num = 2;
    let request = HandlerRequest {
        request: RequestType::MaelstromRequest(MaelstromRequest::Add(num)),
        node: node.clone(),
        id: 1,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let response = handler.call(request).await;
    assert!(response.is_ok());
    assert_eq!(node.lock().unwrap().count, num);
}


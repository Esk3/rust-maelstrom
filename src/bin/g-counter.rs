use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use rust_maelstrom::Node;

fn main() {}

#[tokio::test]
async fn handler_test() {
    let mut handler = Handler {
        maelstrom_handler: MaelstromHandler,
        peer_handler: todo!()
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
    assert!(matches!(response, Ok(MaelstromResponse::ReadOk(value)) if value == 0));
}

#[tokio::test]
async fn add_test() {
    let mut handler = Handler {
        maelstrom_handler: MaelstromHandler,
        peer_handler: todo!()
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

trait Service<Request> {
    type Response;
    type Future: Future<Output = anyhow::Result<Self::Response>>;

    fn call(&mut self, request: Request) -> Self::Future;
}

#[derive(Clone)]
struct Handler<M, P> where M: Clone, P: Clone {
    maelstrom_handler: M,
    peer_handler: P,
}

impl<M, P> Service<HandlerRequest> for Handler<M, P>
where
    M: Service<RequestArgs, Response = MaelstromResponse> + Clone + 'static,
    P: Clone
{
    type Response = MaelstromResponse;
    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: HandlerRequest) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            match request.request {
                RequestType::MaelstromRequest(req) => this.maelstrom_handler.call(RequestArgs {
                    request: req,
                    node: request.node,
                    id: request.id,
                    input: request.input,
                }),
                RequestType::PeerRequest(_) => todo!()
            }
            .await
        })
    }
}

#[derive(Clone)]
struct MaelstromHandler;
impl Service<RequestArgs> for MaelstromHandler {
    type Response = MaelstromResponse;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;
    fn call(&mut self, request: RequestArgs) -> Self::Future {
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
impl Service<RequestArgs> for PeerHandler {
    type Response = ();

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: RequestArgs) -> Self::Future {
        match request.request {
            MaelstromRequest::Add(_) => todo!(),
            MaelstromRequest::Read => todo!(),
        }
    }
}

struct RequestArgs {
    request: MaelstromRequest,
    node: Arc<Mutex<GNode>>,
    id: usize,
    input: tokio::sync::mpsc::UnboundedReceiver<()>,
}

struct HandlerRequest {
    request: RequestType,
    node: Arc<Mutex<GNode>>,
    id: usize,
    input: tokio::sync::mpsc::UnboundedReceiver<()>,
}

enum RequestType {
    MaelstromRequest(MaelstromRequest),
    PeerRequest(MaelstromRequest)
}
enum MaelstromRequest {
    Add(usize),
    Read,
}

#[derive(Debug)]
enum MaelstromResponse {
    AddOk,
    ReadOk(usize),
}

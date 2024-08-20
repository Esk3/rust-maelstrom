use message::{InitRequest, InitResponse, Message, MessageType, PeerMessage, Request};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, fmt::Debug, future::Future};
use tokio::io::AsyncBufReadExt;

pub mod message;

pub async fn main_loop<N, M, P, R, F, Fut>(handler: F)
where
    N: Node + Send + 'static,
    F: Fn(
            Request<M, P>,
            std::sync::Arc<std::sync::Mutex<N>>,
            usize,
            tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<R>>>,
        ) -> Fut
        + Send
        + Sync
        + 'static
        + Clone,
    Fut: Future + Send + Sync,
    M: DeserializeOwned + Debug + Send + 'static,
    P: DeserializeOwned + Debug + Send + 'static,
    R: DeserializeOwned + Debug + Send + 'static,
{
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    let init_line = lines.next_line().await.unwrap().unwrap();
    let init_message: Message<InitRequest> = serde_json::from_str(&init_line).unwrap();
    let (reply, body) = init_message.into_reply();
    let InitRequest::Init {
        msg_id,
        node_id,
        node_ids,
    } = body;

    let node = Node::init(node_id, node_ids);
    let node = std::sync::Arc::new(std::sync::Mutex::new(node));

    {
        let mut output = std::io::stdout().lock();
        reply
            .with_body(InitResponse::InitOk {
                in_reply_to: msg_id,
            })
            .send(&mut output);
    }

    let mut set = tokio::task::JoinSet::new();
    let mut connections = std::collections::HashMap::new();
    let mut next_id = 1;

    loop {
        tokio::select! {
            Ok(Some(line)) = lines.next_line() => {
                handle_input(&line, node.clone(), &mut set, &mut connections, &mut next_id, handler.clone());
            },
            Some(join_handler) = set.join_next() => {
                let id = join_handler.unwrap();
                connections.remove(&id);
            }
        }
    }
}
struct InputHandler;
impl Service<String> for InputHandler {
    type Response = InputResponse;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: String) -> Self::Future {
        let msg: HashMap<String, serde_json::Value> = serde_json::from_str(&request).unwrap();
        Box::pin(async move { Ok(InputResponse::NewHandler) })
    }
}
enum InputResponse {
    NewHandler,
    HandlerMessage,
}
fn handle_input<N, F, Fut, M, P, R>(
    input: &str,
    state: std::sync::Arc<std::sync::Mutex<N>>,
    set: &mut tokio::task::JoinSet<usize>,
    connections: &mut std::collections::HashMap<
        usize,
        tokio::sync::mpsc::UnboundedSender<Message<PeerMessage<R>>>,
    >,
    next_id: &mut usize,
    handler: F,
) where
    N: Node + Send + 'static,
    F: Fn(
            Request<M, P>,
            std::sync::Arc<std::sync::Mutex<N>>,
            usize,
            tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<R>>>,
        ) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future + Send + Sync,
    M: DeserializeOwned + Debug + Send + 'static,
    P: DeserializeOwned + Debug + Send + 'static,
    R: DeserializeOwned + Debug + Send + 'static,
{
    let message_type: MessageType<M, P, R> = serde_json::from_str(input).unwrap();
    let id = *next_id;
    *next_id += 1;
    match message_type {
        MessageType::Request(message) => {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            connections.insert(id, tx);
            set.spawn(async move {
                handler(message, state, id, rx).await;
                id
            });
        }
        MessageType::Response(message) => {
            let id = message.body.dest.unwrap();
            let Some(tx) = connections.get(&id) else {
                return;
            };
            if tx.send(message).is_err() {
                dbg!("connection lost", &id);
                connections.remove(&id);
            }
        }
    }
}

pub trait Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self;
}

use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};

#[tokio::test]
async fn handler_test() {
    let mut handler = Handler {
        maelstrom_handler: MaelstromHandler,
        peer_handler: todo!(),
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
        peer_handler: todo!(),
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
struct Handler<M, P>
where
    M: Clone,
    P: Clone,
{
    maelstrom_handler: M,
    peer_handler: P,
}

impl<M, P> Service<HandlerRequest> for Handler<M, P>
where
    M: Service<RequestArgs, Response = MaelstromResponse> + Clone + 'static,
    P: Clone,
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
                RequestType::PeerRequest(_) => todo!(),
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
    PeerRequest(MaelstromRequest),
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

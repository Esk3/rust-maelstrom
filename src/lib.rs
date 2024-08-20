use core::panic;
use message::{InitRequest, InitResponse, Message, MessageType, PeerMessage, Request};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, fmt::Debug, future::Future, process::Output};
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

pub async fn main_service_loop<H, P, N>(maelstrom_handler: H, peer_handler: P)
where
    H: Service<RequestArgs<N>, Response = MaelstromResponse> + Clone + 'static,
    P: Service<RequestArgs<N>, Response = PeerResponse> + Clone + 'static,
    N: Node + 'static,
{
    let mut handler = Handler {
        maelstrom_handler,
        peer_handler,
    };
    let node = N::init("first".to_string(), Vec::new());
    let input = HandlerRequest {
        request: RequestType::MaelstromRequest(MaelstromRequest::Read),
        node: Arc::new(Mutex::new(node)),
        id: 1,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let res = handler.call(input).await;
    dbg!(res);
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

pub trait Service<Request> {
    type Response;
    type Future: Future<Output = anyhow::Result<Self::Response>>;

    fn call(&mut self, request: Request) -> Self::Future;
}

pub struct Handler<M, P> {
    pub maelstrom_handler: M,
    pub peer_handler: P,
}

impl<M, P, N> Service<HandlerRequest<N>> for Handler<M, P>
where
    M: Service<RequestArgs<N>, Response = MaelstromResponse> + Clone + 'static,
    P: Service<RequestArgs<N>, Response = PeerResponse> + Clone + 'static,
    N: 'static,
{
    type Response = Message<HandlerResponse<MaelstromResponse, PeerResponse>>;
    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: HandlerRequest<N>) -> Self::Future {
        let RequestType::MaelstromRequest(req) = request.request else {
            panic!();
        };
        let mut f = self.maelstrom_handler.clone();
        Box::pin(async move {
            let res = f
                .call(RequestArgs {
                    request: req,
                    node: request.node,
                    id: request.id,
                    input: request.input,
                })
                .await;
            Ok(Message {
                src: "odo".to_string(),
                dest: "todo".to_string(),
                body: HandlerResponse::Maelstrom(res.unwrap()),
            })
        })
    }
}

#[derive(Debug)]
pub enum HandlerResponse<M, P> {
    Maelstrom(M),
    Peer(P),
}

pub struct RequestArgs<N> {
    pub request: MaelstromRequest,
    pub node: Arc<Mutex<N>>,
    pub id: usize,
    pub input: tokio::sync::mpsc::UnboundedReceiver<()>,
}

pub struct HandlerRequest<N> {
    pub request: RequestType,
    pub node: Arc<Mutex<N>>,
    pub id: usize,
    pub input: tokio::sync::mpsc::UnboundedReceiver<()>,
}

pub enum RequestType {
    MaelstromRequest(MaelstromRequest),
    PeerRequest(MaelstromRequest),
}

pub enum MaelstromRequest {
    Add(usize),
    Read,
}

#[derive(Debug)]
pub enum MaelstromResponse {
    AddOk,
    ReadOk(usize),
}

#[derive(Debug)]
pub enum PeerResponse {}

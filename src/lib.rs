use core::panic;
use message::{InitRequest, InitResponse, Message, MessageType, PeerMessage, Request};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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

pub async fn main_service_loop<H, P, N, Req, Res>(mut handler: Handler<H, P>)
where
    H: Service<RequestArgs<Message<Req>, N>, Response = Res> + Clone + 'static,
    P: Service<RequestArgs<Message<PeerMessage<Req>>, N>, Response = PeerMessage<Res>>
        + Clone
        + 'static,
    N: Node + 'static,
    Req: DeserializeOwned + 'static,
    Res: Serialize + Debug,
{
    let stdin = tokio::io::stdin();
    let test = r#"{"src": "a", "dest": "b", "body": {"type": "init", "msg_id": 1, "node_id": "nnnn1", "node_ids": ["nnnn1"]}}"#;
    let test = std::io::Cursor::new(test);
    let mut lines = tokio::io::BufReader::new(test).lines();

    let init_line = lines.next_line().await.unwrap().unwrap();
    let init_message: Message<InitRequest> = serde_json::from_str(&init_line).unwrap();
    let (reply, body) = init_message.into_reply();
    let InitRequest::Init {
        msg_id,
        node_id,
        node_ids,
    } = body;

    let node = N::init(node_id, node_ids);
    let node = std::sync::Arc::new(std::sync::Mutex::new(node));

    {
        let mut output = std::io::stdout().lock();
        reply
            .with_body(InitResponse::InitOk {
                in_reply_to: msg_id,
            })
            .send(&mut output);
    }

    let req = lines.next_line().await.unwrap().unwrap();
    let req = serde_json::from_str(&req).unwrap();
    let req = HandlerRequest {
        request: req,
        node: node.clone(),
        id: 1,
        input: tokio::sync::mpsc::unbounded_channel().1,
    };
    let res = handler.call(req).await;
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

#[derive(Clone)]
pub struct Handler<M, P> {
    pub maelstrom_handler: M,
    pub peer_handler: P,
}

impl<M> Handler<M, PHander<M>> {
    pub fn new(handler: M) -> Self
    where
        M: Clone,
    {
        Self {
            maelstrom_handler: handler.clone(),
            peer_handler: PHander { inner: handler },
        }
    }
}

#[derive(Clone)]
pub struct PHander<S> {
    pub inner: S,
}
impl<Req, N, S, Res> Service<RequestArgs<Message<PeerMessage<Req>>, N>> for PHander<S>
where
    S: Service<RequestArgs<Message<Req>, N>, Response = Res> + Clone + 'static,
    N: 'static,
    Req: 'static,
{
    type Response = PeerMessage<Res>;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(
        &mut self,
        RequestArgs {
            request: request_1,
            node,
            id,
            input,
        }: RequestArgs<message::Message<PeerMessage<Req>>, N>,
    ) -> Self::Future {
        let mut this = self.clone();
        let (message, peer_message) = request_1.split();
        let (peer_message, body) = peer_message.split();
        Box::pin(async move {
            let response = this
                .inner
                .call(RequestArgs {
                    request: message.with_body(body),
                    node,
                    id,
                    input,
                })
                .await;
            Ok(todo!())
        })
    }
}

impl<M, P, N, Req, Res> Service<HandlerRequest<Req, N>> for Handler<M, P>
where
    M: Service<RequestArgs<Message<Req>, N>, Response = Res> + Clone + 'static,
    P: Service<RequestArgs<Message<PeerMessage<Req>>, N>, Response = PeerMessage<Res>>
        + Clone
        + 'static,
    N: 'static,
    Req: 'static,
    Res: Serialize,
{
    type Response = Message<String>;
    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: HandlerRequest<Req, N>) -> Self::Future {
        let mut this = self.clone();
        match request.request {
            RequestType::Maelstrom(_) => Box::pin(async move {
                let body = this
                    .maelstrom_handler
                    .call(RequestArgs {
                        request: todo!(),
                        node: todo!(),
                        id: todo!(),
                        input: todo!(),
                    })
                    .await;
                Ok(Message {
                    src: todo!(),
                    dest: todo!(),
                    body: serde_json::to_string(&body.unwrap()).unwrap(),
                })
            }),
            RequestType::Peer(req) => Box::pin(async move {
                let body = this
                    .peer_handler
                    .call(RequestArgs {
                        request: req,
                        node: todo!(),
                        id: todo!(),
                        input: todo!(),
                    })
                    .await;
                Ok(Message {
                    src: todo!(),
                    dest: todo!(),
                    body: serde_json::to_string(&body.unwrap()).unwrap(),
                })
            }),
        }
    }
}

#[derive(Debug)]
pub enum HandlerResponse<M, P> {
    Maelstrom(M),
    Peer(P),
}

pub struct RequestArgs<Req, N> {
    pub request: Req,
    pub node: Arc<Mutex<N>>,
    pub id: usize,
    pub input: tokio::sync::mpsc::UnboundedReceiver<()>,
}

pub struct HandlerRequest<Req, N> {
    pub request: RequestType<Req>,
    pub node: Arc<Mutex<N>>,
    pub id: usize,
    pub input: tokio::sync::mpsc::UnboundedReceiver<()>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RequestType<Req> {
    Maelstrom(Message<Req>),
    Peer(Message<PeerMessage<Req>>),
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

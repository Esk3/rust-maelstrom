use handler::{HandlerRequest, HandlerResponse};
use input::{InputHandler, InputResponse};
use message::{InitRequest, InitResponse, Message};
use serde::{de::DeserializeOwned, Serialize};
use service::Service;
use std::{
    collections::HashMap,
    fmt::Debug,
    io::Write,
    sync::{Arc, Mutex},
};

use tokio::io::AsyncBufReadExt;

pub mod handler;
pub mod input;
pub mod message;
pub mod server;
pub mod service;
pub mod error;

#[derive(Debug, Clone)]
pub struct Ids(Arc<Mutex<usize>>);

impl Ids {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(0)))
    }
    #[must_use]
    pub fn next_id(&self) -> usize {
        let mut lock = self.0.lock().unwrap();
        *lock += 1;
        *lock
    }
}

pub struct MainLoop<I, R, E> {
    pub input_handler: I,
    pub request_handler: R,
    pub event_handler: Option<E>,
}

impl<R, Req, Res> MainLoop<InputHandler<Req, Res>, R, ()> {
    pub fn new(request_handler: R) -> Self {
        Self {
            input_handler: InputHandler::new(),
            request_handler,
            event_handler: None,
        }
    }
}

impl<T> service::Service<Event<T>> for ()
where
    T: Send + 'static,
{
    type Response = Event<T>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Event<T>) -> Self::Future {
        Box::pin(async move { Ok(request) })
    }
}

impl<I, R, E> MainLoop<I, R, E> {
    pub async fn run<N, Req, Res>(self)
    where
        R: Service<HandlerRequest<Req, Res, N>, Response = Message<HandlerResponse<Res>>>
            + Clone
            + 'static
            + Send,
        I: Service<String, Response = InputResponse<Req, Res>>,
        E: Service<Event<Req>, Response = Event<Req>>,
        N: Node + 'static + Debug + Send,
        Req: DeserializeOwned + 'static + Debug + Send,
        Res: Serialize + DeserializeOwned + Debug + Send + 'static + Debug,
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

        let node = N::init(node_id, node_ids);
        let node = std::sync::Arc::new(std::sync::Mutex::new(node));

        {
            let mut output = std::io::stdout().lock();
            reply
                .with_body(dbg!(InitResponse::InitOk {
                    in_reply_to: msg_id,
                }))
                .send(&mut output);
            output.flush().unwrap();
        }

        let mut input_handler = self.input_handler;
        let mut id = 0;
        let mut set = tokio::task::JoinSet::new();
        let mut channels = HashMap::new();
        loop {
            tokio::select! {
                line = lines.next_line() => {
                    let line = line.unwrap().unwrap();
                    let input = input_handler.call(line).await.unwrap();
                    match input {
                        InputResponse::NewHandler(request) => {
                            id += 1;
                            let (rx, tx) = tokio::sync::mpsc::unbounded_channel();
                            channels.insert(id, rx);
                            let handler_request = HandlerRequest {
                                request,
                                node: node.clone(),
                                id,
                                input: tx,
                            };
                            let mut handler = self.request_handler.clone();
                            set.spawn(async move {
                                let response = handler.call(handler_request).await.unwrap();
                                (id, response)
                            });
                        }
                        InputResponse::HandlerMessage { id, message } => if let Some(rx) = channels.get(&id) {
                            rx.send(message).unwrap();
                        } else {
                            dbg!("channel closed", id);
                        },
                    }
                },
                Some(handler) = set.join_next() => {
                    let (id, response) = handler.unwrap();
                    channels.remove(&id);
                    response.send(std::io::stdout().lock());
                }
            }
        }
    }
}

pub type Fut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>;

pub trait Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self;
}

pub struct EventMiddleware {}
impl EventMiddleware {
    fn call(&mut self, request: Event<()>) -> Event<()> {
        request
    }
}

pub struct EventHandler<T> {
    subscribers: HashMap<usize, tokio::sync::oneshot::Sender<Message<T>>>,
}

impl<T> EventHandler<T>
where
    T: Debug,
{
    pub fn new() -> Self {
        todo!()
    }
    pub fn subscribe(&mut self) {}
    pub fn unsubscribe(&mut self) {}
    pub fn handle_event(&mut self, event: Event<T>) -> Option<Event<T>> {
        match event {
            Event::NetworkMessage(_) => Some(event),
            Event::Internal { id, message } => {
                if let Some(consumer) = self.subscribers.remove(&id) {
                    consumer.send(message).unwrap();
                    None
                } else {
                    todo!(
                        "return event, problem is checking if id exsist in subs without moving msg"
                    )
                }
            }
        }
    }
}

pub enum Event<T> {
    NetworkMessage(Message<T>),
    Internal { id: usize, message: Message<T> },
}

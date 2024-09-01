use std::{collections::HashMap, fmt::Debug, future::Future, pin::Pin};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    message::{Message, MessageType, PeerMessage, Request},
    service::Service,
    Node, RequestType,
};

pub struct InputHandler<Req, Res>(std::marker::PhantomData<Req>, std::marker::PhantomData<Res>);

impl<Req, Res> InputHandler<Req, Res> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData, std::marker::PhantomData)
    }
}

impl<Req, Res> Default for InputHandler<Req, Res> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Req, Res> Service<String> for InputHandler<Req, Res>
where
    Req: DeserializeOwned + 'static + Send,
    Res: DeserializeOwned + 'static + Send,
{
    type Response = InputResponse<Req, Res>;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>> + Send>>;

    fn call(&mut self, request: String) -> Self::Future {
        let input: InputType<Req, Res> = serde_json::from_str(&request).unwrap();
        match input {
            InputType::Request(req) => Box::pin(async move { Ok(InputResponse::NewHandler(req)) }),
            InputType::Response(res) => {
                let id = res.body.dest.unwrap();
                Box::pin(async move { Ok(InputResponse::HandlerMessage { id, message: res }) })
            }
       }
    }
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum InputType<Req, Res> {
    Request(RequestType<Req>),
    Response(Message<PeerMessage<Res>>),
}
#[derive(Debug)]
pub enum InputResponse<Req, Res> {
    NewHandler(RequestType<Req>),
    HandlerMessage {
        id: usize,
        message: Message<PeerMessage<Res>>,
    },
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

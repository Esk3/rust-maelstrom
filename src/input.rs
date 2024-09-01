use std::{collections::HashMap, fmt::Debug, future::Future, pin::Pin};

use serde::de::DeserializeOwned;

use crate::{
    message::{Message, MessageType, PeerMessage, Request},
    service::Service,
    Node,
};

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

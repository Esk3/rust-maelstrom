use std::{future::Future, pin::Pin};

use rust_maelstrom::{
    main_loop, message::{Message, PeerMessage, Request}, service::Service, Node
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    main_loop(rust_maelstrom::handler::Handler::new(Handler)).await;
}

#[derive(Clone)]
pub struct Handler;
impl Service<rust_maelstrom::RequestArgs<Message<()>, EchoNode>> for Handler {
    type Response = ();

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(&mut self, request: rust_maelstrom::RequestArgs<Message<()>, EchoNode>) -> Self::Future {
        todo!()
    }
}

#[derive(Debug)]
struct EchoNode {
    pub _id: String,
}

impl Node for EchoNode {
    fn init(node_id: String, _node_ids: Vec<String>) -> Self {
        Self { _id: node_id }
    }
}

async fn handle_message(
    message: Request<MessageRequest, ()>,
    _node: std::sync::Arc<std::sync::Mutex<EchoNode>>,
    _: usize,
    _: tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<()>>>,
) {
    match message {
        Request::Maelstrom(message) => {
            let (reply, body) = message.into_reply();
            match body {
                MessageRequest::Echo { echo, msg_id } => {
                    let body = MessageResponse::EchoOk {
                        echo,
                        in_reply_to: msg_id,
                    };
                    reply.with_body(body).send(&mut std::io::stdout().lock());
                }
            }
        }
        Request::Peer(message) => {
            let (_reply, _body) = message.into_reply();
            unimplemented!()
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageRequest {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageResponse {
    EchoOk {
        echo: serde_json::Value,
        in_reply_to: usize,
    },
}

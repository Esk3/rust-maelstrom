use std::{future::Future, pin::Pin};

use rust_maelstrom::{main_loop, message::Message, service::Service, Node};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    main_loop(rust_maelstrom::handler::Handler::new(Handler)).await;
}

#[derive(Clone)]
pub struct Handler;
impl Service<rust_maelstrom::RequestArgs<Message<MessageRequest>, MessageResponse, EchoNode>>
    for Handler
{
    type Response = MessageResponse;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(
        &mut self,
        rust_maelstrom::RequestArgs { request, .. }: rust_maelstrom::RequestArgs<
            Message<MessageRequest>,
            MessageResponse,
            EchoNode,
        >,
    ) -> Self::Future {
        match request.body {
            MessageRequest::Echo { echo, msg_id } => Box::pin(async move {
                Ok(MessageResponse::EchoOk {
                    echo,
                    in_reply_to: msg_id,
                })
            }),
        }
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageRequest {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageResponse {
    EchoOk {
        echo: serde_json::Value,
        in_reply_to: usize,
    },
}

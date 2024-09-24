use rust_maelstrom::{event::EventId, message::Message, service::Service, Fut, Node};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let s = rust_maelstrom::server::Server::new(Handler);
    s.run().await?;
    Ok(())
}

#[derive(Clone)]
pub struct Handler;
impl Service<rust_maelstrom::server::HandlerInput<MessageRequest, EchoNode>> for Handler {
    type Response =
        rust_maelstrom::server::HandlerResponse<Message<MessageResponse>, MessageRequest>;

    type Future = Fut<Self::Response>;

    fn call(
        &mut self,
        request: rust_maelstrom::server::HandlerInput<MessageRequest, EchoNode>,
    ) -> Self::Future {
        let (msg, body) = request.message.into_reply();
        match body {
            MessageRequest::Echo { echo, msg_id } => Box::pin(async move {
                Ok(rust_maelstrom::server::HandlerResponse::Response(
                    msg.with_body(MessageResponse::EchoOk {
                        echo,
                        in_reply_to: msg_id,
                    }),
                ))
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageRequest {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}

impl EventId for MessageRequest {
    fn get_event_id(&self) -> usize {
        todo!()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageResponse {
    EchoOk {
        echo: serde_json::Value,
        in_reply_to: usize,
    },
}

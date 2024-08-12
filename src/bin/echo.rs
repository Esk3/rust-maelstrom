use rust_maelstrom::{
    main_loop,
    message::{Message, PeerMessage, Request},
    Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    main_loop(handle_message).await
}

struct EchoNode {
    pub id: String,
}

impl Node for EchoNode {
    fn init(node_id: String, node_ids: Vec<String>) -> Self {
        Self { id: node_id }
    }
}

async fn handle_message(
    message: Request<MessageRequest, ()>,
    node: std::sync::Arc<std::sync::Mutex<EchoNode>>,
    id: usize,
    mut input: tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<()>>>,
) {
    match message {
        Request::Maelstrom(message) => {
            dbg!("handing message: ", &message);
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
            let (reply, body) = message.into_reply();
            match body.body {
                () => unimplemented!(),
            }
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

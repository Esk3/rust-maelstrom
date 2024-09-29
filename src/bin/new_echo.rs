use rust_maelstrom::{new_event::IntoBodyId, node::NodeResponse, node_handler::NodeHandler};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    let handler = NodeHandler::<Request, Response, EchoNode, _>::init()
        .await
        .unwrap();
    handler.run().await;
}

#[derive(Debug, Clone)]
struct EchoNode {}
impl rust_maelstrom::node::Node<Request, Response> for EchoNode {
    fn init(node_id: String, node_ids: Vec<String>, state: ()) -> Self {
        Self {}
    }

    fn handle_message_recived(
        &self,
        message: rust_maelstrom::message::Message<Request>,
        _event_handler: rust_maelstrom::new_event::EventHandler<Request, Response, usize>,
    ) -> rust_maelstrom::Fut<rust_maelstrom::node::NodeResponse<Request, Response>> {
        let (message, Request::Echo { echo, msg_id }) = message.split();
        Box::pin(async move {
            Ok(NodeResponse::Reply(message.with_body(Response::EchoOk {
                echo,
                in_reply_to: msg_id,
            })))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    EchoOk {
        echo: serde_json::Value,
        in_reply_to: usize,
    },
}

impl IntoBodyId<usize> for Request {
    fn into_body_id(self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Request::Echo { echo: _, msg_id } => rust_maelstrom::new_event::BodyId(msg_id),
        }
    }

    fn clone_into_body_id(&self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Request::Echo { echo: _, msg_id } => rust_maelstrom::new_event::BodyId(*msg_id),
        }
    }
}

impl IntoBodyId<usize> for Response {
    fn into_body_id(self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Response::EchoOk { echo: _, in_reply_to } => rust_maelstrom::new_event::BodyId(in_reply_to),
        }
    }

    fn clone_into_body_id(&self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Response::EchoOk { echo: _, in_reply_to } => rust_maelstrom::new_event::BodyId(*in_reply_to),
        }
    }
}

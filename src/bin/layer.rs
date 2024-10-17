use rust_maelstrom::{
    message::Message,
    node_handler::NodeHandler,
    service::{json::JsonLayer, Service},
    Fut, Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    let node = NodeHandler::<()>::init_node::<EchoNode, ()>(()).await;
    let service = JsonLayer::<_, Message<EchoRequest>>::new(node);
    let handler = NodeHandler::new(service);
    handler.run().await;
}

#[derive(Clone)]
struct EchoNode;

impl Node for EchoNode {
    fn init(_node_id: String, _node_ids: Vec<String>, _state: ()) -> Self {
        Self
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EchoRequest {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EchoResponse {
    EchoOk {
        echo: serde_json::Value,
        in_reply_to: usize,
    },
}

impl Service<Message<EchoRequest>> for EchoNode {
    type Response = Option<Message<EchoResponse>>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<EchoRequest>) -> Self::Future {
        Box::pin(async move {
            let (response, body) = request.into_reply();
            match body {
                EchoRequest::Echo { echo, msg_id } => {
                    Ok(Some(response.with_body(EchoResponse::EchoOk {
                        echo,
                        in_reply_to: msg_id,
                    })))
                }
            }
        })
    }
}

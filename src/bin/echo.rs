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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EchoRequest {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rust_maelstrom::message::{InitRequest, InitResponse, Request};

    use super::*;
    #[tokio::test]
    async fn test() {
        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = std::io::Cursor::new(Vec::new());
        send_init_msg(&mut input);
        input.set_position(0);
        let node =
            NodeHandler::<()>::init_node_with_io::<EchoNode, _, _, ()>(input, &mut output, ())
                .await;
        assert_init_response(output.into_inner());
        let service = JsonLayer::<_, Message<EchoRequest>>::new(node);
        let handler = NodeHandler::new(service);

        let mut input = std::io::Cursor::new(Vec::new());
        let output = std::io::Cursor::new(Vec::new());
        let output = std::sync::Arc::new(std::sync::Mutex::new(output));

        Message {
            src: "c".to_string(),
            dest: "node".to_string(),
            body: EchoRequest::Echo {
                echo: serde_json::Value::String("my msg".to_string()),
                msg_id: 1,
            },
        }
        .send(&mut input)
        .unwrap();

        input.set_position(0);
        handler.run_with_io(input, output.clone()).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let msg = Message {
            src: "node".to_string(),
            dest: "c".to_string(),
            body: EchoResponse::EchoOk {
                echo: serde_json::Value::from_str("my msg").unwrap(),
                in_reply_to: 1,
            },
        };
        dbg!(String::from_utf8(
            output.lock().unwrap().clone().into_inner()
        ));
        panic!()
    }

    fn send_init_msg<W>(w: W)
    where
        W: std::io::Write,
    {
        let message = Message {
            src: "test".to_string(),
            dest: "node".to_string(),
            body: InitRequest::Init {
                msg_id: 0,
                node_id: "node".to_string(),
                node_ids: vec!["node".to_string()],
            },
        };
        message.send(w).unwrap();
    }

    fn assert_init_response(bytes: Vec<u8>) {
        let s = String::from_utf8(bytes).unwrap();
        let res: Message<InitResponse> = serde_json::from_str(&s).unwrap();
        assert_eq!(
            res,
            Message {
                src: "node".to_string(),
                dest: "test".to_string(),
                body: InitResponse::InitOk { in_reply_to: 0 }
            }
        );
    }
}

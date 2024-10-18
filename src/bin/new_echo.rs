//use rust_maelstrom::{new_event::IntoBodyId, node::NodeResponse, node_handler::NodeHandler};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    //let handler = NodeHandler::<Request, Response, EchoNode, _>::init()
    //    .await
    //    .unwrap();
    //handler.run().await;
}
//
//#[derive(Debug, Clone)]
//struct EchoNode;
//impl rust_maelstrom::node::Node<Request, Response> for EchoNode {
//    fn init(_node_id: String, _node_ids: Vec<String>, _state: ()) -> Self {
//        Self
//    }
//
//    fn handle_message_recived(
//        self,
//        message: rust_maelstrom::message::Message<Request>,
//        _event_handler: rust_maelstrom::new_event::EventHandler<Request, Response, usize>,
//    ) -> rust_maelstrom::Fut<rust_maelstrom::node::NodeResponse<Request, Response>> {
//        let (message, Request::Echo { echo, msg_id }) = message.split();
//        Box::pin(async move {
//            Ok(NodeResponse::Reply(message.with_body(Response::EchoOk {
//                echo,
//                in_reply_to: msg_id,
//            })))
//        })
//    }
//}
//
//#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
//#[serde(tag = "type", rename_all = "snake_case")]
//enum Request {
//    Echo {
//        echo: serde_json::Value,
//        msg_id: usize,
//    },
//}
//#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
//#[serde(tag = "type", rename_all = "snake_case")]
//enum Response {
//    EchoOk {
//        echo: serde_json::Value,
//        in_reply_to: usize,
//    },
//}
//
//impl IntoBodyId<usize> for Request {
//    fn into_body_id(self) -> rust_maelstrom::new_event::BodyId<usize> {
//        match self {
//            Request::Echo { echo: _, msg_id } => rust_maelstrom::new_event::BodyId(msg_id),
//        }
//    }
//
//    fn clone_into_body_id(&self) -> rust_maelstrom::new_event::BodyId<usize> {
//        match self {
//            Request::Echo { echo: _, msg_id } => rust_maelstrom::new_event::BodyId(*msg_id),
//        }
//    }
//}
//
//impl IntoBodyId<usize> for Response {
//    fn into_body_id(self) -> rust_maelstrom::new_event::BodyId<usize> {
//        match self {
//            Response::EchoOk {
//                echo: _,
//                in_reply_to,
//            } => rust_maelstrom::new_event::BodyId(in_reply_to),
//        }
//    }
//
//    fn clone_into_body_id(&self) -> rust_maelstrom::new_event::BodyId<usize> {
//        match self {
//            Response::EchoOk {
//                echo: _,
//                in_reply_to,
//            } => rust_maelstrom::new_event::BodyId(*in_reply_to),
//        }
//    }
//}
//
//#[cfg(test)]
//mod tests {
//    use std::str::FromStr;
//
//    use rust_maelstrom::message::{InitResponse, Message};
//
//    use super::*;
//    #[must_use]
//    pub fn create_init_line() -> Message<rust_maelstrom::message::InitRequest> {
//        Message {
//            src: "init_node".to_string(),
//            dest: "test_node_from_init".to_string(),
//            body: rust_maelstrom::message::InitRequest::Init {
//                msg_id: 123,
//                node_id: "init_msg_node_id".to_string(),
//                node_ids: Vec::new(),
//            },
//        }
//    }
//
//    #[tokio::test]
//    async fn echo_node_test() {
//        let mut input = std::io::Cursor::new(Vec::new());
//        let mut output = std::io::Cursor::new(Vec::new());
//
//        let init_line = create_init_line();
//        init_line.send(&mut input).unwrap();
//        input.set_position(0);
//
//        let node_handler =
//            NodeHandler::<Request, Response, EchoNode, _>::init_with_io(input, &mut output)
//                .await
//                .unwrap();
//
//        output.set_position(0);
//        let init_response: Message<InitResponse> = serde_json::from_reader(output).unwrap();
//        assert_eq!(
//            init_response,
//            Message {
//                src: "test_node_from_init".to_string(),
//                dest: "init_node".to_string(),
//                body: InitResponse::InitOk { in_reply_to: 123 }
//            }
//        );
//
//        let mut input = std::io::Cursor::new(Vec::new());
//        let message = Message {
//            src: "test".to_string(),
//            dest: "node".to_string(),
//            body: Request::Echo {
//                echo: serde_json::Value::String("the_ehoc_value".to_string()),
//                msg_id: 1,
//            },
//        };
//        message.send(&mut input).unwrap();
//        input.set_position(0);
//
//        let mut output = std::io::Cursor::new(Vec::new());
//
//        let _ = tokio::time::timeout(
//            std::time::Duration::from_millis(10),
//            node_handler.run_with_io(input, &mut output),
//        )
//        .await;
//
//        output.set_position(0);
//        let response: Message<Response> = serde_json::from_reader(output).unwrap();
//
//        assert_eq!(
//            response,
//            Message {
//                src: "node".to_string(),
//                dest: "test".to_string(),
//                body: Response::EchoOk {
//                    echo: serde_json::Value::String("the_ehoc_value".to_string()),
//                    in_reply_to: 1
//                }
//            }
//        );
//    }
//}

use std::fmt::Debug;

use serde::Serialize;

use crate::{error::Error, message::Message, new_event::Event, Fut};

pub trait Node<I, T, S = (), NId = String> {
    fn init(node_id: NId, node_ids: Vec<NId>, state: S) -> Self;
    fn on_event<W>(
        &self,
        event: Event<I, T>,
        event_handler: crate::new_event::EventHandler<I, T, usize>,
        writer: W
    ) where
        T: std::marker::Sync + Serialize + Debug + Send + 'static + Clone,
        I: std::marker::Send + 'static + Debug,
        Self: std::marker::Send + std::marker::Sync + Clone + 'static,
    {
        match event {
            Event::MessageRecived(message) => {
                let this = self.clone();
                let x = tokio::spawn(async move {
                    dbg!("handing msg");
                    let response = this.handle_message_recived(message, event_handler).await;
                    panic!("got response {response:?}");
                    match response {
                        Ok(NodeResponse::Event(e)) => todo!("got event {e:?}"),
                        Ok(NodeResponse::Message(message)) => message
                            .send(std::io::stdout().lock())
                            .expect("failed to write to stdout"),
                        Ok(NodeResponse::Reply(message)) => {
                            let (reply, body) = message.into_reply();
                            reply
                                .with_body(body)
                                .send(std::io::stdout().lock())
                                .expect("failed to write to stdout");
                        }
                        Ok(NodeResponse::Error(e)) => todo!("got node error: {e:?}"),
                        Ok(NodeResponse::None) => (),
                        Err(e) => todo!("got err: {e:?}"),
                    };
                });
            }
            Event::MessageSent(_) => todo!(),
            Event::Error(_) => todo!(),
        }
    }
    fn handle_message_recived(
        &self,
        message: Message<I>,
        event_handler: crate::new_event::EventHandler<I, T, usize>,
    ) -> Fut<NodeResponse<I, T>>;
}

#[derive(Debug, Clone)]
pub enum NodeResponse<I, T> {
    Event(Event<I, T>),
    Message(Message<T>),
    Reply(Message<T>),
    Error(Error),
    None,
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use crate::message::{InitRequest, InitResponse};

    use super::*;
    #[derive(Debug, Clone)]
    struct TestNode;
    #[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Deserialize)]
    enum A {
        C,
    }
    impl crate::new_event::IntoBodyId<usize> for A {
        fn into_body_id(self) -> crate::new_event::BodyId<usize> {
            todo!()
        }

        fn clone_into_body_id(&self) -> crate::new_event::BodyId<usize> {
            todo!()
        }
    }

    #[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize)]
    enum B {
        D,
    }
    impl crate::new_event::IntoBodyId<usize> for B {
        fn into_body_id(self) -> crate::new_event::BodyId<usize> {
            todo!()
        }

        fn clone_into_body_id(&self) -> crate::new_event::BodyId<usize> {
            todo!()
        }
    }

    impl Node<A, B> for TestNode {
        fn init(node_id: String, node_ids: Vec<String>, state: ()) -> Self {
            Self
        }

        fn handle_message_recived(
            &self,
            message: Message<A>,
            event_handler: crate::new_event::EventHandler<A, B, usize>,
        ) -> Fut<NodeResponse<A, B>> {
            Box::pin(async move { Ok(NodeResponse::None) })
        }
    }

    #[tokio::test]
    async fn node_test() {
        let node = TestNode::init("testnode".to_string(), Vec::new(), ());
        let msg = Message {
            src: "tester".to_string(),
            dest: "testnode".to_string(),
            body: A::C,
        };
        let event_handler = crate::new_event::EventHandler::new();
        let my_event = Event::MessageRecived(msg.clone());
        node.on_event(my_event, event_handler.clone());

        let _ = node.handle_message_recived(msg, event_handler).await;
    }

    #[tokio::test]
    async fn node_handler_test() {
        let init_line = Message {
            src: "init_node".to_string(),
            dest: "test_node_from_init".to_string(),
            body: InitRequest::Init {
                msg_id: 123,
                node_id: "init_msg_node_id".to_string(),
                node_ids: Vec::new(),
            },
        };
        let mut input = std::io::Cursor::new(Vec::new());
        init_line.send(&mut input).unwrap();
        input.write_all(b"\n").unwrap();
        input.set_position(0);

        let mut output = std::io::Cursor::new(Vec::new());

        let handler =
            crate::node_handler::NodeHandler::<A, B, TestNode, _>::init_with_io(input, &mut output)
                .await
                .unwrap();
        output.set_position(0);
        let response = serde_json::from_reader::<_, Message<InitResponse>>(&mut output).unwrap();

        assert_eq!(
            response,
            Message {
                src: "test_node_from_init".to_string(),
                dest: "init_node".to_string(),
                body: InitResponse::InitOk { in_reply_to: 123 }
            }
        );

        let mut input = std::io::Cursor::new(Vec::new());
        let mut output = std::io::Cursor::new(Vec::new());

        handler.run_with_io(input, &mut output);
    }
}

use std::fmt::Debug;

use serde::Serialize;

use crate::{error::Error, message::Message, new_event::Event, Fut};

pub trait Node<I, T, S = (), NId = String> {
    fn init(node_id: NId, node_ids: Vec<NId>, state: S) -> Self;
    fn on_event(
        &self,
        event: Event<I, T>,
        event_handler: crate::new_event::EventHandler<I, T, usize>,
    ) -> tokio::task::JoinHandle<anyhow::Result<NodeResponse<I, T>>>
    where
        T: std::marker::Sync + Serialize + Debug + Send + 'static + Clone,
        I: std::marker::Send + 'static + Debug,
        Self: std::marker::Send + std::marker::Sync + Clone + 'static,
    {
        match event {
            Event::MessageRecived(message) => {
                let this = self.clone();
                tokio::spawn(
                    async move { this.handle_message_recived(message, event_handler).await },
                )
            }
            Event::MessageSent(_) => todo!(),
            Event::Error(_) => todo!(),
        }
    }
    fn handle_message_recived(
        self,
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
pub mod tests {
    use std::io::Write;

    use crate::message::{InitRequest, InitResponse};

    use super::*;
    #[derive(Debug, Clone)]
    struct TestNode;
    #[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
    enum A {
        C,
    }
    impl crate::new_event::IntoBodyId<usize> for A {
        fn into_body_id(self) -> crate::new_event::BodyId<usize> {
            crate::new_event::BodyId(1)
        }

        fn clone_into_body_id(&self) -> crate::new_event::BodyId<usize> {
            crate::new_event::BodyId(1)
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
            mut self,
            message: Message<A>,
            event_handler: crate::new_event::EventHandler<A, B, usize>,
        ) -> Fut<NodeResponse<A, B>> {
            Box::pin(async move {
                Ok(NodeResponse::Message(Message {
                    src: "set_test_src".to_string(),
                    dest: "test_dest".to_string(),
                    body: B::D,
                }))
            })
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
        let join_handle = node.on_event(my_event, event_handler.clone());

        let _ = dbg!(node.handle_message_recived(msg, event_handler).await);
    }

    #[must_use]
    pub fn create_init_line() -> Message<InitRequest> {
        Message {
            src: "init_node".to_string(),
            dest: "test_node_from_init".to_string(),
            body: InitRequest::Init {
                msg_id: 123,
                node_id: "init_msg_node_id".to_string(),
                node_ids: Vec::new(),
            },
        }
    }

    #[tokio::test]
    async fn node_handler_test() {
        let init_line = create_init_line();
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

        let message = Message {
            src: "do_something".to_string(),
            dest: "the_node".to_string(),
            body: A::C,
        };

        let mut input = std::io::Cursor::new(Vec::new());
        message.send(&mut input).unwrap();
        let mut output = std::io::Cursor::new(Vec::new());

        input.set_position(0);
        // handler.run_with_io(input, &mut output).await;
        dbg!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                handler.run_with_io(input, &mut output)
            )
            .await
        );
        output.set_position(0);
        // dbg!(output);
        let res: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_reader(output).unwrap();
        dbg!(res);
    }
}

use std::fmt::Debug;

use serde::Serialize;

use crate::{error::Error, message::Message, new_event::Event, Fut};

pub trait Node<I, T, S = (), NId = String> {
    fn init(node_id: NId, node_ids: Vec<NId>, state: S) -> Self;
    fn on_event(&self, event: Event<I, T>)
    where
        T: std::marker::Sync + Serialize + Debug,
        I: std::marker::Send + 'static + Debug,
        Self: std::marker::Send + std::marker::Sync + Clone + 'static,
    {
        match event {
            Event::MessageRecived(message) => {
                let this = self.clone();
                let x = tokio::spawn(async move {
                    dbg!("handing msg");
                    let response = this.handle_message_recived(message).await;
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
    fn handle_message_recived(&self, message: Message<I>) -> Fut<NodeResponse<I, T>>;
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
    use super::*;
    #[derive(Debug, Clone)]
    struct TestNode;
    #[derive(Debug, Clone)]
    enum A {
        C,
    }
    #[derive(Debug, Clone, Serialize)]
    enum B {
        D,
    }
    impl Node<A, B> for TestNode {
        fn init(node_id: String, node_ids: Vec<String>, state: ()) -> Self {
            Self
        }

        fn handle_message_recived(&self, message: Message<A>) -> Fut<NodeResponse<A, B>> {
            Box::pin(async move {
                Ok(NodeResponse::None)
            })
        }
    }

    #[tokio::test]
    async fn test() {
        let node = TestNode::init("testnode".to_string(), Vec::new(), ());
        let msg = Message { src: "tester".to_string(), dest: "testnode".to_string(), body: A::C };
        let my_event = Event::MessageRecived(msg.clone());
        node.on_event(my_event);

        let x = node.handle_message_recived(msg).await;
        panic!("{x:?}");
    }
}

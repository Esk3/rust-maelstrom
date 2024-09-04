use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;

use crate::{
    message::{InitRequest, Message},
    service::Service,
    Fut, Node,
};

pub struct Server<H> {
    handler: H,
}

impl<H> Server<H> {
    pub async fn run<T, Res, N>(mut self)
    where
        H: crate::service::Service<HandlerInput<T, N>, Response = HandlerResponse<Message<Res>, T>>
            + Send
            + 'static,
        T: Serialize + DeserializeOwned + Send + 'static + Debug + Clone,
        Res: Serialize + Send + 'static,
        N: Node + Send + 'static,
    {
        let node = Self::init().await;
        let node = std::sync::Arc::new(std::sync::Mutex::new(node));
        let event_broker = EventBroker::new();

        let input = std::io::stdin().lines().next().unwrap().unwrap();

        let input = serde_json::from_str::<Message<T>>(&input).unwrap();

        let mut set = tokio::task::JoinSet::new();

        let input = HandlerInput {
            message: input,
            node: node.clone(),
            event_broker: event_broker.clone(),
        };

        set.spawn(async move { self.handler.call(input).await });

        let result = set.join_next().await;
        match result.unwrap().unwrap().unwrap() {
            HandlerResponse::Response(res) => res.send(std::io::stdout()),
            HandlerResponse::Event(event) => event_broker.publish_event(event),
        }
    }
    async fn init<N>() -> N
    where
        N: Node,
    {
        let stdin = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(stdin).lines();

        let init_line = lines.next_line().await.unwrap().unwrap();
        let init_message: Message<InitRequest> = serde_json::from_str(&init_line).unwrap();
        let (reply, body) = init_message.into_reply();
        let InitRequest::Init {
            msg_id,
            node_id,
            node_ids,
        } = body;

        let node = N::init(node_id, node_ids);

        {
            let mut output = std::io::stdout().lock();
            reply
                .with_body(dbg!(crate::message::InitResponse::InitOk {
                    in_reply_to: msg_id,
                }))
                .send(&mut output);
            std::io::Write::flush(&mut output).unwrap();
        }
        node
    }
}

#[derive(Debug, Deserialize)]
pub enum Event<T> {
    Maelstrom { dest: usize, body: Message<T> },
    Injected { dest: usize, body: Message<T> },
}

pub enum HandlerResponse<Res, T> {
    Response(Res),
    Event(Event<T>),
}
#[derive(Debug, Clone)]
struct EventBroker<T> {
    new_ids_tx:
        tokio::sync::mpsc::UnboundedSender<(usize, tokio::sync::oneshot::Sender<Message<T>>)>,
    events_tx: tokio::sync::mpsc::UnboundedSender<Event<T>>,
}
impl<T> EventBroker<T>
where
    T: Debug + Send + 'static,
{
    pub fn new() -> Self {
        let (new_ids_tx, mut new_ids_rx) = tokio::sync::mpsc::unbounded_channel();
        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut subscribers = HashMap::<usize, tokio::sync::oneshot::Sender<Message<T>>>::new();
        tokio::spawn(async move {
            let (id, tx) = new_ids_rx.recv().await.unwrap();
            subscribers.insert(id, tx);

            let event = events_rx.recv().await.unwrap();
            let (id, body) = match event {
                Event::Maelstrom { dest, body } | Event::Injected { dest, body } => (dest, body),
            };
            let Some(tx) = subscribers.remove(&id) else {
                return;
            };
            tx.send(body).unwrap();
        });
        Self {
            new_ids_tx,
            events_tx,
        }
    }
    pub fn subscribe(&self, id: usize) -> tokio::sync::oneshot::Receiver<Message<T>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.new_ids_tx.send((id, tx)).unwrap();
        rx
    }
    pub fn publish_event(&self, event: Event<T>) {
        self.events_tx.send(event).unwrap();
    }
}

struct TestHandler;
impl Service<HandlerInput<MyT, MyNode>> for TestHandler {
    type Response = HandlerResponse<Message<()>, MyT>;

    type Future = Fut<Self::Response>;

    fn call(
        &mut self,
        HandlerInput {
            message,
            node,
            event_broker,
        }: HandlerInput<MyT, MyNode>,
    ) -> Self::Future {
        let (msg, body) = message.split();
        match body {
            MyT::Echo { echo } => todo!(),
            MyT::Broadcast {} => todo!(),
            MyT::BroadcastOk { msg_id } => HandlerResponse::<(), _>::Event(Event::Maelstrom {
                dest: msg_id,
                body: msg.with_body(MyT::BroadcastOk { msg_id }),
            }),
        };
        todo!()
    }
}
struct MyNode;
impl Node for MyNode {
    fn init(node_id: String, node_ids: Vec<String>) -> Self {
        todo!()
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum MyT {
    Echo { echo: String },
    Broadcast {},
    BroadcastOk { msg_id: usize },
}
async fn test() {
    let s = Server {
        handler: TestHandler,
    };
    s.run().await;
}
pub struct HandlerInput<T, N> {
    message: Message<T>,
    node: Arc<Mutex<N>>,
    event_broker: EventBroker<T>,
}

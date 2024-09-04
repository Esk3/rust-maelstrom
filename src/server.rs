use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{message::Message, service::Service, Fut};

pub struct Server<H> {
    handler: H,
}

impl<H> Server<H> {
    pub async fn run<T, Res>(mut self)
    where
        H: crate::service::Service<Message<T>, Response = HandlerResponse<Message<Res>, T>>
            + Send
            + 'static,
        T: Serialize + DeserializeOwned + Send + 'static + Debug,
        Res: Serialize + Send + 'static,
    {
        Self::init();
        let event_broker = EventBroker::new();

        let input = std::io::stdin().lines().next().unwrap().unwrap();

        let input = serde_json::from_str::<Message<T>>(&input).unwrap();

        let mut set = tokio::task::JoinSet::new();

        set.spawn(async move { self.handler.call(input).await });

        let result = set.join_next().await;
        match result.unwrap().unwrap().unwrap() {
            HandlerResponse::Response(res) => res.send(std::io::stdout()),
            HandlerResponse::Event(event) => event_broker.publish_event(event),
        }
    }
    fn init() {}
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
        self.new_ids_tx.send((id, tx));
        rx
    }
    pub fn publish_event(&self, event: Event<T>) {
        self.events_tx.send(event);
    }
}

struct TestHandler;
impl Service<Message<MyT>> for TestHandler {
    type Response = HandlerResponse<Message<()>, MyT>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<MyT>) -> Self::Future {
        let (msg, body) = request.split();
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
#[derive(Debug, Serialize, Deserialize)]
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

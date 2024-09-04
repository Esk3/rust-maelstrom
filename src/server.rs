use serde::{de::DeserializeOwned, Deserialize};

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
        T: DeserializeOwned + Send + 'static,
        Res: Send + 'static,
    {
        Self::init();
        let mut event_broker = EventBroker::new();

        let input = std::io::stdin().lines().next().unwrap().unwrap();

        let input = serde_json::from_str::<Message<T>>(&input).unwrap();

        let mut set = tokio::task::JoinSet::new();

        set.spawn(async move { self.handler.call(input).await });

        let result = set.join_next().await;
        match result.unwrap().unwrap().unwrap() {
            HandlerResponse::Response(_) => todo!("send reply"),
            HandlerResponse::Event(event) => event_broker.publish_event(event),
        }

        // get input
        // get event from input
        // send event to handler
        // get event or response from handler
        // if response send response
        // else if event send to event broker
    }
    fn init() {}
}

#[derive(Debug, Deserialize)]
enum Event<T> {
    Maelstrom { dest: usize, body: Message<T> },
    Injected { dest: usize, body: Message<T> },
}

pub enum HandlerResponse<Res, T> {
    Response(Res),
    Event(Event<T>),
}
struct EventBroker<T> {
    t: std::marker::PhantomData<T>,
}
impl<T> EventBroker<T> {
    pub fn new() -> Self {
        todo!();
    }
    pub fn publish_event(&mut self, event: T) {}
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
#[derive(Debug, Deserialize)]
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

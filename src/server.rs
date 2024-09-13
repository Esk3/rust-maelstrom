use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;

use crate::{
    message::{InitRequest, Message},
    service::Service,
    Fut, Node,
};

#[derive(Debug, Clone)]
pub struct Server<H: Clone> {
    handler: H,
}

impl<H> Server<H>
where
    H: Clone,
{
    pub fn new(handler: H) -> Self {
        Self { handler }
    }
    pub async fn run<T, Res, N>(self)
    where
        H: crate::service::Service<HandlerInput<T, N>, Response = HandlerResponse<Message<Res>, T>>
            + Send
            + 'static,
        T: Serialize + DeserializeOwned + Send + 'static + Debug + Clone,
        Res: Serialize + Send + 'static + Debug,
        N: Node + Send + 'static + Debug,
    {
        let node = Self::init().await;
        let node = std::sync::Arc::new(std::sync::Mutex::new(node));

        let event_broker = EventBroker::new();

        let input = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(input).lines();

        let mut set = tokio::task::JoinSet::new();

        loop {
            tokio::select! {
                line = lines.next_line() => {
                    self.clone().handle_input(&line.unwrap().unwrap(), node.clone(), event_broker.clone(), &mut set);
                },
                Some(response) = set.join_next() => {
                    Self::handle_output(response.context("thread panicked").unwrap().context("handler returned error").unwrap(), &event_broker);
                }
            }
        }
    }
    fn handle_input<T, N, Res>(
        mut self,
        line: &str,
        node: Arc<Mutex<N>>,
        event_broker: EventBroker<T>,
        set: &mut tokio::task::JoinSet<anyhow::Result<HandlerResponse<Message<Res>, T>>>,
    ) where
        H: crate::service::Service<HandlerInput<T, N>, Response = HandlerResponse<Message<Res>, T>>
            + Send
            + 'static,
        N: Send + 'static + Debug,
        T: DeserializeOwned + Send + 'static + Debug,
        Res: Send + 'static,
    {
        let input = serde_json::from_str::<Message<T>>(line).with_context(|| format!("found unknown input {line}")).unwrap();
        let input = HandlerInput {
            message: input,
            node,
            event_broker,
        };

        set.spawn(async move { self.handler.call(input).await });
    }
    fn handle_output<Res, T>(
        handler_response: HandlerResponse<Message<Res>, T>,
        event_broker: &EventBroker<T>,
    ) where
        T: Debug + Send + 'static,
        Res: Serialize + Debug,
    {
        match handler_response {
            HandlerResponse::Response(response) => response.send(std::io::stdout().lock()),
            HandlerResponse::Event(event) => event_broker.publish_event(event),
            HandlerResponse::None => (),
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
                .with_body(crate::message::InitResponse::InitOk {
                    in_reply_to: msg_id,
                })
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

#[derive(Debug)]
pub enum HandlerResponse<Res, T> {
    Response(Res),
    Event(Event<T>),
    None,
}
#[derive(Debug, Clone)]
pub struct EventBroker<T> {
    new_ids_tx:
        tokio::sync::mpsc::UnboundedSender<(usize, tokio::sync::oneshot::Sender<Message<T>>)>,
    events_tx: tokio::sync::mpsc::UnboundedSender<Event<T>>,
}
impl<T> EventBroker<T>
where
    T: Debug + Send + 'static,
{
    #[must_use]
    pub fn new() -> Self {
        let (new_ids_tx, mut new_ids_rx) = tokio::sync::mpsc::unbounded_channel();
        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut subscribers = HashMap::<usize, tokio::sync::oneshot::Sender<Message<T>>>::new();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = new_ids_rx.recv() => {
                        let (id, tx) = msg.unwrap();
                        subscribers.insert(id, tx);
                    },
                    event = events_rx.recv() => {
                        dbg!(&event);
                        let (id, body) = match event.unwrap() {
                            Event::Maelstrom { dest, body } | Event::Injected { dest, body } => (dest, body),
                        };
                        let Some(tx) = subscribers.remove(&id) else {
                            return;
                        };
                        tx.send(body).unwrap();
                    }
                }
            }
        });
        Self {
            new_ids_tx,
            events_tx,
        }
    }
    #[must_use]
    pub fn subscribe(&self, id: usize) -> tokio::sync::oneshot::Receiver<Message<T>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.new_ids_tx.send((id, tx)).unwrap();
        rx
    }
    pub fn publish_event(&self, event: Event<T>) {
        self.events_tx.send(event).unwrap();
    }
}

impl<T> Default for EventBroker<T>
where
    T: Debug + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct HandlerInput<T, N> {
    pub message: Message<T>,
    pub node: Arc<Mutex<N>>,
    pub event_broker: EventBroker<T>,
}

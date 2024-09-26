use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::AsyncBufReadExt;

use crate::{
    event::{Event, EventBroker, EventId},
    message::{InitRequest, Message},
    service, Node,
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

    pub async fn run<T, Res, N>(self) -> anyhow::Result<()>
    where
        H: service::Service<HandlerInput<T, N>, Response = HandlerResponse<Message<Res>, T>>
            + Send
            + 'static,
        T: Serialize + DeserializeOwned + Send + 'static + Debug + Clone + EventId,
        Res: Serialize + Send + 'static + Debug,
        N: Node + Send + 'static + Debug,
    {
        let node = Self::init().await?;
        let node = std::sync::Arc::new(std::sync::Mutex::new(node));

        let event_broker = EventBroker::new();

        let input = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(input).lines();

        let mut set = tokio::task::JoinSet::new();

        loop {
            tokio::select! {
                line = lines.next_line() => {
                    let line = line.context("Error reading line")?
                                .context("Out of lines to read")?;
                    self.clone()
                        .handle_input(&line, node.clone(), event_broker.clone(), &mut set)?;
                },
                Some(response) = set.join_next() => {
                    let response = response.context("Future panicked")?
                        .context("Handler returned an error")?;
                    Self::handle_output(response, &event_broker)?;
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
    ) -> anyhow::Result<()>
    where
        H: service::Service<HandlerInput<T, N>, Response = HandlerResponse<Message<Res>, T>>
            + Send
            + 'static,
        N: Send + 'static + Debug,
        T: DeserializeOwned + Send + 'static + Debug + Clone + EventId,
        Res: Send + 'static,
    {
        //TODO this error gets ignored. only the builtin error message gets displayed
        // let input = if let Ok(input) = serde_json::from_str::<Message<T>>(line) {
        //     Event::Maelstrom(input)
        // // } else {
        //     let event = serde_json::from_str::<Message<BuiltInEvent>>(line)
        //         .with_context(|| format!("Found unknown input {line}"))?;
        //     Event::BuiltIn(event)
        // };

        let input = serde_json::from_str::<Message<T>>(line)
            .with_context(|| format!("Found unknown input {line}"))?;
        let input = Event::Maelstrom(input);
        let input = HandlerInput {
            event: input,
            node,
            event_broker,
        };

        set.spawn(async move { self.handler.call(input).await });
        Ok(())
    }
    fn handle_output<Res, T>(
        handler_response: HandlerResponse<Message<Res>, T>,
        event_broker: &EventBroker<T>,
    ) -> anyhow::Result<()>
    where
        T: Debug + Send + 'static + Clone + EventId,
        Res: Serialize + Debug,
    {
        match handler_response {
            HandlerResponse::Response(response) => response.send(std::io::stdout().lock()),
            HandlerResponse::Error(error) => error.send(std::io::stdout().lock()),
            HandlerResponse::Event(event) => event_broker.publish_event(event),
            HandlerResponse::None => Ok(()),
        }
    }
    async fn init<N>() -> anyhow::Result<N>
    where
        N: Node,
    {
        let stdin = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(stdin).lines();

        let init_line = lines
            .next_line()
            .await
            .context("Failed to read init line")?
            .context("Init line missing")?;
        let init_message: Message<InitRequest> = serde_json::from_str(&init_line)
            .with_context(|| format!("failed to parse init line from {init_line}"))?;

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
                .send(&mut output)?;
        }
        Ok(node)
    }
}

#[derive(Debug)]
pub enum HandlerResponse<Res, T: EventId + Clone> {
    Response(Res),
    Error(Message<crate::error::Error>),
    Event(Event<T>),
    None,
}

#[derive(Debug)]
pub struct HandlerInput<T: EventId + Clone, N> {
    pub event: Event<T>,
    pub node: Arc<Mutex<N>>,
    pub event_broker: EventBroker<T>,
}

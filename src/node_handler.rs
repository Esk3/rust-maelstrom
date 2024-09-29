use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, hash::Hash};
use tokio::io::AsyncBufReadExt;

use crate::{
    message::{InitRequest, Message},
    new_event::{self, EventHandler},
};

pub struct NodeHandler<I, T, N, Id> {
    node: N,
    event_handler: EventHandler<I, T, Id>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<new_event::Event<I, T>>,
}

impl<I, T, N, Id> NodeHandler<I, T, N, usize>
where
    N: crate::node::Node<I, T> + Clone + Sync + Send,
    T: new_event::IntoBodyId<Id> + 'static + Clone + Serialize,
    I: new_event::IntoBodyId<Id> + 'static + DeserializeOwned + Clone,
    Id: Hash + Eq + PartialEq + Debug + Clone + Sync + Send + 'static,
{
    pub async fn init() -> anyhow::Result<Self> {
        Self::init_with_io(tokio::io::stdin(), std::io::stdout().lock()).await
    }

    pub async fn init_with_io<R, W>(reader: R, writer: W) -> anyhow::Result<Self>
    where
        R: tokio::io::AsyncRead + std::marker::Unpin + Debug,
        W: std::io::Write,
    {
        let node = Self::init_node(reader, writer).await?;
        let (event_handler, event_rx) = new_event::EventHandler::new_with_fallback();
        Ok(Self {
            node,
            event_handler,
            event_rx,
        })
    }

    async fn init_node<R, W>(reader: R, writer: W) -> anyhow::Result<N>
    where
        R: tokio::io::AsyncRead + std::marker::Unpin + Debug,
        W: std::io::Write,
    {
        let init_request = Self::read_init_line(reader).await.unwrap();
        let (message, body) = init_request.split();
        let InitRequest::Init {
            msg_id,
            node_id,
            node_ids,
        } = body;
        let node = N::init(node_id, node_ids, ());
        Self::send_init_reply(message, msg_id, writer)?;
        Ok(node)
    }

    async fn read_init_line<R>(reader: R) -> anyhow::Result<Message<InitRequest>>
    where
        R: tokio::io::AsyncRead + Unpin + Debug,
    {
        let mut lines = tokio::io::BufReader::new(reader).lines();

        let init_line = lines
            .next_line()
            .await
            .context("Failed to read init line")?
            .context("Init line missing")?;
        serde_json::from_str(&init_line)
            .with_context(|| format!("failed to parse init line from {init_line}"))
    }

    fn send_init_reply<W>(init_message: Message<()>, msg_id: usize, writer: W) -> anyhow::Result<()>
    where
        W: std::io::Write,
    {
        let (reply, ()) = init_message.into_reply();
        reply
            .with_body(crate::message::InitResponse::InitOk {
                in_reply_to: msg_id,
            })
            .send(writer)
    }

    pub async fn run(self) {
        self.run_with_io(tokio::io::stdin(), std::io::stdout().lock())
            .await;
    }

    pub async fn run_with_io<R, W>(mut self, reader: R, writer: W)
    where
        R: tokio::io::AsyncRead + std::marker::Unpin,
        W: std::io::Write,
    {
        let mut lines = tokio::io::BufReader::new(reader).lines();
        self.event_rx.recv().await;
        tokio::select! {
            line = lines.next_line() => {
                self.on_new_line(line);
            },
            event = self.event_rx.recv() => {
                self.handle_event(event, writer);
            }
        }
    }

    fn on_new_line(&self, line: Result<Option<String>, std::io::Error>) {
        let input_message = serde_json::from_str(&line.unwrap().unwrap()).unwrap();
        let event = new_event::Event::MessageRecived(input_message);
        self.event_handler.publish_event(event);
    }
    fn handle_event<W>(self, event: Option<new_event::Event<I, T>>, writer: W) {
        self.node
            .on_event(event.unwrap(), self.event_handler.clone(), writer);
    }
}

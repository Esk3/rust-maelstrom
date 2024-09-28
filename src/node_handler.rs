use anyhow::Context;
use std::{fmt::Debug, hash::Hash};
use tokio::io::AsyncBufReadExt;

use crate::{
    message::{InitRequest, Message},
    new_event::{self, EventHandler},
};

pub struct NodeHandler<I, T, N, Id> {
    node: N,
    event_handler: EventHandler<I, T, Id>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<new_event::Event<I,T>>,
}

impl<I, T, N, Id> NodeHandler<I, T, N, Id>
where
    N: crate::node::Node<I, T>,
    T: new_event::IntoBodyId<Id> + 'static,
    I: new_event::IntoBodyId<Id> + 'static,
    Id: Hash + Eq + PartialEq + Debug + Clone + Sync + Send + 'static,
{
    pub async fn init() -> Self {
        let node = Self::init_node().await.unwrap();
        let (event_handler, event_rx) = new_event::EventHandler::new_with_fallback();
        Self {
            node,
            event_handler,
            event_rx
        }
    }

    async fn init_node() -> anyhow::Result<N> {
        let init_request = Self::read_init_line().await.unwrap();
        let (message, body) = init_request.split();
        let InitRequest::Init {
            msg_id,
            node_id,
            node_ids,
        } = body;
        let node = N::init(node_id, node_ids, ());
        Self::send_init_reply(message, msg_id)?;
        Ok(node)
    }

    async fn read_init_line() -> anyhow::Result<Message<InitRequest>> {
        let stdin = tokio::io::stdin();
        let mut lines = tokio::io::BufReader::new(stdin).lines();

        let init_line = lines
            .next_line()
            .await
            .context("Failed to read init line")?
            .context("Init line missing")?;
        serde_json::from_str(&init_line)
            .with_context(|| format!("failed to parse init line from {init_line}"))
    }

    fn send_init_reply(init_message: Message<()>, msg_id: usize) -> anyhow::Result<()> {
        let (reply, ()) = init_message.into_reply();
        reply
            .with_body(crate::message::InitResponse::InitOk {
                in_reply_to: msg_id,
            })
            .send(std::io::stdout().lock())
    }

    pub async fn run(mut self) {
        self.event_rx.recv().await;
    }
}

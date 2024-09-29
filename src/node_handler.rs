use anyhow::{bail, Context};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use tokio::io::AsyncBufReadExt;

use crate::{
    message::{InitRequest, Message},
    new_event::{self, EventHandler},
    node::NodeResponse,
};

pub struct NodeHandler<I, T, N, Id> {
    node: N,
    event_handler: EventHandler<I, T, Id>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<new_event::Event<I, T>>,
    tasks: tokio::task::JoinSet<Result<anyhow::Result<NodeResponse<I, T>>, tokio::task::JoinError>>,
}

impl<I, T, N> NodeHandler<I, T, N, usize>
where
    N: crate::node::Node<I, T> + Clone + Sync + Send + 'static,
    T: new_event::IntoBodyId<usize> + 'static + Clone + Serialize,
    I: new_event::IntoBodyId<usize> + 'static + DeserializeOwned + Clone,
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
            tasks: tokio::task::JoinSet::new(),
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
        self.run_with_io(tokio::io::stdin(), std::io::stdout())
            .await;
    }

    pub async fn run_with_io<R, W>(mut self, reader: R, mut writer: W)
    where
        R: tokio::io::AsyncRead + std::marker::Unpin,
        W: std::io::Write + Send,
    {
        let mut lines = tokio::io::BufReader::new(reader).lines();
        loop {
            tokio::select! {
                Ok(Some(line)) = lines.next_line() => {
                    if self.on_new_line(line).is_err() {
                        break;
                    }
                },
                event = self.event_rx.recv() => {
                    self.handle_event(event);
                }
                Some(task) = self.tasks.join_next() => {
                    if Self::on_task_complete(task.unwrap(), &mut writer).is_err() {
                        break;
                    }
                }
                else => {
                    panic!("got else?");
                }
            }
        }
        while self.tasks.join_next().await.is_some() {}
    }

    fn on_new_line(&self, line: String) -> anyhow::Result<()> {
        let value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(e) => {
                dbg!("failed to parse line", &line, &e);
                bail!("failed to parse line {line:?}: {e}");
            }
        };
        let event = new_event::Event::MessageRecived(value);
        self.event_handler.publish_event(event).unwrap();
        Ok(())
    }
    fn handle_event(&mut self, event: Option<new_event::Event<I, T>>) {
        let join_handle = self
            .node
            .on_event(event.unwrap(), self.event_handler.clone());
        self.tasks.spawn(join_handle);
    }

    fn on_task_complete<W>(
        task: Result<anyhow::Result<NodeResponse<I, T>>, tokio::task::JoinError>,
        writer: W,
    ) -> anyhow::Result<()>
    where
        W: std::io::Write,
    {
        let response = task.unwrap();
        match response {
            Ok(NodeResponse::Event(e)) => todo!("got event {e:?}"),
            Ok(NodeResponse::Message(message)) => {
                message.send(writer).context("failed to write to stdout")?
            }
            Ok(NodeResponse::Reply(message)) => {
                let (reply, body) = message.into_reply();
                reply
                    .with_body(body)
                    .send(writer)
                    .context("failed to write to stdout")?;
            }
            Ok(NodeResponse::Error(e)) => todo!("got node error: {e:?}"),
            Ok(NodeResponse::None) => (),
            Err(e) => todo!("got err: {e:?}"),
        };
        Ok(())
    }
}

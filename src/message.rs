use std::fmt::Debug;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::event::{EventBroker, EventId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Message<T> {
    pub src: String,
    pub dest: String,
    pub body: T,
}

pub trait MessageId: Serialize + Debug {
    fn get_id(&self) -> usize;
}

impl<T> Message<T> {
    pub fn split(self) -> (Message<()>, T) {
        (
            Message {
                src: self.src,
                dest: self.dest,
                body: (),
            },
            self.body,
        )
    }
    pub fn into_reply(self) -> (Message<()>, T) {
        let body = self.body;
        (
            Message {
                src: self.dest,
                dest: self.src,
                body: (),
            },
            body,
        )
    }
    pub fn with_body<B>(self, body: B) -> Message<B> {
        Message {
            src: self.src,
            dest: self.dest,
            body,
        }
    }
}

impl<T> Message<T>
where
    T: Serialize,
{
    pub fn send<W>(&self, mut writer: W) -> anyhow::Result<()>
    where
        W: std::io::Write,
    {
        serde_json::to_writer(&mut writer, self).context("Error writing to stdout")?;
        writer.write_all(b"\n").context("Error writing to stdout")
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MessageType<M, P, R> {
    Request(Request<M, P>),
    Response(Message<PeerMessage<R>>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Request<M, P> {
    Maelstrom(Message<M>),
    Peer(Message<PeerMessage<P>>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerMessage<T> {
    pub src: usize,
    pub dest: Option<usize>,
    pub id: usize,
    pub body: T,
}

impl<T> PeerMessage<T> {
    pub fn split(self) -> (PeerMessage<()>, T) {
        (
            PeerMessage {
                src: self.src,
                dest: self.dest,
                id: self.id,
                body: (),
            },
            self.body,
        )
    }
    pub fn into_reply(self, src: usize) -> (PeerMessage<()>, T) {
        let body = self.body;
        (
            PeerMessage {
                src,
                dest: Some(self.src),
                id: self.id,
                body: (),
            },
            body,
        )
    }
    pub fn with_body<B>(self, body: B) -> PeerMessage<B> {
        PeerMessage {
            src: self.src,
            dest: self.dest,
            id: self.id,
            body,
        }
    }
    pub fn matches_response<B>(&self, res: &Message<PeerMessage<B>>) -> bool {
        res.body.id == self.id
    }
}

pub async fn send_messages_with_retry<T>(
    mut messages: Vec<Message<T>>,
    interval: std::time::Duration,
    event_broker: EventBroker<T>,
) -> anyhow::Result<()>
where
    T: EventId + Clone + Send + 'static + Debug + Serialize,
{
    let mut interval = tokio::time::interval(interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut set = tokio::task::JoinSet::new();
    messages
        .iter()
        .map(|message| event_broker.subscribe(message.body.get_event_id()))
        .for_each(|c| {
            set.spawn(c);
        });
    while !messages.is_empty() {
        tokio::select! {
            _ = interval.tick() => {
                let mut out = std::io::stdout().lock();
                for message in &messages {
                    message.send(&mut out)?;
                }
            },
            Some(response) = set.join_next() => {
                let response = response.context("thread panicked").unwrap().context("future returned error").unwrap();
                messages.retain(|message| message.body.get_event_id() != response.id());
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InitRequest {
    Init {
        msg_id: usize,
        node_id: String,
        node_ids: Vec<String>,
    },
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InitResponse {
    InitOk { in_reply_to: usize },
}

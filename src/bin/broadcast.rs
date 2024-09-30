use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use rust_maelstrom::{
    id_counter::SeqIdCounter,
    message::Message,
    new_event::{self, EventHandler, IntoBodyId, MessageId},
    node::NodeResponse,
    Fut,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rust_maelstrom::node_handler::NodeHandler::<Request, Response, BroadcastNode, _>::init()
        .await?
        .run()
        .await;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct BroadcastNode {
    node_id: Arc<String>,
    neighbors: Arc<RwLock<Vec<String>>>,
    messages: Arc<Mutex<Vec<serde_json::Value>>>,
    id_generator: SeqIdCounter,
}

impl rust_maelstrom::node::Node<Request, Response> for BroadcastNode {
    fn init(node_id: String, _node_ids: Vec<String>, _state: ()) -> Self {
        Self {
            node_id: Arc::new(node_id),
            neighbors: Arc::default(),
            messages: Arc::default(),
            id_generator: SeqIdCounter::new(),
        }
    }

    fn handle_message_recived(
        self,
        message: Message<Request>,
        event_handler: rust_maelstrom::new_event::EventHandler<Request, Response, usize>,
    ) -> Fut<rust_maelstrom::node::NodeResponse<Request, Response>> {
        let (message, body) = message.split();
        Box::pin(async move {
            match body {
                Request::Topology { topology, msg_id } => self
                    .topology(topology, msg_id)
                    .map(|body| message.with_body(body).into_node_reply()),
                Request::Broadcast {
                    message: value,
                    msg_id,
                } => {
                    message
                        .clone()
                        .into_reply()
                        .0
                        .with_body(Response::BroadcastOk {
                            in_reply_to: msg_id,
                        })
                        .send(std::io::stdout())
                        .unwrap();
                    let body = self.broadcast(msg_id, value, &message, event_handler).await;
                    Ok(message.with_body(body).into_node_reply())
                }
                Request::Read { msg_id } => {
                    let body = self.read(msg_id);
                    Ok(message.with_body(body).into_node_reply())
                }
                Request::BroadcastOk { in_reply_to: _ } => Ok(NodeResponse::Event(
                    new_event::Event::MessageRecived(message.with_body(body)),
                )),
            }
        })
    }
}

impl BroadcastNode {
    fn topology(
        &self,
        mut topology: HashMap<String, Vec<String>>,
        msg_id: usize,
    ) -> anyhow::Result<Response> {
        {
            let neighbors = topology
                .remove(&*self.node_id)
                .context("node id not in topology")?;
            *self.neighbors.write().unwrap() = neighbors;
        }
        Ok(Response::TopologyOk {
            in_reply_to: msg_id,
        })
    }

    async fn broadcast<T>(
        &self,
        msg_id: usize,
        value: serde_json::Value,
        message: &Message<T>,
        event_handler: EventHandler<Request, Response, usize>,
    ) -> Response {
        {
            let mut messages = self.messages.lock().unwrap();
            if messages.contains(&value) {
                return Response::BroadcastOk {
                    in_reply_to: msg_id,
                };
            }

            messages.push(value.clone());
        }

        let mut listners = tokio::task::JoinSet::new();
        let messages = self.create_filtered_peer_messages(&value, &message.src);

        for message in messages {
            let mut listner = event_handler
                .subscribe(&MessageId {
                    src: message.dest.clone(),
                    dest: message.src.clone(),
                    id: message.body.clone_into_body_id(),
                })
                .unwrap();
            listners.spawn(async move { listner.recv().await });
            message.send(std::io::stdout()).unwrap();
        }

        let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async move {
            while (listners.join_next().await).is_some() {}
        })
        .await;

        Response::BroadcastOk {
            in_reply_to: msg_id,
        }
    }

    fn create_filtered_peer_messages(
        &self,
        message: &serde_json::Value,
        src: &str,
    ) -> Vec<Message<Request>> {
        self.neighbors
            .read()
            .unwrap()
            .iter()
            .filter(|nei| nei.as_str() != src)
            .map(|nei| Message {
                src: self.node_id.to_string(),
                dest: nei.clone(),
                body: Request::Broadcast {
                    message: message.clone(),
                    msg_id: self.id_generator.next_id(),
                },
            })
            .collect()
    }

    fn create_peer_messages(&self, message: &serde_json::Value) -> Vec<Message<Request>> {
        self.neighbors
            .read()
            .unwrap()
            .iter()
            .map(|nei| Message {
                src: self.node_id.to_string(),
                dest: nei.clone(),
                body: Request::Broadcast {
                    message: message.clone(),
                    msg_id: self.id_generator.next_id(),
                },
            })
            .collect()
    }

    fn read(&self, msg_id: usize) -> Response {
        let messages = self.messages.lock().unwrap().clone();
        Response::ReadOk {
            messages,
            in_reply_to: msg_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Topology {
        topology: HashMap<String, Vec<String>>,
        msg_id: usize,
    },
    Broadcast {
        message: serde_json::Value,
        msg_id: usize,
    },
    Read {
        msg_id: usize,
    },
    BroadcastOk {
        in_reply_to: usize,
    },
}

impl rust_maelstrom::new_event::IntoBodyId<usize> for Request {
    fn into_body_id(self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Request::Topology {
                topology: _,
                msg_id,
            }
            | Request::Broadcast { message: _, msg_id }
            | Request::Read { msg_id } => msg_id.into(),
            Request::BroadcastOk { in_reply_to } => in_reply_to.into(),
        }
    }

    fn clone_into_body_id(&self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Request::Topology {
                topology: _,
                msg_id,
            }
            | Request::Broadcast { message: _, msg_id }
            | Request::Read { msg_id } => msg_id.into(),
            Request::BroadcastOk { in_reply_to } => in_reply_to.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    TopologyOk {
        in_reply_to: usize,
    },
    BroadcastOk {
        in_reply_to: usize,
    },
    ReadOk {
        messages: Vec<serde_json::Value>,
        in_reply_to: usize,
    },
}

impl rust_maelstrom::new_event::IntoBodyId<usize> for Response {
    fn into_body_id(self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Response::TopologyOk { in_reply_to }
            | Response::BroadcastOk { in_reply_to }
            | Response::ReadOk {
                messages: _,
                in_reply_to,
            } => in_reply_to.into(),
        }
    }

    fn clone_into_body_id(&self) -> rust_maelstrom::new_event::BodyId<usize> {
        match self {
            Response::TopologyOk { in_reply_to }
            | Response::BroadcastOk { in_reply_to }
            | Response::ReadOk {
                messages: _,
                in_reply_to,
            } => in_reply_to.into(),
        }
    }
}

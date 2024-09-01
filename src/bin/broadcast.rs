use std::{collections::HashMap, future::Future, pin::Pin};

use anyhow::bail;
use rust_maelstrom::{
    main_loop,
    message::{self, send_messages_with_retry, Message, PeerMessage},
    service::Service,
    Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    main_loop(rust_maelstrom::handler::Handler::new(Handler)).await;
}

#[derive(Clone)]
struct Handler;
impl Service<rust_maelstrom::RequestArgs<Message<Request>, Response, BroadcastNode>> for Handler {
    type Response = Response;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>>>>;

    fn call(
        &mut self,
        rust_maelstrom::RequestArgs {
            request,
            node,
            id,
            input,
        }: rust_maelstrom::RequestArgs<Message<Request>, Response, BroadcastNode>,
    ) -> Self::Future {
        match request.body {
            Request::Topology {
                mut topology,
                msg_id,
            } => Box::pin(async move {
                {
                    let mut node = node.lock().unwrap();
                    let neighbors = topology.remove(&node.id).unwrap();
                    node.neighbors = neighbors;
                }
                Ok(Response::TopologyOk {
                    in_reply_to: msg_id,
                })
            }),
            Request::Broadcast { message, msg_id } => Box::pin(async move {
                let messages = {
                    let mut node = node.lock().unwrap();
                    let Some(messages) = node.broadcast(&message, id, &request.src, msg_id) else {
                        return Ok(Response::BroadcastOk {
                            in_reply_to: msg_id,
                        });
                    };
                    messages
                };

                send_messages_with_retry(messages, std::time::Duration::from_millis(100), input)
                    .await;
                Ok(Response::BroadcastOk {
                    in_reply_to: msg_id,
                })
            }),
            Request::Read { msg_id } => Box::pin(async move {
                let messages = {
                    let node = node.lock().unwrap();
                    node.messages.clone()
                };
                let body = Response::ReadOk {
                    messages,
                    in_reply_to: msg_id,
                };
                Ok(body)
            }),
        }
    }
}

#[derive(Debug)]
pub struct BroadcastNode {
    id: String,
    neighbors: Vec<String>,
    messages: Vec<serde_json::Value>,
}
impl Node for BroadcastNode {
    fn init(node_id: String, _node_ids: Vec<String>) -> Self {
        Self {
            id: node_id,
            neighbors: Vec::new(),
            messages: Vec::new(),
        }
    }
}

impl BroadcastNode {
    pub fn broadcast(
        &mut self,
        message: &serde_json::Value,
        id: usize,
        src: &str,
        msg_id: usize,
    ) -> Option<Vec<Message<PeerMessage<Request>>>> {
        if self.messages.contains(message) {
            return None;
        }
        self.messages.push(message.clone());

        let messages = self
            .neighbors
            .clone()
            .into_iter()
            .filter(|neighbor| neighbor != src)
            .enumerate()
            .map(|(i, neighbor)| Message {
                src: self.id.clone(),
                dest: neighbor,
                body: PeerMessage {
                    src: id,
                    dest: None,
                    id: i,
                    body: Request::Broadcast {
                        message: message.clone(),
                        msg_id,
                    },
                },
            })
            .collect();
        Some(messages)
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
}
#[derive(Debug, Serialize, Deserialize)]
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

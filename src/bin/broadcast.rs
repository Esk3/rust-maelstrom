use std::collections::HashMap;

use rust_maelstrom::{
    main_loop,
    message::{self, send_messages_with_retry, Message, PeerMessage},
    Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    main_loop(handler).await;
}

async fn handler(
    message: message::Request<Request, PeerRequest>,
    node: std::sync::Arc<std::sync::Mutex<BroadcastNode>>,
    id: usize,
    input: tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<PeerResponse>>>,
) {
    match message {
        message::Request::Maelstrom(message) => {
            let src = message.src.clone();
            let (reply, body) = message.into_reply();
            match body {
                Request::Topology {
                    mut topology,
                    msg_id,
                } => {
                    {
                        let mut lock = node.lock().unwrap();
                        let neighbors = topology.remove(&lock.id).unwrap();
                        lock.neighbors = neighbors;
                    }
                    let mut output = std::io::stdout().lock();
                    let body = Response::TopologyOk {
                        in_reply_to: msg_id,
                    };
                    reply.with_body(body).send(&mut output);
                }
                Request::Broadcast { message, msg_id } => {
                    {
                        let out = std::io::stdout().lock();
                        let body = Response::BroadcastOk {
                            in_reply_to: msg_id,
                        };
                        reply.with_body(body).send(out);
                    }
                    let messages = {
                        let mut node = node.lock().unwrap();
                        let Some(messages) = node.broadcast(message, id, src) else {
                            return;
                        };
                        messages
                    };

                    send_messages_with_retry(
                        messages,
                        std::time::Duration::from_millis(100),
                        input,
                    )
                    .await;
                }
                Request::Read { msg_id } => {
                    let messages = {
                        let node = node.lock().unwrap();
                        node.messages.clone()
                    };
                    let output = std::io::stdout().lock();
                    let body = Response::ReadOk {
                        messages,
                        in_reply_to: msg_id,
                    };
                    reply.with_body(body).send(output);
                }
            }
        }
        message::Request::Peer(message) => {
            let src = message.src.clone();
            let (reply, peer) = message.into_reply();
            let (peer, body) = peer.into_reply(id);
            match body {
                PeerRequest::Broadcast { message } => {
                    let messages = {
                        let output = std::io::stdout().lock();
                        let messages = {
                            let mut node = node.lock().unwrap();
                            node.broadcast(message, id, src)
                        };
                        reply
                            .with_body(peer.with_body(PeerResponse::BroadcastOk))
                            .send(output);
                        match messages {
                            Some(messages) => messages,
                            None => return,
                        }
                    };

                    send_messages_with_retry(
                        messages,
                        std::time::Duration::from_millis(100),
                        input,
                    )
                    .await;
                }
            }
        }
    }
}

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
        message: serde_json::Value,
        id: usize,
        src: String,
    ) -> Option<Vec<Message<PeerMessage<PeerRequest>>>> {
        if self.messages.contains(&message) {
            return None;
        }
        self.messages.push(message.clone());

        let messages = self
            .neighbors
            .clone()
            .into_iter()
            .filter(|neighbor| neighbor != &src)
            .enumerate()
            .map(|(i, neighbor)| Message {
                src: self.id.clone(),
                dest: neighbor,
                body: PeerMessage {
                    src: id,
                    dest: None,
                    id: i,
                    body: PeerRequest::Broadcast {
                        message: message.clone(),
                    },
                },
            })
            .collect();
        Some(messages)
    }
}

#[derive(Debug, Deserialize)]
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
#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub enum PeerRequest {
    Broadcast { message: serde_json::Value },
}
#[derive(Debug, Serialize, Deserialize)]
pub enum PeerResponse {
    BroadcastOk,
}

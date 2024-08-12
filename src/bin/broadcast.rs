use std::collections::HashMap;

use rust_maelstrom::{
    main_loop,
    message::{self, Message, PeerMessage},
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
    mut _input: tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<PeerResponse>>>,
) {
    match message {
        message::Request::Maelstrom(message) => {
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
                    let mut lock = node.lock().unwrap();
                    if lock.messages.contains(&message) {
                        return;
                    }
                    lock.messages.push(message.clone());

                    let mut output = std::io::stdout().lock();
                    for neighbor in &lock.neighbors {
                        Message {
                            src: lock.id.clone(),
                            dest: neighbor.clone(),
                            body: PeerMessage {
                                src: id,
                                dest: None,
                                body: PeerRequest::Broadcast {
                                    message: message.clone(),
                                },
                            },
                        }
                        .send(&mut output);
                    }
                    let body = Response::BroadcastOk {
                        in_reply_to: msg_id,
                    };
                    reply.with_body(body).send(&mut output);
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
            let (_reply, peer) = message.into_reply();
            match peer.body {
                PeerRequest::Broadcast { message } => {
                    let mut lock = node.lock().unwrap();
                    if lock.messages.contains(&message) {
                        return;
                    }
                    lock.messages.push(message.clone());

                    let mut output = std::io::stdout().lock();
                    for neighbor in &lock.neighbors {
                        Message {
                            src: lock.id.clone(),
                            dest: neighbor.clone(),
                            body: PeerMessage {
                                src: id,
                                dest: None,
                                body: PeerRequest::Broadcast {
                                    message: message.clone(),
                                },
                            },
                        }
                        .send(&mut output);
                    }
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
pub enum PeerResponse {}

use std::{collections::HashMap};

use rust_maelstrom::{
    message::{send_messages_with_retry, Message, PeerMessage}, service::Service, Fut, Ids, Node
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    let s = rust_maelstrom::server::Server::new(Handler);
    s.run().await;
}

#[derive(Clone)]
struct Handler;
impl Service<rust_maelstrom::server::HandlerInput<Request, BroadcastNode>> for Handler {
    type Response = rust_maelstrom::server::HandlerResponse<Message<Response>, Request>;

    type Future = Fut<Self::Response>;

    fn call(
        &mut self,
        rust_maelstrom::server::HandlerInput {
            message,
            node,
            event_broker,
        }: rust_maelstrom::server::HandlerInput<Request, BroadcastNode>,
    ) -> Self::Future {
        let src = message.src.clone();
        let (reply, body) = message.into_reply();
        match body {
            Request::Topology {
                mut topology,
                msg_id,
            } => Box::pin(async move {
                {
                    let mut node = node.lock().unwrap();
                    let neighbors = topology.remove(&node.node_id).unwrap();
                    node.neighbors = neighbors;
                }
                Ok(rust_maelstrom::server::HandlerResponse::Response(reply.with_body(Response::TopologyOk {
                    in_reply_to: msg_id,
                })))
            }),
            Request::Broadcast { message, msg_id } => Box::pin(async move {
                let messages = {
                    let mut node = node.lock().unwrap();
                    let Some(messages) = node.broadcast(&message, &src) else {
                        return Ok(rust_maelstrom::server::HandlerResponse::Response(
                            reply.with_body(Response::BroadcastOk {
                                in_reply_to: msg_id,
                            }),
                        ));
                    };
                    messages
                };

                send_messages_with_retry(messages, std::time::Duration::from_millis(100), event_broker)
                    .await;
                Ok(rust_maelstrom::server::HandlerResponse::Response(
                    reply.with_body(Response::BroadcastOk {
                        in_reply_to: msg_id,
                    }),
                ))
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
                Ok(rust_maelstrom::server::HandlerResponse::Response(reply.with_body(body)))
            }),
        }
    }
}

#[derive(Debug)]
pub struct BroadcastNode {
    node_id: String,
    neighbors: Vec<String>,
    messages: Vec<serde_json::Value>,
    id_generator: Ids,

}
impl Node for BroadcastNode {
    fn init(node_id: String, _node_ids: Vec<String>) -> Self {
        Self {
            node_id,
            neighbors: Vec::new(),
            messages: Vec::new(),
            id_generator: Ids::new(),
        }
    }
}

impl BroadcastNode {
    pub fn broadcast(
        &mut self,
        message: &serde_json::Value,
        src: &str,
    ) -> Option<Vec<Message<Request>>> {
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
            .map(|(_i, neighbor)| Message {
                src: self.node_id.clone(),
                dest: neighbor,
                body: Request::Broadcast {
                        message: message.clone(),
                        msg_id: self.id_generator.next_id(),
                },
            })
            .collect();
        Some(messages)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
impl rust_maelstrom::message::MessageId for Request {
    fn get_id(&self) -> usize {
        match self {
            Request::Topology { msg_id, .. }
            | Request::Broadcast { msg_id, .. }
            | Request::Read { msg_id } => *msg_id,
        }
    }
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

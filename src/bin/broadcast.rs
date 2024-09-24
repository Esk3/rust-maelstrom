use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use rust_maelstrom::{
    event,
    id_counter::Ids,
    message::{send_messages_with_retry, Message},
    server,
    service::Service,
    Fut, Node,
};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let s = server::Server::new(Handler);
    s.run().await?;
    Ok(())
}

#[derive(Clone)]
struct Handler;
impl Service<server::HandlerInput<Request, BroadcastNode>> for Handler {
    type Response = server::HandlerResponse<Message<Response>, Request>;

    type Future = Fut<Self::Response>;

    fn call(
        &mut self,
        server::HandlerInput {
            message,
            node,
            event_broker,
        }: server::HandlerInput<Request, BroadcastNode>,
    ) -> Self::Future {
        let src = message.src.clone();
        let (reply, body) = message.into_reply();
        match body {
            Request::Topology { topology, msg_id } => Box::pin(async move {
                handle_topology(&node, topology, msg_id)
                    .map(|body| server::HandlerResponse::Response(reply.with_body(body)))
            }),
            Request::Broadcast { message, msg_id } => Box::pin(async move {
                broadcast(node, message, &src, msg_id, event_broker)
                    .await
                    .map(|body| server::HandlerResponse::Response(reply.with_body(body)))
            }),
            Request::Read { msg_id } => Box::pin(async move {
                let body = read(&node, msg_id);
                Ok(server::HandlerResponse::Response(reply.with_body(body)))
            }),
            Request::BroadcastOk { in_reply_to } => Box::pin(async move {
                Ok(server::HandlerResponse::Event(event::Event::Injected (
                    reply.into_reply().0.with_body(body),
                )))
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
    BroadcastOk {
        in_reply_to: usize,
    },
}

impl event::EventId for Request {
    fn get_event_id(&self) -> usize {
        match self {
            Request::Topology { msg_id, .. }
            | Request::Broadcast { msg_id, .. }
            | Request::Read { msg_id } => *msg_id,
            Request::BroadcastOk { in_reply_to } => *in_reply_to,
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

fn handle_topology(
    node: &Arc<Mutex<BroadcastNode>>,
    mut topology: HashMap<String, Vec<String>>,
    msg_id: usize,
) -> anyhow::Result<Response> {
    {
        let mut node = node.lock().unwrap();
        let neighbors = topology
            .remove(&node.node_id)
            .context("node id not in topology")?;
        node.neighbors = neighbors;
    }
    Ok(Response::TopologyOk {
        in_reply_to: msg_id,
    })
}
async fn broadcast(
    node: Arc<Mutex<BroadcastNode>>,
    message: serde_json::Value,
    src: &str,
    msg_id: usize,
    event_broker: event::EventBroker<Request>,
) -> anyhow::Result<Response> {
    let messages = {
        let mut node = node.lock().unwrap();
        let Some(messages) = node.broadcast(&message, src) else {
            return Ok(Response::BroadcastOk {
                in_reply_to: msg_id,
            });
        };
        messages
    };

    send_messages_with_retry(
        messages,
        std::time::Duration::from_millis(100),
        event_broker,
    )
    .await?;
    Ok(Response::BroadcastOk {
        in_reply_to: msg_id,
    })
}
fn read(node: &Arc<Mutex<BroadcastNode>>, msg_id: usize) -> Response {
    let messages = {
        let node = node.lock().unwrap();
        node.messages.clone()
    };
    Response::ReadOk {
        messages,
        in_reply_to: msg_id,
    }
}

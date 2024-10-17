use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use rust_maelstrom::{
    id_counter::SeqIdCounter,
    message::Message,
    node_handler::NodeHandler,
    service::{
        event::{AsBodyId, EventBroker, EventLayer, MessageId},
        json::JsonLayer,
        Service,
    },
    Fut, Node,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let event_broker = EventBroker::<Request>::new();
    let node = NodeHandler::<()>::init_node::<BroadcastNode, _>(event_broker.clone()).await;
    let service = JsonLayer::new(EventLayer::new(node, event_broker));
    let handler = NodeHandler::new(service);
    handler.run().await;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct BroadcastNode {
    node_id: Arc<String>,
    neighbors: Arc<RwLock<Vec<String>>>,
    messages: Arc<Mutex<Vec<serde_json::Value>>>,
    id_generator: SeqIdCounter,
    event_broker: EventBroker<Request>,
}

impl Node<EventBroker<Request>> for BroadcastNode {
    fn init(node_id: String, _node_ids: Vec<String>, state: EventBroker<Request>) -> Self {
        Self {
            node_id: Arc::new(node_id),
            neighbors: Arc::default(),
            messages: Arc::default(),
            id_generator: SeqIdCounter::new(),
            event_broker: state,
        }
    }
}

impl Service<Message<Request>> for BroadcastNode {
    type Response = Option<Message<Response>>;

    type Future = Fut<Self::Response>;

    fn call(&mut self, request: Message<Request>) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move { this.handle_request(request).await })
    }
}

impl BroadcastNode {
    async fn handle_request(
        &mut self,
        message: Message<Request>,
    ) -> anyhow::Result<Option<Message<Response>>> {
        let (message, body) = message.split();
        let response = match body {
            Request::Topology { topology, msg_id } => self.topology(topology, msg_id),
            Request::Broadcast {
                message: value,
                msg_id,
            } => Ok(self.broadcast(msg_id, value, &message).await),
            Request::Read { msg_id } => Ok(self.read(msg_id)),
            Request::BroadcastOk { in_reply_to } => {
                let message = message.with_body(Request::BroadcastOk { in_reply_to });
                let id = MessageId {
                    this: message.dest.to_string(),
                    other: message.src.to_string(),
                    id: message.body.as_body_id(),
                };
                self.event_broker.publish_or_store_id(&id, message);
                return Ok(None);
            }
        };
        response
            .map(|body| message.into_reply().0.with_body(body))
            .map(Some)
    }

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
            let id = MessageId {
                this: message.src.to_string(),
                other: message.dest.to_string(),
                id: message.body.as_body_id(),
            };
            let mut listner = self.event_broker.subsribe_to_id(id);
            listners.spawn(async move {
                let mut sleep_time = 100;
                let sleep_increase = 100;
                loop {
                    message.send(std::io::stdout().lock()).unwrap();
                    tokio::select! {
                        _ = &mut listner => {
                            break;
                        },
                        () = tokio::time::sleep(std::time::Duration::from_millis(sleep_time)) => {
                            sleep_time += sleep_increase;
                            if sleep_time > 2000 {
                                dbg!("no response");
                                break;
                            }
                        }
                    }
                }
            });
        }

        while listners.join_next().await.is_some() {}

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

impl AsBodyId for Request {
    fn as_raw_id(&self) -> String {
        match self {
            Request::Topology {
                topology: _,
                msg_id: id,
            }
            | Request::Broadcast {
                message: _,
                msg_id: id,
            }
            | Request::Read { msg_id: id }
            | Request::BroadcastOk { in_reply_to: id } => id.to_string(),
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

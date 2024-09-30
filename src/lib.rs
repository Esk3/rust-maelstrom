pub mod error;
pub mod event;
pub mod handler;
pub mod id_counter;
pub mod input;
pub mod maelstrom_service;
pub mod message;
pub mod new_event;
pub mod node;
pub mod node_handler;
pub mod server;
pub mod service;
pub mod event_worker;

pub type Fut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>;

pub trait Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self;
}

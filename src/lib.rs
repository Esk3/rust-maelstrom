pub mod error;
pub mod id_counter;
pub mod maelstrom_service;
pub mod message;
pub mod node_handler;
pub mod service;

pub type Fut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>;

pub trait Node<S = ()> {
    fn init(node_id: String, node_ids: Vec<String>, state: S) -> Self;
}

pub mod error;
pub mod event;
pub mod handler;
pub mod id_counter;
pub mod input;
pub mod message;
pub mod server;
pub mod service;

pub type Fut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>;

pub trait Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self;
}

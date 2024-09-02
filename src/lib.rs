use handler::{Handler, HandlerRequest, HandlerResponse, RequestArgs};
use input::{InputHandler, InputResponse};
use message::{InitRequest, InitResponse, Message, PeerMessage};
use serde::{de::DeserializeOwned, Serialize};
use service::Service;
use std::{collections::HashMap, fmt::Debug, io::Write};

use tokio::io::AsyncBufReadExt;
pub mod handler;
pub mod input;
pub mod message;
pub mod service;

pub type Fut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>;

pub async fn main_loop<H, N, Req, Res>(handler: H)
where
    H: Service<HandlerRequest<Req, Res, N>, Response = Message<HandlerResponse<Res>>> + Clone + 'static + Send,
    N: Node + 'static + Debug + Send,
    Req: DeserializeOwned + 'static + Debug + Send,
    Res: Serialize + DeserializeOwned + Debug + Send + 'static + Debug,
{
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    let init_line = lines.next_line().await.unwrap().unwrap();
    let init_message: Message<InitRequest> = serde_json::from_str(&init_line).unwrap();
    let (reply, body) = init_message.into_reply();
    let InitRequest::Init {
        msg_id,
        node_id,
        node_ids,
    } = body;

    let node = N::init(node_id, node_ids);
    let node = std::sync::Arc::new(std::sync::Mutex::new(node));

    {
        let mut output = std::io::stdout().lock();
        reply
            .with_body(dbg!(InitResponse::InitOk {
                in_reply_to: msg_id,
            }))
            .send(&mut output);
        output.flush().unwrap();
    }

    let mut input_handler = InputHandler::<Req, Res>::new();
    let mut id = 0;
    let mut set = tokio::task::JoinSet::new();
    let mut channels = HashMap::new();
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let line = line.unwrap().unwrap();
                let input = input_handler.call(line).await.unwrap();
                match input {
                    InputResponse::NewHandler(request) => {
                        id += 1;
                        let (rx, tx) = tokio::sync::mpsc::unbounded_channel();
                        channels.insert(id, rx);
                        let handler_request = HandlerRequest {
                            request,
                            node: node.clone(),
                            id,
                            input: tx,
                        };
                        let mut handler = handler.clone();
                        set.spawn(async move {
                            let response = handler.call(handler_request).await.unwrap();
                            (id, response)
                        });
                    }
                    InputResponse::HandlerMessage { id, message } => if let Some(rx) = channels.get(&id) {
                        rx.send(message).unwrap();
                    } else {
                        dbg!("channel closed", id);
                    },
                }
            },
            Some(handler) = set.join_next() => {
                let (id, response) = handler.unwrap();
                channels.remove(&id);
                response.send(std::io::stdout().lock());
            }
        }
    }
}

pub trait Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self;
}

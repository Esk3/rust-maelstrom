use handler::Handler;
use message::{InitRequest, InitResponse, Message, MessageType, PeerMessage, Request};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use service::Service;
use std::{
    collections::HashMap,
    fmt::Debug,
    future::Future,
    io::{stdout, Write},
};
use tokio::io::AsyncBufReadExt;

pub mod handler;
pub mod input;
pub mod message;
pub mod service;

pub async fn main_loop<H, P, N, Req, Res>(mut handler: Handler<H, P>)
where
    H: Service<RequestArgs<Message<Req>, Res, N>, Response = Res> + Clone + 'static,
    P: Service<RequestArgs<Message<PeerMessage<Req>>, Res, N>, Response = PeerMessage<Res>>
        + Clone
        + 'static,
    N: Node + 'static + Debug,
    Req: DeserializeOwned + 'static,
    Res: Serialize + DeserializeOwned + Debug + 'static,
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

    let mut id = 0;
    while let Ok(line) = lines.next_line().await {
        let line = dbg!(line.unwrap());
        id += 1;
        let request: MessageType2<Req, Res> = serde_json::from_str(&line).unwrap();
        let request = match request {
            MessageType2::Request(req) => req,
            MessageType2::Response(_) => continue,
        };
        let handler_request = HandlerRequest {
            request,
            node: node.clone(),
            id,
            input: tokio::sync::mpsc::unbounded_channel().1,
        };
        let response: Message<_> = handler.call(handler_request).await.unwrap();
        response.send(std::io::stdout().lock());
    }
}

pub trait Node {
    fn init(node_id: String, node_ids: Vec<String>) -> Self;
}

use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum HandlerResponse<Res> {
    Maelstrom(Res),
    Peer(PeerMessage<Res>),
}

#[derive(Debug)]
pub struct RequestArgs<Req, Res, N> {
    pub request: Req,
    pub node: Arc<Mutex<N>>,
    pub id: usize,
    pub input: tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<Res>>>,
}

pub struct HandlerRequest<Req, Res, N> {
    pub request: RequestType<Req>,
    pub node: Arc<Mutex<N>>,
    pub id: usize,
    pub input: tokio::sync::mpsc::UnboundedReceiver<Message<PeerMessage<Res>>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RequestType<Req> {
    Maelstrom(Message<Req>),
    Peer(Message<PeerMessage<Req>>),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum MessageType2<Req, Res> {
    Request(RequestType<Req>),
    Response(Message<PeerMessage<Res>>),
}

pub enum MaelstromRequest {
    Add(usize),
    Read,
}

#[derive(Debug)]
pub enum MaelstromResponse {
    AddOk,
    ReadOk(usize),
}

#[derive(Debug)]
pub enum PeerResponse {}

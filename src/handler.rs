use std::sync::{Arc, Mutex};
use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};

use crate::{
    message::{self, Message, PeerMessage},
    service::Service,
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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RequestType<Req> {
    Maelstrom(Message<Req>),
    Peer(Message<PeerMessage<Req>>),
}

#[derive(Clone)]
pub struct Handler<M, P> {
    pub maelstrom_handler: M,
    pub peer_handler: P,
}

impl<M, P, N, Req, Res> Service<HandlerRequest<Req, Res, N>> for Handler<M, P>
where
    M: Service<RequestArgs<Message<Req>, Res, N>, Response = Res> + Clone + 'static + Send,
    P: Service<RequestArgs<Message<PeerMessage<Req>>, Res, N>, Response = PeerMessage<Res>>
        + Clone
        + 'static
        + Send,
    N: 'static + Send,
    Req: 'static + Send,
    Res: Serialize + 'static + Send,
{
    type Response = Message<HandlerResponse<Res>>;
    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>> + Send>>;

    fn call(&mut self, request: HandlerRequest<Req, Res, N>) -> Self::Future {
        let mut this = self.clone();
        match request.request {
            RequestType::Maelstrom(msg) => Box::pin(async move {
                let (src, dest) = (msg.src.clone(), msg.dest.clone());
                let body = this
                    .maelstrom_handler
                    .call(RequestArgs {
                        request: msg,
                        node: request.node,
                        id: request.id,
                        input: request.input,
                    })
                    .await;
                Ok(Message {
                    src: dest,
                    dest: src,
                    body: HandlerResponse::Maelstrom(body.unwrap()),
                })
            }),
            RequestType::Peer(req) => Box::pin(async move {
                let (src, dest) = (req.src.clone(), req.dest.clone());
                let body = this
                    .peer_handler
                    .call(RequestArgs {
                        request: req,
                        node: request.node,
                        id: request.id,
                        input: request.input,
                    })
                    .await;
                Ok(Message {
                    src: dest,
                    dest: src,
                    body: HandlerResponse::Peer(body.unwrap()),
                })
            }),
        }
    }
}

impl<M> Handler<M, PeerHander<M>> {
    pub fn new(handler: M) -> Self
    where
        M: Clone,
    {
        Self {
            maelstrom_handler: handler.clone(),
            peer_handler: PeerHander { inner: handler },
        }
    }
}

#[derive(Clone)]
pub struct PeerHander<S> {
    pub inner: S,
}
impl<Req, N, S, Res> Service<RequestArgs<Message<PeerMessage<Req>>, Res, N>> for PeerHander<S>
where
    S: Service<RequestArgs<Message<Req>, Res, N>, Response = Res> + Clone + 'static + Send,
    N: 'static + Send,
    Req: 'static + Send,
    Res: 'static + Send,
{
    type Response = PeerMessage<Res>;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>> + Send>>;

    fn call(
        &mut self,
        RequestArgs {
            request: request_1,
            node,
            id,
            input,
        }: RequestArgs<message::Message<PeerMessage<Req>>, Res, N>,
    ) -> Self::Future {
        let mut this = self.clone();
        let (message, peer_message) = request_1.split();
        let (peer_message, body) = peer_message.split();
        Box::pin(async move {
            let response = this
                .inner
                .call(RequestArgs {
                    request: message.with_body(body),
                    node,
                    id,
                    input,
                })
                .await?;
            Ok(peer_message.into_reply(id).0.with_body(response))
        })
    }
}

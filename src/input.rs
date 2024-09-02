use std::{fmt::Debug, future::Future, pin::Pin};

use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    handler::RequestType,
    message::{Message, PeerMessage},
    service::Service,
};

pub struct InputHandler<Req, Res>(std::marker::PhantomData<Req>, std::marker::PhantomData<Res>);

impl<Req, Res> InputHandler<Req, Res> {
    #[must_use]
    pub fn new() -> Self {
        Self(std::marker::PhantomData, std::marker::PhantomData)
    }
}

impl<Req, Res> Default for InputHandler<Req, Res> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Req, Res> Service<String> for InputHandler<Req, Res>
where
    Req: DeserializeOwned + 'static + Send,
    Res: DeserializeOwned + 'static + Send,
{
    type Response = InputResponse<Req, Res>;

    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response>> + Send>>;

    fn call(&mut self, request: String) -> Self::Future {
        let input: InputType<Req, Res> = serde_json::from_str(&request).unwrap();
        match input {
            InputType::Request(req) => Box::pin(async move { Ok(InputResponse::NewHandler(req)) }),
            InputType::Response(res) => {
                let id = res.body.dest.unwrap();
                Box::pin(async move { Ok(InputResponse::HandlerMessage { id, message: res }) })
            }
        }
    }
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum InputType<Req, Res> {
    Request(RequestType<Req>),
    Response(Message<PeerMessage<Res>>),
}
#[derive(Debug)]
pub enum InputResponse<Req, Res> {
    NewHandler(RequestType<Req>),
    HandlerMessage {
        id: usize,
        message: Message<PeerMessage<Res>>,
    },
}

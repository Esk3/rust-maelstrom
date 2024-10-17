use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tokio::io::AsyncBufReadExt;

use crate::{
    message::{InitRequest, InitResponse, Message},
    service::{self, Service},
    Node,
};

pub async fn default_handler<N, Req>(
) -> NodeHandler<service::json::JsonLayer<service::event::EventLayer<N, Req>, Message<Req>>>
where
    N: Node,
    Req: service::event::AsBodyId + Debug,
{
    let node = NodeHandler::<()>::init_node::<N, ()>(()).await;
    let event_broker = service::event::EventBroker::new();
    let service = service::json::JsonLayer::<_, Message<Req>>::new(
        service::event::EventLayer::new(node, event_broker),
    );
    NodeHandler::new(service)
}

pub struct NodeHandler<S> {
    service: S,
}
impl<S> NodeHandler<S> {
    pub async fn init_node_with_io<N, R, W, State>(reader: R, writer: W, state: State) -> N
    where
        N: Node<State>,
        R: tokio::io::AsyncRead + Unpin,
        W: std::io::Write,
    {
        let line = tokio::io::BufReader::new(reader)
            .lines()
            .next_line()
            .await
            .unwrap()
            .unwrap();
        let init_message = serde_json::from_str::<Message<InitRequest>>(&line).unwrap();
        let (reply, body) = init_message.into_reply();
        let InitRequest::Init {
            msg_id,
            node_id,
            node_ids,
        } = body;
        let node = N::init(node_id, node_ids, state);

        let response = InitResponse::InitOk {
            in_reply_to: msg_id,
        };
        reply.with_body(response).send(writer).unwrap();
        node
    }
    pub async fn init_node<N, State>(state: State) -> N
    where
        N: Node<State>,
    {
        Self::init_node_with_io(tokio::io::stdin(), std::io::stdout().lock(), state).await
    }
    pub fn new(service: S) -> Self {
        Self { service }
    }
    pub async fn run_with_io<R, W, Res>(self, reader: R, writer: W)
    where
        R: tokio::io::AsyncRead + Unpin,
        S: Service<String, Response = Option<Res>> + Clone + Send + 'static,
        Res: Serialize + Send + Debug,
        W: std::io::Write + Send + 'static,
    {
        let mut lines = tokio::io::BufReader::new(reader).lines();
        let output = Arc::new(Mutex::new(writer));
        while let Ok(Some(line)) = lines.next_line().await {
            let mut this = self.service.clone();
            let output = Arc::clone(&output);
            // TODO joinset to hold handles?
            let _handle = tokio::spawn(async move {
                let response = this.call(line).await.unwrap();
                if let Some(res) = response {
                    let mut lock = output.lock().unwrap();
                    let s = serde_json::to_string(&res).unwrap();
                    lock.write_all(s.as_bytes()).unwrap();
                    lock.write_all(b"\n").unwrap();
                    lock.flush().unwrap();
                }
            });
        }
    }
    pub async fn run<Res>(self)
    where
        S: Service<String, Response = Option<Res>> + Send + Clone + 'static,
        Res: Serialize + Send + Debug,
    {
        self.run_with_io(tokio::io::stdin(), std::io::stdout())
            .await;
    }
}

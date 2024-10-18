use anyhow::Context;
use rust_maelstrom::{
    //event_queue::EventQueue,
    message::{InitRequest, Message},
};
use serde::Deserialize;
use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    todo!()
    //init().await?;
    //
    //let event_queue = EventQueue::new();
    //
    //{
    //    let event_queue = event_queue.clone();
    //    tokio::spawn(async move {
    //        let stdin = tokio::io::stdin();
    //        let mut lines = tokio::io::BufReader::new(stdin).lines();
    //        while let Ok(Some(line)) = lines.next_line().await {
    //            let message = serde_json::from_str::<Message<Request>>(&line).unwrap();
    //            event_queue.publish_message(Topic::Message, message);
    //        }
    //    });
    //}
    //
    //loop {
    //    event_queue.await_topic(Topic::Message).await;
    //    for message in event_queue.get_messages(&Topic::Message) {}
    //}
    //Ok(())
}

struct EchoNode {}

impl EchoNode {
    pub fn init() -> Self {
        Self {}
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
enum Topic {
    Message,
}
#[derive(Debug, Deserialize, Clone)]
enum Request {
    Echo {
        echo: serde_json::Value,
        msg_id: usize,
    },
}
enum Response {
    EchoOk {
        echo: serde_json::Value,
        in_reply_to: usize,
    },
}

async fn init() -> anyhow::Result<()> {
    let reader = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(reader).lines();

    let init_line = lines
        .next_line()
        .await
        .context("Failed to read init line")?
        .context("Init line missing")?;
    let init_message = serde_json::from_str::<Message<InitRequest>>(&init_line)
        .with_context(|| format!("failed to parse init line from {init_line}"))?;
    let (msg, body) = init_message.split();
    let InitRequest::Init {
        msg_id,
        node_id,
        node_ids,
    } = body;
    let (reply, _) = msg.into_reply();
    reply
        .with_body(rust_maelstrom::message::InitResponse::InitOk {
            in_reply_to: msg_id,
        })
        .send(std::io::stdout())?;
    Ok(())
}

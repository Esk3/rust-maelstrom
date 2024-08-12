use std::io::Write;

use message::{InitRequest, InitResponse, Message, MessageRequest, MessageResponse};
use tokio::io::AsyncBufReadExt;

pub mod message;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    let init_line = lines.next_line().await.unwrap().unwrap();
    let init_message: Message<InitRequest> = serde_json::from_str(&init_line).unwrap();

    let node = Node::init();

    {
        let mut output = std::io::stdout().lock();
        let InitRequest::Init { msg_id, .. } = init_message.body;
        let message = Message {
            src: init_message.dest,
            dest: init_message.src,
            body: InitResponse::InitOk {
                in_reply_to: msg_id,
            },
        };
        serde_json::to_writer(&mut output, &message);
        output.write_all(b"\n").unwrap();
    }

    let mut set = tokio::task::JoinSet::new();
    let mut connections = std::collections::HashMap::new();
    let mut next_id = 1;
    let state = std::sync::Arc::new(std::sync::Mutex::new(Node { id: 1 }));

    loop {
        tokio::select! {
            Ok(Some(line)) = lines.next_line() => {
                handle_input(line, state.clone(), &mut set, &mut connections, &mut next_id).await;
            },
            Some(join_handler) = set.join_next() => {
                let (_msg, id) = join_handler.unwrap();
                connections.remove(&id);
            }
        }
    }
}

async fn handle_input(
    input: String,
    state: std::sync::Arc<std::sync::Mutex<Node>>,
    set: &mut tokio::task::JoinSet<(String, usize)>,
    connections: &mut std::collections::HashMap<usize, tokio::sync::mpsc::UnboundedSender<String>>,
    next_id: &mut usize,
) {
    let message: Message<MessageRequest> = serde_json::from_str(&input).unwrap();
    let id = *next_id;
    *next_id += 1;
    dbg!(&message);
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    set.spawn(async move { (handle_message(message, state, id, rx).await, id) });

    // match message {
    //     Message::New(_) => {
    //         let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    //         connections.insert(id, tx);
    //         set.spawn(async move { (handle_message(message, state, id, rx).await, id) });
    //     }
    //     Message::Reply { id, msg } => {
    //         let Some(tx) = connections.get(&id) else {
    //             return;
    //         };
    //         tx.send(msg).unwrap()
    //     }
    // };
}

async fn handle_message(
    msg: Message<MessageRequest>,
    s: std::sync::Arc<std::sync::Mutex<Node>>,
    id: usize,
    mut input: tokio::sync::mpsc::UnboundedReceiver<String>,
) -> String {
    dbg!("handing message: ", &msg);
    match msg.body {
        MessageRequest::Echo { echo, msg_id } => {
            let message = Message {
                src: msg.dest,
                dest: msg.src,
                body: MessageResponse::EchoOk {
                    echo,
                    in_reply_to: msg_id,
                },
            };
            let mut output = std::io::stdout().lock();
            serde_json::to_writer(&mut output, &message).unwrap();
            output.write_all(b"\n").unwrap();
        }
    }
    "Ok".to_string()
}

struct Node {
    pub id: usize,
}

impl Node {
    pub fn init() -> Self {
        Self {
            id: 1
        }
    }
    pub fn handle_message(&mut self) {}
}

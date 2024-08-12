use message::{InitRequest, Message as MaelstromMessage, MessageRequest};
use tokio::io::AsyncBufReadExt;

pub mod message;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    let init_line = lines.next_line().await.unwrap().unwrap();
    let init_message: MaelstromMessage<InitRequest> = serde_json::from_str(&init_line).unwrap();

    let node = Node::init();

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
    let message: MaelstromMessage<MessageRequest> = serde_json::from_str(&input).unwrap();
    let id = *next_id;
    *next_id += 1;
    dbg!(&message);
    match message {
        Message::New(_) => {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            connections.insert(id, tx);
            set.spawn(async move { (handle_message(message, state, id, rx).await, id) });
        }
        Message::Reply { id, msg } => {
            let Some(tx) = connections.get(&id) else {
                return;
            };
            tx.send(msg).unwrap()
        }
    };
}

async fn handle_message(
    msg: Message,
    s: std::sync::Arc<std::sync::Mutex<Node>>,
    id: usize,
    mut input: tokio::sync::mpsc::UnboundedReceiver<String>,
) -> String {
    dbg!("handing message: ", &msg);
    match msg {
        Message::New(msg) => match msg.as_str() {
            "sleep" => {
                {
                    let lock = s.lock().unwrap();
                    dbg!(lock.id);
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            "print" => {
                let _lock = s.lock().unwrap();
                dbg!("hello world");
            }
            "reply" => {
                dbg!("awaiting reply on", id);
                let reply = input.recv().await.unwrap();
                dbg!("got reply: ", reply);
                return "Ok Reply.".to_string();
            }
            _ => {
                dbg!("unknown command");
            }
        },
        Message::Reply { id, msg } => todo!(),
    }
    "Ok".to_string()
}

struct Node {
    pub id: usize,
}

impl Node {
    pub fn init() -> Self {
        todo!()
    }
    pub fn handle_message(&mut self) {}
}

#[derive(Debug)]
enum Message {
    New(String),
    Reply { id: usize, msg: String },
}

impl Message {
    pub fn new(s: String) -> Self {
        if let Some(reply) = Self::parse_reply(&s) {
            reply
        } else {
            Self::New(s)
        }
    }
    fn parse_reply(s: &str) -> Option<Self> {
        let mut sp = s.splitn(3, ' ');
        if sp.next() != Some("conn") {
            return None;
        }
        if let Some(id) = sp.next() {
            let id = id.parse().ok()?;
            Some(Self::Reply {
                id,
                msg: sp.next()?.to_string(),
            })
        } else {
            None
        }
    }
}

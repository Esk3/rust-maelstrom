use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    let mut set = tokio::task::JoinSet::new();
    let mut connections = std::collections::HashMap::new();
    let mut next_id = 1;
    let state = std::sync::Arc::new(std::sync::Mutex::new(Node { id: 1 }));

    loop {
        tokio::select! {
            Ok(Some(line)) = lines.next_line() => {
                handle_input(line, state.clone(), &mut set, &mut connections, &mut next_id).await;
            },
            Some(t) = set.join_next() => {
                let t = t.unwrap();
                let id = t.1;
                connections.remove(&id);
                dbg!("task done", t);
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
    let message = Message::new(input);
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

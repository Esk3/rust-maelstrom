use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();

    let mut set = tokio::task::JoinSet::new();
    let mut s = "hey";
    let state = std::sync::Arc::new(std::sync::Mutex::new(Node { id: 1 }));

    loop {
        tokio::select! {
            Ok(Some(line)) = lines.next_line() => {
                handle_input(line, &state, &mut set).await;
            },
            Some(t) = set.join_next() => {
                dbg!("task done", t);
            }
        }
    }
}

async fn handle_input(
    input: String,
    state: &std::sync::Arc<std::sync::Mutex<Node>>,
    set: &mut tokio::task::JoinSet<String>,
) {
    let line = input;
    let message = Message::new(line);
    dbg!(&message);
    let state = state.clone();
    set.spawn(async move { handle_message(message, state).await });
}

async fn handle_message(msg: Message, s: std::sync::Arc<std::sync::Mutex<Node>>) -> String {
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

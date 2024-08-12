use std::io::BufRead;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let stdin = std::io::stdin().lock();
        let lines = stdin.lines();
        for line in lines {
            let line = line.unwrap();
            tx.send(line).unwrap();
        }
    });

    let mut set = tokio::task::JoinSet::new();
    let mut s = "hey";
    let state = std::sync::Arc::new(std::sync::Mutex::new(Node { id: 1 }));

    loop {
        tokio::select! {
            msg = rx.recv() => {
                let state = state.clone();
                set.spawn(async move {
                    let msg = msg.unwrap();
                    handle_message(&msg, state).await
                });
            }
            Some(t) = set.join_next() => {
                dbg!("task done", t);
            }
        }
    }
}

async fn handle_message(msg: &str, s: std::sync::Arc<std::sync::Mutex<Node>>) -> String {
    dbg!("handing message: ", &msg);
    match msg {
        "sleep" => {
            {
                let lock = s.lock().unwrap();
                dbg!(lock.id);
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        "print" => {
            let lock = s.lock().unwrap();
            dbg!("hello world");
        }
        _ => {
            dbg!("unknown command");
        }
    }
    "Ok".to_string()
}

struct Node {
    pub id: usize,
}

impl Node {
    pub fn handle_message(&mut self) {}
}

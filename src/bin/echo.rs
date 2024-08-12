#[tokio::main]
async fn main(){
}

struct EchoNode {
    pub id: String,
}

impl Node for EchoNode {
    fn init(node_id: String, node_ids: Vec<String>) -> Self {
        Self { id: node_id }
    }
}

async fn handle_message(
    msg: Message<MessageRequest>,
    node: std::sync::Arc<std::sync::Mutex<EchoNode>>,
    id: usize,
    mut input: tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    dbg!("handing message: ", &msg);
    let (reply, body) = msg.into_reply();
    match body {
        MessageRequest::Echo { echo, msg_id } => {
            let body = MessageResponse::EchoOk {
                echo,
                in_reply_to: msg_id,
            };
            reply.with_body(body).send(&mut std::io::stdout().lock());
        }
    }
}

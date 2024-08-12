use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Message<T> {
    pub src: String,
    pub dest: String,
    pub body: T,
}

impl<T> Message<T> {
    pub fn into_reply(self) -> (Message<()>, T) {
        let body = self.body;
        (
            Message {
                src: self.dest,
                dest: self.src,
                body: (),
            },
            body,
        )
    }
    pub fn with_body<B>(self, body: B) -> Message<B> {
        Message {
            src: self.src,
            dest: self.dest,
            body,
        }
    }
}

impl<T> Message<T>
where
    T: Serialize,
{
    pub fn send<W>(&self, mut writer: W)
    where
        W: std::io::Write,
    {
        serde_json::to_writer(&mut writer, self).unwrap();
        writer.write_all(b"\n").unwrap();
    }
}


#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MessageType<M, P, R> {
    Request(Request<M, P>),
    Response(Message<PeerMessage<R>>)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Request<M, P> {
    Maelstrom(Message<M>),
    Peer(Message<PeerMessage<P>>)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerMessage<T> {
    pub src: usize,
    pub dest: Option<usize>,
    pub body: T
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InitRequest {
    Init {
        msg_id: usize,
        node_id: String,
        node_ids: Vec<String>,
    },
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InitResponse {
    InitOk { in_reply_to: usize },
}

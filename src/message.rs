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
    Response(Message<PeerMessage<R>>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Request<M, P> {
    Maelstrom(Message<M>),
    Peer(Message<PeerMessage<P>>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerMessage<T> {
    pub src: usize,
    pub dest: Option<usize>,
    pub id: usize,
    pub body: T,
}

impl<T> PeerMessage<T> {
    pub fn into_reply(self, src: usize) -> (PeerMessage<()>, T) {
        let body = self.body;
        (
            PeerMessage {
                src,
                dest: Some(self.src),
                id: self.id,
                body: (),
            },
            body,
        )
    }
    pub fn with_body<B>(self, body: B) -> PeerMessage<B> {
        PeerMessage {
            src: self.src,
            dest: self.dest,
            id: self.id,
            body,
        }
    }
    pub fn matches_response<B>(&self, res: &Message<PeerMessage<B>>) -> bool {
        res.body.id == self.id
    }
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

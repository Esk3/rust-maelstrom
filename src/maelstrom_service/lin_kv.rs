use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error;
use crate::message::Message;
use crate::service::event::{AsBodyId, EventBroker};

impl<T, W> Clone for LinKvClient<T, W> {
    fn clone(&self) -> Self {
        Self {
            src: Arc::clone(&self.src),
            event_broker: self.event_broker.clone(),
            w: Arc::clone(&self.w),
        }
    }
}

#[derive(Debug)]
pub struct LinKvClient<T, W> {
    src: Arc<Mutex<String>>,
    event_broker: EventBroker<LinKvInput<T>>,
    w: Arc<Mutex<W>>,
}

impl<T, W> LinKvClient<T, W>
where
    T: Debug,
    W: std::io::Write,
{
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(src: impl ToString, w: Arc<Mutex<W>>) -> Self {
        Self {
            src: Arc::new(Mutex::new(src.to_string())),
            event_broker: EventBroker::new(),
            w,
        }
    }
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_src(&self, src: impl ToString) {
        *self.src.lock().unwrap() = src.to_string();
    }
    pub async fn read<K>(&self, key: K, id: usize) -> Result<Option<T>, error::Error>
    where
        K: Serialize,
        T: Serialize,
        W: std::io::Write,
    {
        let response = self.send_read(key, id).await;
        match response.unwrap().body {
            LinKvInput::ReadOk {
                value,
                in_reply_to: _,
            } => Ok(Some(value)),
            LinKvInput::WriteOk { in_reply_to } => todo!(),
            LinKvInput::CasOk { in_reply_to } => todo!(),
            LinKvInput::Error {
                code,
                text,
                in_reply_to,
            } if error::ErrorCode::from(code) == error::ErrorCode::KeyDoesNotExist => Ok(None),
            LinKvInput::Error {
                code,
                text,
                in_reply_to,
            } => Err(error::Error::new(code, text, in_reply_to)),
        }
    }

    pub fn send_message<K>(
        &self,
        body: LinKvOutput<K, T>,
    ) -> tokio::sync::oneshot::Receiver<Message<LinKvInput<T>>>
    where
        K: Serialize,
        T: Serialize,
        W: std::io::Write,
    {
        let id = body.as_body_id();
        let src = self.src.lock().unwrap().to_string();
        let message = Message {
            src: src.to_string(),
            dest: "lin-kv".to_string(),
            body,
        };
        let id = crate::service::event::MessageId {
            this: src,
            other: "lin-kv".to_string(),
            id,
        };
        let rx = self.event_broker.subsribe_to_id(id);
        message.send(&mut *self.w.lock().unwrap()).unwrap();
        rx
    }

    pub fn send_read<K>(
        &self,
        key: K,
        msg_id: usize,
    ) -> tokio::sync::oneshot::Receiver<Message<LinKvInput<T>>>
    where
        K: Serialize,
        T: Serialize,
        W: std::io::Write,
    {
        self.send_message(LinKvOutput::Read { key, msg_id })
    }

    pub fn send_write<K>(
        &self,
        key: K,
        value: T,
        msg_id: usize,
    ) -> tokio::sync::oneshot::Receiver<Message<LinKvInput<T>>>
    where
        K: Serialize,
        T: Serialize,
        W: std::io::Write,
    {
        self.send_message(LinKvOutput::Write { key, value, msg_id })
    }

    pub fn send_cas<K>(
        &self,
        key: K,
        from: T,
        to: T,
        msg_id: usize,
    ) -> tokio::sync::oneshot::Receiver<Message<LinKvInput<T>>>
    where
        K: Serialize,
        T: Serialize,
        W: std::io::Write,
    {
        self.send_message(LinKvOutput::Cas {
            key,
            from,
            to,
            msg_id,
        })
    }

    pub async fn cas<K>(&self, key: K, from: T, to: T, msg_id: usize) -> Result<(), error::Error>
    where
        K: Serialize,
        T: Serialize,
        W: std::io::Write,
    {
        match self.send_cas(key, from, to, msg_id).await.unwrap().body {
            LinKvInput::ReadOk { value, in_reply_to } => todo!(),
            LinKvInput::WriteOk { in_reply_to } => todo!(),
            LinKvInput::CasOk { in_reply_to } => Ok(()),
            LinKvInput::Error {
                code,
                text,
                in_reply_to,
            } => todo!(),
        }
    }

    pub fn handle_message(&self, message: Message<LinKvInput<T>>) {
        let id = crate::service::event::MessageId {
            this: message.dest.to_string(),
            other: message.src.to_string(),
            id: message.body.as_body_id(),
        };
        self.event_broker.publish_or_store_id(id, message);
    }
}

//#[derive(Debug)]
//pub struct LinKv<T: Clone + EventId> {
//    node_id: String,
//    event_broker: EventBroker<T>,
//}
//
//impl<T> LinKv<T>
//where
//    T: Debug + Send + Clone + EventId + 'static,
//{
//    #[must_use]
//    pub fn new(node_id: String, event_broker: EventBroker<T>) -> Self {
//        Self {
//            node_id,
//            event_broker,
//        }
//    }
//
//    pub async fn read<K, V>(&self, key: K) -> Result<V, error::Error>
//    where
//        K: Serialize,
//        V: Serialize,
//        T: ExtractInput<ReturnValue = V>,
//    {
//        let msg_id = self.event_broker.get_id_counter().next_id();
//        let message = Message {
//            src: self.node_id.clone(),
//            dest: "lin-kv".to_string(),
//            body: LinKvOutput::<K, V>::Read { key, msg_id },
//        };
//        let listner = self.event_broker.subscribe(msg_id);
//        message
//            .send(std::io::stdout())
//            .map_err(|_| error::Error::crash(msg_id))?;
//        let response = listner
//            .await
//            .map_err(|_| error::Error::crash_with_message("event broker dropped event", msg_id))?;
//
//        match response {
//            Event::Injected(Message { body, .. }) => match body.extract_input() {
//                Some(LinKvInput::ReadOk {
//                    value,
//                    in_reply_to: _,
//                }) => Ok(value),
//                Some(LinKvInput::WriteOk { in_reply_to }) => Err(error::Error::crash(in_reply_to)),
//                Some(LinKvInput::Error {
//                    code,
//                    text,
//                    in_reply_to,
//                }) => Err(error::Error::new(code, text, in_reply_to)),
//                Some(LinKvInput::CasOk { in_reply_to }) => Err(error::Error::crash_with_message(
//                    "expected: read_ok got: cas_ok",
//                    in_reply_to,
//                )),
//                None => Err(error::Error::crash_with_message(
//                    "failed to extract lin kv reply",
//                    /* TODO in_reply_to. use get_event_id() ?  */ 0,
//                )),
//            },
//            // TODO in_reply_to
//            Event::Maelstrom(_) => Err(error::Error::new(ErrorCode::NotSupported, "", 0)),
//        }
//    }
//
//    pub async fn read_or_default<K, V>(&self, key: K) -> Result<V, error::Error>
//    where
//        K: Serialize + Clone,
//        V: Serialize + Default,
//        T: ExtractInput<ReturnValue = V>,
//    {
//        match self.read(key.clone()).await {
//            Ok(value) => Ok(value),
//            Err(err) if matches!(err.code(), error::ErrorCode::KeyDoesNotExist) => {
//                self.write(key, V::default()).await.map(|()| V::default())
//            }
//            Err(err) => Err(err),
//        }
//    }
//
//    pub async fn write<K, V>(&self, key: K, value: V) -> Result<(), error::Error>
//    where
//        K: Serialize,
//        V: Serialize,
//        T: ExtractInput<ReturnValue = V>,
//    {
//        let msg_id = self.event_broker.get_id_counter().next_id();
//        let message = Message {
//            src: self.node_id.clone(),
//            dest: "lin-kv".to_string(),
//            body: LinKvOutput::Write { key, value, msg_id },
//        };
//        let listner = self.event_broker.subscribe(msg_id);
//        message
//            .send(std::io::stdout())
//            .map_err(|_| error::Error::crash_with_message("error writing to stdout", msg_id))?;
//        match listner.await {
//            Ok(Event::Injected(message)) => match message.body.extract_input() {
//                Some(LinKvInput::WriteOk { in_reply_to: _ }) => Ok(()),
//                Some(
//                    LinKvInput::ReadOk { in_reply_to, .. } | LinKvInput::Error { in_reply_to, .. },
//                ) => Err(error::Error::crash_with_message(
//                    "expected write ok",
//                    in_reply_to,
//                )),
//                Some(LinKvInput::CasOk { in_reply_to }) => Err(error::Error::crash_with_message(
//                    "expected: write_ok. got: cas_ok",
//                    in_reply_to,
//                )),
//                None => todo!(),
//            },
//            Ok(Event::Maelstrom(_)) => Err(error::Error::crash_with_message(
//                "expected injected message",
//                /* TODO in_reply_to  */ 0,
//            )),
//            Err(e) => Err(error::Error::crash_with_message(
//                format!("recv dropped?: {e:?}"),
//                /* TODO in_reply_to */ 0,
//            )),
//        }
//    }
//    pub async fn cas<K, V>(&self, key: K, from: V, to: V) -> Result<(), error::Error>
//    where
//        K: Serialize,
//        V: Serialize,
//        T: ExtractInput<ReturnValue = V>,
//    {
//        let msg_id = self.event_broker.get_id_counter().next_id();
//        let message = Message {
//            src: self.node_id.clone(),
//            dest: "lin-kv".to_string(),
//            body: LinKvOutput::Cas {
//                key,
//                from,
//                to,
//                msg_id,
//            },
//        };
//        let listner = self.event_broker.subscribe(msg_id);
//        message
//            .send(std::io::stdout())
//            // TODO msg_id is wrong.
//            .map_err(|_| error::Error::crash_with_message("error writing to stdout", msg_id))?;
//        match listner.await {
//            Ok(Event::Injected(message)) => match message.body.extract_input() {
//                Some(LinKvInput::CasOk { in_reply_to: _ }) => Ok(()),
//                Some(LinKvInput::Error {
//                    code,
//                    text,
//                    in_reply_to,
//                }) => Err(error::Error::new(code, text, in_reply_to)),
//                Some(
//                    LinKvInput::WriteOk { in_reply_to } | LinKvInput::ReadOk { in_reply_to, .. },
//                ) => Err(error::Error::crash_with_message(
//                    "expected write ok",
//                    in_reply_to,
//                )),
//                None => todo!(),
//            },
//            Ok(Event::Maelstrom(_)) => Err(error::Error::crash_with_message(
//                "expected injected message",
//                /* TODO in_reply_to  */ 0,
//            )),
//            Err(e) => Err(error::Error::crash_with_message(
//                format!("recv dropped?: {e:?}"),
//                /* TODO in_reply_to */ 0,
//            )),
//        }
//    }
//}
//
//pub trait ExtractInput {
//    type ReturnValue;
//    fn extract_input(self) -> Option<LinKvInput<Self::ReturnValue>>;
//}
//
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinKvInput<T> {
    ReadOk {
        value: T,
        in_reply_to: usize,
    },
    WriteOk {
        in_reply_to: usize,
    },
    CasOk {
        in_reply_to: usize,
    },
    Error {
        code: usize,
        text: String,
        in_reply_to: usize,
    },
}

impl<T> AsBodyId for LinKvInput<T> {
    fn as_raw_id(&self) -> String {
        match self {
            LinKvInput::ReadOk {
                value: _,
                in_reply_to,
            }
            | LinKvInput::WriteOk { in_reply_to }
            | LinKvInput::CasOk { in_reply_to }
            | LinKvInput::Error {
                code: _,
                text: _,
                in_reply_to,
            } => in_reply_to.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinKvOutput<K, V> {
    Read {
        key: K,
        msg_id: usize,
    },
    Write {
        key: K,
        value: V,
        msg_id: usize,
    },
    Cas {
        key: K,
        from: V,
        to: V,
        msg_id: usize,
    },
}

impl<K, V> AsBodyId for LinKvOutput<K, V> {
    fn as_raw_id(&self) -> String {
        match self {
            LinKvOutput::Read { key: _, msg_id }
            | LinKvOutput::Write {
                key: _,
                value: _,
                msg_id,
            }
            | LinKvOutput::Cas {
                key: _,
                from: _,
                to: _,
                msg_id,
            } => msg_id.to_string(),
        }
    }
}

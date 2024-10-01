use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event<T> {
    pub id: usize,
    pub body: T,
}

#[derive(Debug, Clone)]
pub struct EventQueue<K, V> {
    topics: Arc<Mutex<HashMap<K, Vec<Event<V>>>>>,
    topic_offset: Arc<Mutex<HashMap<K, usize>>>,
    topic_notifications: Arc<Mutex<HashMap<K, Vec<tokio::sync::mpsc::UnboundedSender<()>>>>>,
}

impl<K, V> EventQueue<K, V>
where
    K: Hash + Eq + PartialEq + Clone + Debug,
    V: Clone + Debug,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            topics: Arc::default(),
            topic_offset: Arc::default(),
            topic_notifications: Arc::default(),
        }
    }

    pub fn publish_message(&self, topic_name: K, message: V) {
        let mut topics = self.topics.lock().unwrap();
        let mut notifiers = self.topic_notifications.lock().unwrap();
        let listners = notifiers.get_mut(&topic_name);
        let topic = topics.entry(topic_name).or_default();
        topic.push(Event {
            id: topic.len(),
            body: message,
        });
        if let Some(listners) = listners {
            listners.retain(|tx| tx.send(()).is_ok());
        }
    }

    pub fn get_messages(&self, topic: &K) -> Vec<Event<V>> {
        let topics = self.topics.lock().unwrap();
        let Some(messages) = topics.get(topic) else {
            return Vec::new();
        };
        let offsets = self.topic_offset.lock().unwrap();
        let offset = offsets.get(topic).unwrap_or(&0);
        messages[*offset..].to_vec()
    }

    pub fn get_listner(&self, topic: K) -> tokio::sync::mpsc::UnboundedReceiver<()> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.topic_notifications
            .lock()
            .unwrap()
            .entry(topic)
            .or_default()
            .push(tx);
        rx
    }

    pub async fn await_topic(&self, topic: K) {
        self.get_listner(topic).recv().await;
    }

    pub fn commit_offset(&self, topic_key: K, offset: usize) {
        let binding = self.topics.lock().unwrap();
        let Some(topic) = binding.get(&topic_key) else {
            return;
        };
        let Some(pos) = topic.iter().position(|t| t.id == offset) else {
            return;
        };
        let mut offsets = self.topic_offset.lock().unwrap();
        *offsets.entry(topic_key).or_insert(0) = pos + 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::message::Message;

    use super::*;

    #[test]
    fn publishing_messages() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::new();
        assert!(event_queue.get_messages(&topic).is_empty());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic)[0].body,
            serde_json::Value::String("hello world".to_string())
        );
    }

    #[test]
    fn publishing_more_messages() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::new();
        assert!(event_queue.get_messages(&topic).is_empty());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic)[0].body,
            serde_json::Value::String("hello world".to_string())
        );
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("my other message".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic),
            vec![
                Event {
                    id: 0,
                    body: serde_json::Value::String("hello world".to_string()),
                },
                Event {
                    id: 1,
                    body: serde_json::Value::String("my other message".to_string())
                }
            ]
        );
    }

    #[test]
    fn commit_offset() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::new();
        assert!(event_queue.get_messages(&topic).is_empty());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        let msg = &event_queue.get_messages(&topic)[0];
        assert_eq!(
            msg.body,
            serde_json::Value::String("hello world".to_string())
        );
        event_queue.commit_offset(topic.clone(), msg.id);
        assert!(dbg!(event_queue.get_messages(&topic)).is_empty());
    }

    #[tokio::test]
    async fn await_topic_timeout() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<String, ()>::new();

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            event_queue.await_topic(topic),
        )
        .await;

        assert!(timeout.is_err());
    }

    #[tokio::test]
    async fn await_topic_test() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::new();

        {
            let event_queue = event_queue.clone();
            let topic = topic.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                event_queue.publish_message(
                    topic.clone(),
                    serde_json::Value::String("wait on this".to_string()),
                );
            });
        }

        assert!(event_queue.get_messages(&topic).is_empty());

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(6),
            event_queue.await_topic(topic.clone()),
        )
        .await;
        assert!(timeout.is_ok());

        assert_eq!(
            event_queue.get_messages(&topic)[0].body,
            serde_json::Value::String("wait on this".to_string())
        );
    }

    #[tokio::test]
    async fn test() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::new();
        assert!(event_queue.get_messages(&topic).is_empty());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        let msg = &event_queue.get_messages(&topic)[0];
        assert_eq!(
            msg.body,
            serde_json::Value::String("hello world".to_string())
        );
        event_queue.commit_offset(topic.clone(), msg.id);
        assert!(event_queue.get_messages(&topic).is_empty());

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            event_queue.await_topic(topic.clone()),
        )
        .await;

        assert!(timeout.is_err());

        {
            let event_queue = event_queue.clone();
            let topic = topic.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                event_queue.publish_message(
                    topic.clone(),
                    serde_json::Value::String("wait on this".to_string()),
                );
            });
        }

        assert!(event_queue.get_messages(&topic).is_empty());

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(4),
            event_queue.await_topic(topic.clone()),
        )
        .await;
        assert!(timeout.is_ok());

        assert!(!event_queue.get_messages(&topic).is_empty());
    }

    #[test]
    fn messages_test() {
        let topic = "mytopic".to_string();
        let event_queue = EventQueue::new();

        let msg = Message {
            src: "none".to_string(),
            dest: "none".to_string(),
            body: (),
        };

        let msg = serde_json::value::to_value(msg).unwrap();

        event_queue.publish_message(topic, msg);
    }

    #[test]
    fn use_test() {
        #[derive(Debug, Clone, Hash, PartialEq, Eq)]
        enum Types {
            Msg,
            Event,
            Other,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        enum Payload {
            Msg(String),
            Event(String),
        }

        let event_queue = EventQueue::<Types, Payload>::new();

        event_queue.publish_message(Types::Msg, Payload::Msg("hello".to_string()));
        event_queue.publish_message(Types::Msg, Payload::Msg("what is up".to_string()));

        let msgs = event_queue.get_messages(&Types::Msg);

        for Event { id, body } in msgs {
            let Payload::Msg(msg) = body else {
                event_queue.commit_offset(Types::Msg, id);
                continue;
            };
            if msg.contains("what") {
                event_queue.publish_message(Types::Event, Payload::Event(msg));
            } else {
                event_queue.publish_message(Types::Other, Payload::Event(msg));
            }
            event_queue.commit_offset(Types::Msg, id);
        }

        assert!(dbg!(event_queue.get_messages(&Types::Msg)).is_empty());

        assert_eq!(
            event_queue.get_messages(&Types::Event)[0].body,
            Payload::Event("what is up".to_string())
        );

        assert_eq!(
            event_queue.get_messages(&Types::Other)[0].body,
            Payload::Event("hello".to_string())
        );
    }
}

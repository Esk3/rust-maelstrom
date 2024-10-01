use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    sync::{Arc, Mutex},
};

use crate::message::Message;

#[derive(Debug, Clone)]
pub struct EventQueue<K, V> {
    topics: Arc<Mutex<HashMap<K, Vec<V>>>>,
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
        topic.push(message);
        if let Some(listners) = listners {
            listners.retain(|tx| tx.send(()).is_ok());
        }
    }

    pub fn get_messages(&self, topic: &K) -> Option<Vec<V>> {
        let topics = self.topics.lock().unwrap();
        let Some(messages) = topics.get(topic) else {
            return None;
        };
        let offsets = self.topic_offset.lock().unwrap();
        let offset = offsets.get(topic).unwrap_or(&0);
        Some(messages.iter().skip(*offset).cloned().collect())
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

    pub fn commit_offset(&self, topic: K, offset: usize) {
        let mut offsets = self.topic_offset.lock().unwrap();
        *offsets.entry(topic).or_insert(0) = offset;
    }
}

#[cfg(test)]
mod tests {
    use crate::message::Message;

    use super::*;

    #[test]
    fn publishing_messages() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<(), _, _>::new();
        assert!(event_queue.get_messages(&topic).is_none());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic),
            Some(vec![serde_json::Value::String("hello world".to_string())])
        );
    }

    #[test]
    fn publishing_more_messages() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<(), _, _>::new();
        assert!(event_queue.get_messages(&topic).is_none());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic),
            Some(vec![serde_json::Value::String("hello world".to_string())])
        );
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("my other message".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic),
            Some(vec![
                serde_json::Value::String("hello world".to_string()),
                serde_json::Value::String("my other message".to_string())
            ])
        );
    }

    #[test]
    fn commit_offset() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<(), _, _>::new();
        assert!(event_queue.get_messages(&topic).is_none());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic),
            Some(vec![serde_json::Value::String("hello world".to_string())])
        );
        event_queue.commit_offset(topic.clone(), 1);
        assert!(event_queue.get_messages(&topic).unwrap().is_empty());
    }

    #[tokio::test]
    async fn await_topic_timeout() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<(), _, ()>::new();

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(10),
            event_queue.await_topic(&topic),
        )
        .await;

        assert!(timeout.is_err());
    }

    #[tokio::test]
    async fn await_topic_test() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<(), _, _>::new();

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

        assert!(event_queue.get_messages(&topic).is_none());

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(6),
            event_queue.await_topic(topic.clone()),
        )
        .await;
        assert!(timeout.is_ok());

        assert_eq!(
            event_queue.get_messages(&topic),
            Some(vec![serde_json::Value::String("wait on this".to_string())])
        );
    }

    #[tokio::test]
    async fn test() {
        let topic = "test_topic".to_string();
        let event_queue = EventQueue::<(), _, _>::new();
        assert!(event_queue.get_messages(&topic).is_none());
        event_queue.publish_message(
            topic.clone(),
            serde_json::Value::String("hello world".to_string()),
        );
        assert_eq!(
            event_queue.get_messages(&topic),
            Some(vec![serde_json::Value::String("hello world".to_string())])
        );
        event_queue.commit_offset(topic.clone(), 1);
        assert!(event_queue.get_messages(&topic).unwrap().is_empty());

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

        assert!(event_queue.get_messages(&topic).unwrap().is_empty());

        let timeout = tokio::time::timeout(
            std::time::Duration::from_millis(4),
            event_queue.await_topic(topic.clone()),
        )
        .await;
        assert!(timeout.is_ok());

        assert!(!event_queue.get_messages(&topic).unwrap().is_empty());
    }

    #[test]
    fn messages_test() {
        let topic = "mytopic".to_string();
        let queue = EventQueue::<(), _, _>::new();

        let msg = Message {
            src: "none".to_string(),
            dest: "none".to_string(),
            body: (),
        };

        let msg = serde_json::value::to_value(msg).unwrap();

        queue.publish_message(topic, msg);
    }
}

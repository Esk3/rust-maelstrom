#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    usize,
};

use anyhow::bail;

use crate::{
    message::Message,
    new_event::{Event, IntoBodyId, IntoMessageId, MessageId, SubscribeOption},
};

/// TODO timeout for long events?
pub struct EventWorker<I, T, Id> {
    new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(MessageId<Id>, SubscribeOption<I, T>)>,
    new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>,
    queue: VecDeque<Event<I, T>>,
    static_subscriptions: HashMap<MessageId<Id>, SubscribeOption<I, T>>,
    dynamic_subscriptions: Vec<(MessageId<Id>, SubscribeOption<I, T>)>,
}

impl<I, T, Id> EventWorker<I, T, Id>
where
    I: IntoBodyId<Id>,
    T: IntoBodyId<Id>,
    Id: Hash + Eq + PartialEq + Debug + Send + Sync,
    I: Debug,
    T: Debug,
    Id: Hash + PartialEq + Eq + Clone,
{
    #[must_use]
    pub fn new(
        new_subscriber_rx: tokio::sync::mpsc::UnboundedReceiver<(
            MessageId<Id>,
            SubscribeOption<I, T>,
        )>,
        new_event_rx: tokio::sync::mpsc::UnboundedReceiver<Event<I, T>>,
    ) -> Self {
        Self {
            new_subscriber_rx,
            new_event_rx,
            queue: VecDeque::new(),
            static_subscriptions: HashMap::new(),
            dynamic_subscriptions: Vec::new(),
        }
    }

    pub async fn run(mut self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut failed_in_a_row = 0;
        loop {
            dbg!(self.queue.len());
            tokio::select! {
                Some(new_subscriber) = self.new_subscriber_rx.recv() => {
                    let (message_id, subscriber_option) = new_subscriber;
                    self.new_subscriber(message_id, subscriber_option);
                }
                Some(new_event) = self.new_event_rx.recv() => {
                    self.push_event(dbg!(new_event));
                },
                _ = interval.tick() =>  {
                }
                else => {
                    panic!("am i closed now?")
                }

            }
            if self.try_consume_events().is_ok() {
                failed_in_a_row = 0;
            } else {
                failed_in_a_row += 1;
                if failed_in_a_row > 5 {
                    dbg!("dropping event", self.queue.pop_front());
                }
            }
        }
    }

    fn new_subscriber(
        &mut self,
        message_id: MessageId<Id>,
        subscriber_option: SubscribeOption<I, T>,
    ) {
        if message_id.src.contains('*') || message_id.dest.contains('*') {
            self.dynamic_subscriptions
                .push((message_id, subscriber_option));
        } else {
            self.static_subscriptions
                .insert(message_id, subscriber_option);
        }
    }

    fn push_event(&mut self, event: Event<I, T>) {
        self.queue.push_back(event);
    }

    fn can_consume_event(&self, event: &Event<I, T>) -> bool {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.static_subscriptions.get(&message_id) {
            match (sub, event) {
                (
                    SubscribeOption::MessageRecived(tx),
                    Event::MessageRecived(Message { body: _, .. }),
                ) => !tx.is_closed(),
                (SubscribeOption::MessageRecived(_), _) => {
                    dbg!("here");
                    false
                }
                (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body: _, .. })) => {
                    !tx.is_closed()
                }
                (SubscribeOption::MessageSent(_), _) => todo!(),
                (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
                (SubscribeOption::Error(_), _) => todo!(),
                (SubscribeOption::None(tx), _event) => !tx.is_closed(),
            }
        } else {
            self.match_dynamic_subcribers(event).is_some()
        }
    }

    fn find_consumable_event(&self, event: &Event<I, T>) -> Option<&SubscribeOption<I, T>> {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.static_subscriptions.get(&message_id) {
            Some(sub)
        } else {
            self.match_dynamic_subcribers(event).map(|(_id, sub)| sub)
        }
    }

    fn try_consume_events(&mut self) -> anyhow::Result<()> {
        while let Some(event) = self.queue.pop_front() {
            if let Some(option) = self.find_consumable_event(&event) {
                // TODO
                option.send(event)?;
            } else {
                self.queue.push_front(event);
                bail!("no subscribers match event");
            }
        }
        Ok(())
    }

    /// TODO always return first (0th) if available
    fn match_dynamic_subscribers_position(&self, event: &Event<I, T>) -> Option<usize> {
        if self.dynamic_subscriptions.is_empty() {
            None
        } else {
            Some(0)
        }
    }

    fn match_dynamic_subcribers(
        &self,
        event: &Event<I, T>,
    ) -> Option<&(MessageId<Id>, SubscribeOption<I, T>)> {
        let i = self.match_dynamic_subscribers_position(event)?;
        self.dynamic_subscriptions.get(i)
    }

    fn new_event(&mut self, event: Event<I, T>) {
        let message_id = match event {
            Event::MessageRecived(ref msg) => msg.clone_into_message_id(),
            Event::MessageSent(ref msg) => msg.clone_into_message_id(),
            Event::Error(_) => todo!(),
        };
        if let Some(sub) = self.static_subscriptions.get(&message_id) {
            match (sub, event) {
                (
                    SubscribeOption::MessageRecived(tx),
                    Event::MessageRecived(Message { body, .. }),
                ) => {
                    // errors if event listner dropped
                    let _ = tx.send(body);
                    let _ = self.static_subscriptions.remove(&message_id);
                }
                (SubscribeOption::MessageRecived(_), _) => {
                    dbg!("here");
                }
                (SubscribeOption::MessageSent(tx), Event::MessageSent(Message { body, .. })) => {
                    // errors if event listner dropped
                    let _ = tx.send(body);
                    let _ = self.static_subscriptions.remove(&message_id);
                }
                (SubscribeOption::MessageSent(_), _) => todo!(),
                (SubscribeOption::Error(_), Event::Error(_)) => todo!(),
                (SubscribeOption::Error(_), _) => todo!(),
                (SubscribeOption::None(tx), event) => {
                    // errors if event listner dropped
                    let _ = tx.send(event);
                    let _ = self.static_subscriptions.remove(&message_id);
                }
            }
        }
    }
}

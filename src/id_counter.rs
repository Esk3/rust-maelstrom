use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SeqIdCounter(Arc<Mutex<usize>>);

impl SeqIdCounter {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(0)))
    }
    #[must_use]
    pub fn next_id(&self) -> usize {
        let mut lock = self.0.lock().unwrap();
        *lock += 1;
        *lock
    }
}

impl Default for SeqIdCounter {
    fn default() -> Self {
        Self::new()
    }
}

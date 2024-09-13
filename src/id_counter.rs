use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Ids(Arc<Mutex<usize>>);

impl Ids {
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

impl Default for Ids {
    fn default() -> Self {
        Self::new()
    }
}

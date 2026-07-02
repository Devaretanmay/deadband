use std::collections::VecDeque;

use crate::event::ToolCallEvent;

pub trait HistoryStore: Send + Sync {
    fn push(&mut self, event: ToolCallEvent);
    fn get_recent(&self) -> Vec<ToolCallEvent>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct InMemoryHistoryStore {
    history: VecDeque<ToolCallEvent>,
    max_size: usize,
}

impl InMemoryHistoryStore {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
}

impl HistoryStore for InMemoryHistoryStore {
    fn push(&mut self, event: ToolCallEvent) {
        self.history.push_back(event);
        while self.history.len() > self.max_size {
            self.history.pop_front();
        }
    }

    fn get_recent(&self) -> Vec<ToolCallEvent> {
        self.history.iter().cloned().collect()
    }

    fn len(&self) -> usize {
        self.history.len()
    }
}

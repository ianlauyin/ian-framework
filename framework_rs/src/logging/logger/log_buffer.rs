use std::collections::VecDeque;

use crate::logging::LogEvent;

pub const CAPACITY: usize = 20_000;
pub const CAPACITY_WARN_NOTE: &str = "SHIPPER/capacityMeets80%";

pub struct LogBuffer {
    pub events: VecDeque<LogEvent>,
    capacity_warned: bool,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            capacity_warned: false,
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn push(&mut self, event: LogEvent) {
        while self.events.len() >= CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(event);
        if !self.capacity_warned && self.meets_capacity_warn() {
            while self.events.len() >= CAPACITY {
                self.events.pop_front();
            }
            self.events.push_back(LogEvent::warn(CAPACITY_WARN_NOTE));
            self.capacity_warned = true;
        }
    }

    pub fn take_chunk(&mut self, max: usize) -> Vec<LogEvent> {
        let count = max.min(self.events.len());
        self.events.drain(..count).collect()
    }

    pub fn restore(&mut self, batch: Vec<LogEvent>) {
        for event in batch.into_iter().rev() {
            self.events.push_front(event);
        }
        while self.events.len() > CAPACITY {
            self.events.pop_front();
        }
    }

    pub fn shipped(&mut self) {
        if !self.meets_capacity_warn() {
            self.capacity_warned = false;
        }
    }

    fn meets_capacity_warn(&self) -> bool {
        (self.events.len() as u128) * 5 >= (CAPACITY as u128) * 4
    }
}

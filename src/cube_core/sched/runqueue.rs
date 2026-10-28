// src/cube_core/sched/runqueue.rs
use super::task::{Task, State};

pub const MAX_TASKS: usize = 64;

pub struct RunQueue {
    pub tasks: [Task; MAX_TASKS],
    pub bitmap: u128,
    pub count: usize,
}

impl RunQueue {
    pub const fn new() -> Self {
        Self {
            tasks: [const { Task::free() }; MAX_TASKS],   // ← фикс
            bitmap: 0,
            count: 0,
        }
    }

    pub fn insert(&mut self, task: Task) -> Option<usize> {
        if self.count >= MAX_TASKS { return None; }
        for i in 0..MAX_TASKS {
            if self.tasks[i].state == State::Free {
                self.tasks[i] = task;
                if self.tasks[i].state == State::Ready {
                    self.bitmap |= 1u128 << (self.tasks[i].priority as u32);
                }
                self.count += 1;
                return Some(i);
            }
        }
        None
    }

    pub fn highest_ready(&self) -> Option<usize> {
        if self.bitmap == 0 { return None; }
        let prio = 127 - self.bitmap.leading_zeros() as u32;
        for i in 0..MAX_TASKS {
            if self.tasks[i].state == State::Ready
                && self.tasks[i].priority as u32 == prio
                {
                    return Some(i);
                }
        }
        None
    }

    pub fn set_ready_bit(&mut self, prio: u8) {
        self.bitmap |= 1u128 << prio;
    }

    pub fn idle_task(&self) -> Option<usize> {
        for i in 0..MAX_TASKS {
            if self.tasks[i].state == State::Ready && self.tasks[i].priority == 0 {
                return Some(i);
            }
        }
        None
    }
}

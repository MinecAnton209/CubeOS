use super::task::{Task, State, Qos};
use super::runqueue::{RunQueue, MAX_TASKS};

pub const AGING_INTERVAL: u64 = 5;
pub const DECAY_INTERVAL: u64 = 5;
pub const AGING_MAX: u8 = 60;
pub const DECAY_MIN: u8 = 80;

pub struct TimeshareState {
    pub tick_counter: u64,
}

impl TimeshareState {
    pub const fn new() -> Self { Self { tick_counter: 0 } }

    pub fn tick(&mut self, rq: &mut RunQueue) {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        // Aging: каждый Ready +1 tick_waited
        for i in 0..MAX_TASKS {
            if rq.tasks[i].state == State::Ready && rq.tasks[i].qos != Qos::Idle {
                rq.tasks[i].ticks_waited = rq.tasks[i].ticks_waited.saturating_add(1);
            }
        }

        // Aging раз в 25 тиков — поднять голодающих
        if self.tick_counter % AGING_INTERVAL == 0 {
            for i in 0..MAX_TASKS {
                if rq.tasks[i].state == State::Ready
                    && rq.tasks[i].ticks_waited >= AGING_INTERVAL as u32
                    && rq.tasks[i].priority < rq.tasks[i].base_priority.saturating_add(AGING_MAX)
                    {
                        rq.tasks[i].priority = rq.tasks[i].priority.saturating_add(1);
                        rq.tasks[i].ticks_waited = 0;
                        rq.set_ready_bit(rq.tasks[i].priority);
                    }
            }
        }

        // Decay раз в 20 тиков — опустить активных НИЖЕ base
        if self.tick_counter % DECAY_INTERVAL == 0 {
            for i in 0..MAX_TASKS {
                if rq.tasks[i].state == State::Running
                    && rq.tasks[i].qos != Qos::Idle
                    && rq.tasks[i].priority > rq.tasks[i].base_priority.saturating_sub(DECAY_MIN)
                    {
                        rq.tasks[i].priority = rq.tasks[i].priority.saturating_sub(1);
                        rq.tasks[i].ticks_waited = 0;
                    }
            }
        }
    }
}

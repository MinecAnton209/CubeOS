// src/cube_core/sched/timeshare.rs
use super::task::{Task, State, Qos};

pub const AGING_INTERVAL: u32 = 50;   // каждые 50 тиков — aging
pub const DECAY_INTERVAL: u32 = 100;  // каждые 100 тиков — decay для активных

pub struct TimeshareState {
    pub tick_counter: u64,
}

impl TimeshareState {
    pub const fn new() -> Self { Self { tick_counter: 0 } }

    /// Каждый тик: для всех Ready (не Running) — ticks_waited += 1.
    /// Раз в AGING_INTERVAL: priority += 1 (до base_priority.max(...)).
    pub fn tick(&mut self, rq: &mut super::runqueue::RunQueue) {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        for i in 0..super::runqueue::MAX_TASKS {
            if rq.tasks[i].state == State::Ready
                && rq.tasks[i].qos != Qos::Idle
                {
                    rq.tasks[i].ticks_waited = rq.tasks[i].ticks_waited.saturating_add(1);
                }
        }

        if self.tick_counter % AGING_INTERVAL as u64 == 0 {
            // Голодающие (ticks_waited > AGING_INTERVAL) — priority += 1
            for i in 0..super::runqueue::MAX_TASKS {
                if rq.tasks[i].state == State::Ready
                    && rq.tasks[i].ticks_waited >= AGING_INTERVAL
                    && rq.tasks[i].priority < rq.tasks[i].base_priority.saturating_add(40)
                    {
                        rq.tasks[i].priority = rq.tasks[i].priority.saturating_add(1);
                        rq.tasks[i].ticks_waited = 0;
                        rq.set_ready_bit(rq.tasks[i].priority);
                    }
            }
        }

        if self.tick_counter % DECAY_INTERVAL as u64 == 0 {
            // Активные — priority -= 1 (до base)
            for i in 0..super::runqueue::MAX_TASKS {
                if rq.tasks[i].state == State::Running
                    && rq.tasks[i].priority > rq.tasks[i].base_priority
                    {
                        rq.tasks[i].priority = rq.tasks[i].priority.saturating_sub(1);
                        rq.tasks[i].ticks_waited = 0;
                    }
            }
        }
    }

    /// Сбросить quantum текущей задачи.
    pub fn reset_quantum(&mut self, task: &mut Task) {
        task.reset_quantum();
        task.ticks_waited = 0;
    }
}

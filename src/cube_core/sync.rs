// src/cube_core/sync.rs
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    pub const fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    pub fn lock(&self) {
        while self.locked.swap(true, Ordering::Acquire) {
            unsafe { core::arch::asm!("pause") }
        }
    }

    pub fn try_lock(&self) -> bool {
        !self.locked.swap(true, Ordering::Acquire)
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

pub static ATA_LOCK: SpinLock = SpinLock::new();

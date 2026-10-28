// src/cube_core/sched.rs
// Round-robin вытесняющий планировщик.

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const MAX_TASKS: usize = 64;

#[derive(Copy, Clone, PartialEq)]
pub enum State { Free, Ready, Running, Zombie }

#[repr(C)]
pub struct Task {
    pub rsp: u64,
    pub state: State,
    pub pid: u64,
    pub stack_base: u64,
    pub stack_pages: usize,
    pub cr3: u64,
}

const FREE_TASK: Task = Task {
    rsp: 0, state: State::Free, pid: 0, stack_base: 0, stack_pages: 0, cr3: 0,
};

static mut TASKS: [Task; MAX_TASKS] = [FREE_TASK; MAX_TASKS];
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

pub fn init() {
    unsafe {
        TASKS[0] = Task {
            rsp: 0, state: State::Running, pid: 0, stack_base: 0, stack_pages: 0,
            cr3: crate::vmm::kernel_cr3(),
        };
    }
    CURRENT.store(0, Ordering::Relaxed);
}

pub fn current_pid() -> u64 {
    unsafe { TASKS[CURRENT.load(Ordering::Relaxed)].pid }
}

/// Вызывается из timer_body (IRQ 0x40) каждые 10 мс.
pub fn tick() {
    unsafe {
        let cur = CURRENT.load(Ordering::Relaxed);
        let mut next = cur;
        for i in 1..=MAX_TASKS {
            let idx = (cur + i) % MAX_TASKS;
            if TASKS[idx].state == State::Ready {
                next = idx;
                break;
            }
        }
        if next == cur { return; }
        if TASKS[cur].state == State::Running {
            TASKS[cur].state = State::Ready;
        }
        TASKS[next].state = State::Running;
        CURRENT.store(next, Ordering::Relaxed);
        crate::vmm::load_cr3(TASKS[next].cr3);
        context_switch(&mut TASKS[cur].rsp, TASKS[next].rsp);
    }
}

extern "C" {
    fn context_switch(save_rsp: *mut u64, load_rsp: u64);
}

pub fn spawn(entry: extern "C" fn() -> !, stack_base: u64, stack_pages: usize) -> Option<u64> {
    unsafe {
        for i in 1..MAX_TASKS {
            if TASKS[i].state == State::Free {
                let stack_top = stack_base + (stack_pages as u64) * 4096;
                let mut sp = stack_top;

                // 1. ABI padding — последний ret оставит RSP ≡ 8 (mod 16)
                sp -= 8;
                *(sp as *mut u64) = 0;

                // 2. Entry (return address для ret)
                sp -= 8;
                *(sp as *mut u64) = entry as u64;

                // 3. 6 callee-saved (r15..rbx) — читаются pop r15..rbx
                for _ in 0..6 {
                    sp -= 8;
                    *(sp as *mut u64) = 0;
                }

                // 4. RFLAGS — читается popfq (первый pop = самый низкий адрес)
                sp -= 8;
                *(sp as *mut u64) = 0x202;   // IF=1

                let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed) as u64;
                TASKS[i] = Task {
                    rsp: sp,        // = stack_top - 72
                    state: State::Ready,
                    pid,
                    stack_base,
                    stack_pages,
                    cr3: crate::vmm::kernel_cr3(),
                };
                return Some(pid);
            }
        }
        None
    }
}

core::arch::global_asm!(
    r#"
    .global context_switch
    .type context_switch, @function
    context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    pushfq
    mov [rdi], rsp
    mov rsp, rsi
    popfq
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
    .size context_switch, . - context_switch
    "#
);

pub fn exit_current() -> ! {
    unsafe {
        let cur = CURRENT.load(Ordering::Relaxed);
        TASKS[cur].state = State::Zombie;
    }
    loop { tick(); unsafe { asm!("hlt") } }
}

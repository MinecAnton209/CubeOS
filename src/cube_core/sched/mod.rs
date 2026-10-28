// src/cube_core/sched/mod.rs
pub mod task;
pub mod runqueue;
pub mod timeshare;
pub mod context;

pub use task::{Task, State, Qos};
use runqueue::{RunQueue, MAX_TASKS};
use timeshare::TimeshareState;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::arch::asm;

static mut RQ: RunQueue = RunQueue::new();
static mut TS: TimeshareState = TimeshareState::new();
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
static mut KERNEL_CR3: u64 = 0;

pub fn init() {
    unsafe {
        KERNEL_CR3 = read_cr3();
        let idle = Task {
            rsp: 0, state: State::Running, pid: 0, qos: Qos::Idle,
            base_priority: 0, priority: 0, quantum: 1,
            ticks_waited: 0, stack_base: 0, stack_pages: 0,
            cr3: KERNEL_CR3, user_rsp: 0, user_rip: 0,
            name: *b"idle\0\0\0\0\0\0\0\0\0\0\0\0",
        };
        RQ.tasks[0] = idle;
        RQ.count = 1;
        CURRENT.store(0, Ordering::Relaxed);
    }
}

/// Создать kernel task (всегда CR3 = 0, identity).
pub fn spawn(
    entry: extern "C" fn() -> !,
             stack_base: u64,
             stack_pages: usize,
             qos: Qos,
             name: &[u8],
) -> Option<u64> {
    unsafe {
        let stack_top = stack_base + (stack_pages as u64) * 4096;
        let mut sp = stack_top;

        sp -= 8; *(sp as *mut u64) = 0;
        sp -= 8; *(sp as *mut u64) = entry as u64;
        for _ in 0..6 { sp -= 8; *(sp as *mut u64) = 0; }
        sp -= 8; *(sp as *mut u64) = 0x202;

        let mut t = Task::free();
        t.rsp = sp;
        t.state = State::Ready;
        t.pid = NEXT_PID.fetch_add(1, Ordering::Relaxed) as u64;
        t.qos = qos;
        t.base_priority = qos.base_priority();
        t.priority = qos.base_priority();
        t.quantum = qos.quantum_ticks();
        t.stack_base = stack_base;
        t.stack_pages = stack_pages;
        t.cr3 = KERNEL_CR3;   // kernel task — identity
        let n = name.len().min(15);
        t.name[..n].copy_from_slice(&name[..n]);
        t.name[n] = 0;

        let idx = RQ.insert(t)?;
        Some(RQ.tasks[idx].pid)
    }
}

/// Создать user task с отдельным PML4.
/// Вызывается из `fork`/`exec` — PML4 уже создан VMM'ом.
pub fn spawn_user(
    pml4_phys: u64,
    user_rip: u64,
    user_rsp: u64,
    kernel_stack_base: u64,
    qos: Qos,
    name: &[u8],
) -> Option<u64> {
    unsafe {
        // Kernel-стек для задачи (на нём будет iretq в user)
        let stack_top = kernel_stack_base + 4096;
        let mut sp = stack_top;

        sp -= 8; *(sp as *mut u64) = 0;                // ABI padding
        sp -= 8; *(sp as *mut u64) = user_entry_trampoline as u64;
        for _ in 0..6 { sp -= 8; *(sp as *mut u64) = 0; }
        sp -= 8; *(sp as *mut u64) = 0x202;

        let mut t = Task::free();
        t.rsp = sp;
        t.state = State::Ready;
        t.pid = NEXT_PID.fetch_add(1, Ordering::Relaxed) as u64;
        t.qos = qos;
        t.base_priority = qos.base_priority();
        t.priority = qos.base_priority();
        t.quantum = qos.quantum_ticks();
        t.stack_base = kernel_stack_base;
        t.stack_pages = 1;
        t.cr3 = pml4_phys;
        t.user_rsp = user_rsp;
        t.user_rip = user_rip;
        let n = name.len().min(15);
        t.name[..n].copy_from_slice(&name[..n]);
        t.name[n] = 0;

        let idx = RQ.insert(t)?;
        Some(RQ.tasks[idx].pid)
    }
}

/// Трамплин для user-task: iretq в user по user_rip/user_rsp.
/// Вызывается как entry kernel-task — на kernel-стеке.
#[no_mangle]
pub extern "C" fn user_entry_trampoline() -> ! {
    unsafe {
        // Текущий Task — взять user_rip/user_rsp
        let cur = CURRENT.load(Ordering::Relaxed);
        let rip = RQ.tasks[cur].user_rip;
        let rsp = RQ.tasks[cur].user_rsp;

        asm!(
            "mov ds, {s:x}",
             "mov es, {s:x}",
             "mov fs, {s:x}",
             "mov gs, {s:x}",
             "push 0x23",       // user SS
             "push {rsp}",
             "push 0x202",      // user RFLAGS
             "push 0x1B",       // user CS
             "push {rip}",
             "iretq",
             s = in(reg) 0x23u16,
             rsp = in(reg) rsp,
             rip = in(reg) rip,
             options(noreturn)
        );
    }
}

pub fn tick() {
    unsafe {
        TS.tick(&mut RQ);

        let cur = CURRENT.load(Ordering::Relaxed);

        if RQ.tasks[cur].quantum != u32::MAX {
            RQ.tasks[cur].quantum = RQ.tasks[cur].quantum.saturating_sub(1);
        }

        if RQ.tasks[cur].quantum > 0 { return; }

        let next = if let Some(n) = RQ.highest_ready() { n }
        else if let Some(n) = RQ.idle_task() { n }
        else { return; };

        if next == cur { return; }

        if RQ.tasks[cur].state == State::Running {
            RQ.tasks[cur].state = State::Ready;
            RQ.tasks[cur].reset_quantum();
            RQ.set_ready_bit(RQ.tasks[cur].priority);
        }
        RQ.tasks[next].state = State::Running;
        CURRENT.store(next, Ordering::Relaxed);

        context::context_switch(
            &mut RQ.tasks[cur].rsp,
            RQ.tasks[next].rsp,
            RQ.tasks[next].cr3,    // ← переключение PML4
        );
    }
}

pub fn exit_current() -> ! {
    unsafe {
        let cur = CURRENT.load(Ordering::Relaxed);
        RQ.tasks[cur].state = State::Zombie;
        RQ.tasks[cur].quantum = 0;
    }
    loop {
        tick();
        unsafe { asm!("hlt") }
    }
}

pub fn task_state(pid: u64) -> State {
    unsafe {
        for i in 0..MAX_TASKS {
            if RQ.tasks[i].pid == pid { return RQ.tasks[i].state; }
        }
    }
    State::Zombie
}

pub fn current_pid() -> u64 {
    unsafe { RQ.tasks[CURRENT.load(Ordering::Relaxed)].pid }
}

pub fn current_cr3() -> u64 {
    unsafe { RQ.tasks[CURRENT.load(Ordering::Relaxed)].cr3 }
}

pub fn set_qos(pid: u64, qos: Qos) {
    unsafe {
        for i in 0..MAX_TASKS {
            if RQ.tasks[i].pid == pid && RQ.tasks[i].state != State::Free {
                RQ.tasks[i].qos = qos;
                RQ.tasks[i].base_priority = qos.base_priority();
                RQ.tasks[i].priority = qos.base_priority();
                RQ.tasks[i].reset_quantum();
            }
        }
    }
}

fn read_cr3() -> u64 {
    let cr3: u64;
    unsafe { asm!("mov {}, cr3", out(reg) cr3); }
    cr3
}

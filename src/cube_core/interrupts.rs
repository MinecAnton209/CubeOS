// src/cube_core/interrupts.rs — PIC remap + keyboard buffer
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const KEY_BUF_SIZE: usize = 64;
static mut KEY_BUF: [u8; KEY_BUF_SIZE] = [0; KEY_BUF_SIZE];
static KEY_HEAD: AtomicUsize = AtomicUsize::new(0);
static KEY_TAIL: AtomicUsize = AtomicUsize::new(0);

static PIC_REMAPPED: AtomicBool = AtomicBool::new(false);

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") v) }
    v
}

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val) }
}

/// Ремап PIC: IRQ 0-7 → 0x20-0x27, IRQ 8-15 → 0x28-0x2F
pub fn remap_pic() {
    if PIC_REMAPPED.swap(true, Ordering::Relaxed) {
        return;
    }

    let a1 = inb(PIC1_DATA);
    let a2 = inb(PIC2_DATA);

    // ICW1: инициализация
    outb(PIC1_CMD, 0x11);
    outb(PIC2_CMD, 0x11);

    // ICW2: базовые векторы
    outb(PIC1_DATA, 0x20); // IRQ 0-7 → 0x20-0x27
    outb(PIC2_DATA, 0x28); // IRQ 8-15 → 0x28-0x2F

    // ICW3: каскадирование
    outb(PIC1_DATA, 0x04); // Slave на IRQ2
    outb(PIC2_DATA, 0x02); // Cascade identity

    // ICW4: режим 8086
    outb(PIC1_DATA, 0x01);
    outb(PIC2_DATA, 0x01);

    // Восстанавливаем маски, но маскируем таймер (IRQ0)
    outb(PIC1_DATA, a1 | 0x01);
    outb(PIC2_DATA, a2);
}

/// Отправить EOI ведущему PIC
pub fn send_eoi() {
    outb(PIC1_CMD, 0x20);
}

/// Отправить EOI ведомому PIC (для IRQ 8-15)
pub fn send_eoi_slave() {
    outb(PIC2_CMD, 0x20);
    outb(PIC1_CMD, 0x20);
}

/// Разрешить конкретный IRQ
pub fn enable_irq(irq: u8) {
    let port = if irq < 8 { PIC1_DATA } else { PIC2_DATA };
    let bit = if irq < 8 { irq } else { irq - 8 };
    let mask = inb(port) & !(1 << bit);
    outb(port, mask);
}

/// Маскировать все IRQ кроме каскада (IRQ2)
pub fn mask_all_irqs() {
    outb(PIC1_DATA, 0xFB); // 1111 1011 — только IRQ2
    outb(PIC2_DATA, 0xFF); // Все замаскированы
}

// === Keyboard buffer ===
pub fn keyboard_push(scancode: u8) {
    let head = KEY_HEAD.load(Ordering::Relaxed);
    let next = (head + 1) & (KEY_BUF_SIZE - 1);
    if next != KEY_TAIL.load(Ordering::Acquire) {
        unsafe { KEY_BUF[head] = scancode; }
        KEY_HEAD.store(next, Ordering::Release);
    }
}

pub fn keyboard_pop() -> Option<u8> {
    let tail = KEY_TAIL.load(Ordering::Relaxed);
    if tail == KEY_HEAD.load(Ordering::Acquire) {
        return None;
    }
    let sc = unsafe { KEY_BUF[tail] };
    KEY_TAIL.store((tail + 1) & (KEY_BUF_SIZE - 1), Ordering::Release);
    Some(sc)
}

pub fn read_pic1_mask() -> u8 { inb(PIC1_DATA) }
pub fn read_pic2_mask() -> u8 { inb(PIC2_DATA) }

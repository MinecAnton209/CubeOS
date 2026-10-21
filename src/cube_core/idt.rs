// src/cube_core/idt.rs — исправленная таблица прерываний
use core::mem::size_of;

const IDT_ENTRIES: usize = 256;

#[repr(C)]
pub struct InterruptStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn new() -> Self {
        Self {
            offset_low: 0, selector: 0, ist: 0, flags: 0,
            offset_mid: 0, offset_high: 0, reserved: 0,
        }
    }

    pub fn set_handler_addr(&mut self, handler_addr: u64) {
        self.offset_low = (handler_addr & 0xFFFF) as u16;
        self.selector = 0x08; // Kernel code segment
        self.ist = 0;
        self.flags = 0x8E;    // Present, DPL=0, Interrupt Gate
        self.offset_mid = ((handler_addr >> 16) & 0xFFFF) as u16;
        self.offset_high = (handler_addr >> 32) as u32;
        self.reserved = 0;
    }
}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; IDT_ENTRIES] = [IdtEntry::new(); IDT_ENTRIES];

// === Обработчики исключений (0-31) ===
macro_rules! gen_stub {
    ($name:ident, $num:expr) => {
        #[unsafe(naked)]
        extern "x86-interrupt" fn $name(_sf: &InterruptStackFrame) {
            core::arch::naked_asm!(
                "push rdi",
                "mov rdi, {num}",
                "call {handler}",
                "pop rdi",
                "iretq",
                num = const $num,
                handler = sym exception_handler,
            );
        }
    };
}

macro_rules! gen_stub_error {
    ($name:ident, $num:expr) => {
        #[unsafe(naked)]
        extern "x86-interrupt" fn $name(_sf: &InterruptStackFrame, _error_code: u64) {
            core::arch::naked_asm!(
                "push rdi",
                "mov rdi, {num}",
                "call {handler}",
                "add rsp, 8",  // Убираем error code со стека
                "pop rdi",
                "iretq",
                num = const $num,
                handler = sym exception_handler,
            );
        }
    };
}

// Генерируем все 32 обработчика исключений
gen_stub!(isr0, 0);   gen_stub!(isr1, 1);   gen_stub!(isr2, 2);   gen_stub!(isr3, 3);
gen_stub!(isr4, 4);   gen_stub!(isr5, 5);   gen_stub!(isr6, 6);   gen_stub!(isr7, 7);
gen_stub_error!(isr8, 8);
gen_stub!(isr9, 9);
gen_stub_error!(isr10, 10); gen_stub_error!(isr11, 11); gen_stub_error!(isr12, 12);
gen_stub_error!(isr13, 13); gen_stub_error!(isr14, 14);
gen_stub!(isr15, 15); gen_stub!(isr16, 16);
gen_stub_error!(isr17, 17);
gen_stub!(isr18, 18); gen_stub!(isr19, 19); gen_stub!(isr20, 20); gen_stub!(isr21, 21);
gen_stub!(isr22, 22); gen_stub!(isr23, 23); gen_stub!(isr24, 24); gen_stub!(isr25, 25);
gen_stub!(isr26, 26); gen_stub!(isr27, 27); gen_stub!(isr28, 28); gen_stub!(isr29, 29);
gen_stub!(isr30, 30); gen_stub!(isr31, 31);

// === Обработчик IRQ клавиатуры (ПРАВИЛЬНЫЙ — без двойного сохранения!) ===
// Найди в src/idt.rs функцию keyboard_handler и замени её целиком на эту:

extern "x86-interrupt" fn keyboard_handler(_sf: InterruptStackFrame) {
    let scancode: u8;
    unsafe {
        // Читаем сканкод
        core::arch::asm!("in al, 0x60", out("al") scancode);
        // Отправляем в буфер прерываний
        crate::interrupts::keyboard_push(scancode);
        // Отправляем EOI ведущему PIC
        core::arch::asm!("out 0x20, al", in("al") 0x20u8);
    }
}

// === Заглушка для остальных IRQ ===
#[unsafe(naked)]
extern "x86-interrupt" fn irq_stub(_sf: &InterruptStackFrame) {
    core::arch::naked_asm!(
        "push rax",
        "mov al, 0x20",
        "mov dx, 0x20",
        "out dx, al",       // EOI для master PIC
        "pop rax",
        "iretq",
    );
}

// === Общий обработчик исключений ===
#[no_mangle]
pub extern "C" fn exception_handler(int_num: u64) {
    let stop_code = match int_num {
        0 => "DIVIDE_BY_ZERO",
        1 => "DEBUG",
        3 => "BREAKPOINT",
        6 => "INVALID_OPCODE",
        8 => "DOUBLE_FAULT",
        10 => "INVALID_TSS",
        11 => "SEGMENT_NOT_PRESENT",
        12 => "STACK_SEGMENT_FAULT",
        13 => "GENERAL_PROTECTION",
        14 => "PAGE_FAULT",
        17 => "ALIGNMENT_CHECK",
        _ => "UNKNOWN_EXCEPTION",
    };

    crate::serial::log_error("EXCEPTION");
    crate::serial::send_string("  Vector: ");
    // Выводим номер вектора
    let mut buf = [0u8; 20];
    let mut i = 20;
    let mut n = int_num;
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while n > 0 {
            i -= 1;
            buf[i] = (n % 10) as u8 + b'0';
            n /= 10;
        }
    }
    crate::serial::send_string(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
    crate::serial::send_string("\n  Stop code: ");
    crate::serial::send_string(stop_code);
    crate::serial::send_string("\n");

    // Halt
    loop {
        unsafe { core::arch::asm!("cli; hlt") }
    }
}

pub unsafe fn init_idt() {
    // Устанавливаем обработчики исключений
    IDT[0].set_handler_addr(isr0 as u64);
    IDT[1].set_handler_addr(isr1 as u64);
    IDT[2].set_handler_addr(isr2 as u64);
    IDT[3].set_handler_addr(isr3 as u64);
    IDT[4].set_handler_addr(isr4 as u64);
    IDT[5].set_handler_addr(isr5 as u64);
    IDT[6].set_handler_addr(isr6 as u64);
    IDT[7].set_handler_addr(isr7 as u64);
    IDT[8].set_handler_addr(isr8 as u64);
    IDT[9].set_handler_addr(isr9 as u64);
    IDT[10].set_handler_addr(isr10 as u64);
    IDT[11].set_handler_addr(isr11 as u64);
    IDT[12].set_handler_addr(isr12 as u64);
    IDT[13].set_handler_addr(isr13 as u64);
    IDT[14].set_handler_addr(isr14 as u64);
    IDT[15].set_handler_addr(isr15 as u64);
    IDT[16].set_handler_addr(isr16 as u64);
    IDT[17].set_handler_addr(isr17 as u64);
    IDT[18].set_handler_addr(isr18 as u64);
    IDT[19].set_handler_addr(isr19 as u64);
    IDT[20].set_handler_addr(isr20 as u64);
    IDT[21].set_handler_addr(isr21 as u64);
    IDT[22].set_handler_addr(isr22 as u64);
    IDT[23].set_handler_addr(isr23 as u64);
    IDT[24].set_handler_addr(isr24 as u64);
    IDT[25].set_handler_addr(isr25 as u64);
    IDT[26].set_handler_addr(isr26 as u64);
    IDT[27].set_handler_addr(isr27 as u64);
    IDT[28].set_handler_addr(isr28 as u64);
    IDT[29].set_handler_addr(isr29 as u64);
    IDT[30].set_handler_addr(isr30 as u64);
    IDT[31].set_handler_addr(isr31 as u64);

    // Заглушки для IRQ 0x20-0x2F
    for i in 0..16 {
        IDT[0x20 + i].set_handler_addr(irq_stub as u64);
    }

    // Клавиатура (IRQ1 = вектор 0x21)
    IDT[0x21].set_handler_addr(keyboard_handler as u64);

    // Загружаем IDT
    let idt_ptr = IdtPtr {
        limit: (size_of::<[IdtEntry; IDT_ENTRIES]>() - 1) as u16,
        base: IDT.as_ptr() as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &idt_ptr);
}

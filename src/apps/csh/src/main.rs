#![no_std]
#![no_main]

use core::arch::asm;

// Must match KernelApi in src/cube_core/loader.rs; the kernel passes it in rdi.
#[repr(C)]
pub struct KernelApi {
    putc: extern "C" fn(u8),
    clear: extern "C" fn(),
}

static mut API: *const KernelApi = core::ptr::null();

// Простой вывод в Serial (COM1)
fn serial_out(byte: u8) {
    unsafe { asm!("out dx, al", in("dx") 0x3F8u16, in("al") byte) };
}

fn console_putc(byte: u8) {
    unsafe {
        if !API.is_null() {
            ((*API).putc)(byte);
        }
    }
}

fn console_clear() {
    unsafe {
        if !API.is_null() {
            ((*API).clear)();
        }
    }
}

fn out_byte(byte: u8) {
    serial_out(byte);
    console_putc(byte);
}

fn serial_print(s: &str) {
    for byte in s.bytes() {
        if byte == b'\n' {
            serial_out(b'\r');
        }
        out_byte(byte);
    }
}

// For ANSI sequences that only a serial terminal understands
fn serial_only(s: &str) {
    for byte in s.bytes() {
        serial_out(byte);
    }
}

// Port 0x60 keeps returning the last scancode when empty, so only read it when port 0x64 reports data
fn keyboard_read() -> Option<u8> {
    let status: u8;
    unsafe { asm!("in al, 0x64", out("al") status) };
    if status & 0x01 == 0 {
        return None;
    }
    let sc: u8;
    unsafe { asm!("in al, 0x60", out("al") sc) };
    Some(sc)
}

// Простая таблица сканкодов → ASCII (только буквы и цифры)
fn scancode_to_ascii(sc: u8) -> Option<u8> {
    match sc {
        0x02 => Some(b'1'), 0x03 => Some(b'2'), 0x04 => Some(b'3'),
        0x05 => Some(b'4'), 0x06 => Some(b'5'), 0x07 => Some(b'6'),
        0x08 => Some(b'7'), 0x09 => Some(b'8'), 0x0A => Some(b'9'),
        0x0B => Some(b'0'),
        0x10 => Some(b'q'), 0x11 => Some(b'w'), 0x12 => Some(b'e'),
        0x13 => Some(b'r'), 0x14 => Some(b't'), 0x15 => Some(b'y'),
        0x16 => Some(b'u'), 0x17 => Some(b'i'), 0x18 => Some(b'o'),
        0x19 => Some(b'p'),
        0x1E => Some(b'a'), 0x1F => Some(b's'), 0x20 => Some(b'd'),
        0x21 => Some(b'f'), 0x22 => Some(b'g'), 0x23 => Some(b'h'),
        0x24 => Some(b'j'), 0x25 => Some(b'k'), 0x26 => Some(b'l'),
        0x2C => Some(b'z'), 0x2D => Some(b'x'), 0x2E => Some(b'c'),
        0x2F => Some(b'v'), 0x30 => Some(b'b'), 0x31 => Some(b'n'),
        0x32 => Some(b'm'),
        0x1C => Some(b'\n'),  // Enter
        0x0E => Some(b'\x08'), // Backspace
        0x39 => Some(b' '),    // Space
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn _start(api: *const KernelApi) -> ! {
    unsafe { API = api };

    // TODO: KernelApi не виден из user-PML4. Перевести на syscall.
    // Пока — просто loop, чтобы проверить, что user-mode работает.
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial_print("\n[CSH] PANIC!\n");
    loop { unsafe { asm!("cli; hlt") } }
}

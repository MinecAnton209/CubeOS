// src/cube_core/panic.rs
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::serial::log_error("KERNEL PANIC");

    if let Some(location) = info.location() {
        crate::serial::send_string("  at ");
        crate::serial::send_string(location.file());
        crate::serial::send_string(":");
        // Выводим номер строки
        let mut buf = [0u8; 10];
        let mut n = location.line() as usize;
        let mut i = 10;
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
        crate::serial::send_string("\n");
    }

    if let Some(msg) = info.message().as_str() {
        crate::serial::send_string("  message: ");
        crate::serial::send_string(msg);
        crate::serial::send_string("\n");
    }

    loop {
        unsafe { core::arch::asm!("cli; hlt") }
    }
}

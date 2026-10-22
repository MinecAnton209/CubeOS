#![no_std]
#![no_main]

use core::arch::asm;

// Простой вывод в Serial (COM1)
fn serial_out(byte: u8) {
    unsafe { asm!("out dx, al", in("dx") 0x3F8u16, in("al") byte) };
}

fn serial_print(s: &str) {
    for byte in s.bytes() {
        if byte == b'\n' {
            serial_out(b'\r');
        }
        serial_out(byte);
    }
}

// Чтение символа с клавиатуры (через порт 0x60)
fn keyboard_read() -> u8 {
    let sc: u8;
    unsafe { asm!("in al, 0x60", out("al") sc) };
    sc
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
pub extern "C" fn _start() -> ! {
    serial_print("\n");
    serial_print("========================================\n");
    serial_print("  CubeOS Shell v0.1 (csh)\n");
    serial_print("========================================\n");
    serial_print("Type 'help' for commands, 'exit' to quit\n\n");

    let mut cmd_buf = [0u8; 256];
    let mut cmd_len = 0;

    loop {
        // Выводим приглашение
        serial_print("root@cubeos # ");

        // Читаем команду
        cmd_len = 0;
        loop {
            let sc = keyboard_read();

            // Игнорируем отпускание клавиш
            if sc & 0x80 != 0 {
                continue;
            }

            if let Some(ch) = scancode_to_ascii(sc) {
                if ch == b'\n' {
                    serial_out(b'\n');
                    break;
                } else if ch == b'\x08' {
                    // Backspace
                    if cmd_len > 0 {
                        cmd_len -= 1;
                        serial_print("\x08 \x08");
                    }
                } else if cmd_len < 255 {
                    cmd_buf[cmd_len] = ch;
                    cmd_len += 1;
                    serial_out(ch);
                }
            }
        }

        // Парсим команду
        let cmd = core::str::from_utf8(&cmd_buf[..cmd_len]).unwrap_or("");
        let cmd = cmd.trim();

        match cmd {
            "help" => {
                serial_print("Available commands:\n");
                serial_print("  help    - show this help\n");
                serial_print("  echo    - print text\n");
                serial_print("  clear   - clear screen\n");
                serial_print("  exit    - exit shell\n");
                serial_print("\n");
            }
            "clear" => {
                // Очищаем экран через ANSI (работает в Serial)
                serial_print("\x1B[2J\x1B[H");
            }
            "exit" => {
                serial_print("Goodbye!\n");
                loop { unsafe { asm!("cli; hlt") } }
            }
            _ if cmd.starts_with("echo ") => {
                let text = &cmd[5..];
                serial_print(text);
                serial_print("\n");
            }
            "" => {}
            _ => {
                serial_print("Unknown command: ");
                serial_print(cmd);
                serial_print("\n");
            }
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial_print("\n[CSH] PANIC!\n");
    loop { unsafe { asm!("cli; hlt") } }
}

#![no_std]
#![no_main]

use cube_syscall::*;

// Must match KernelApi in src/cube_core/loader.rs; the kernel passes it in rdi.
#[repr(C)]
struct KernelApi {
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

fn cmd_pwd(_args: &[u8]) {
    let mut buf = [0u8; 256];
    let n = getcwd(&mut buf);
    if n > 0 {
        print(&buf[..n as usize]);
        print(b"\n");
    }
}

fn cmd_cat(args: &[u8]) {
    if args.is_empty() { eprint(b"cat: missing argument\n"); return; }
    let fd = open(args, O_RDONLY);
    if fd < 0 { eprint(b"cat: cannot open\n"); return; }
    let mut buf = [0u8; 512];
    loop {
        let n = read(fd as usize, &mut buf);
        if n <= 0 { break; }
        write(STDOUT, &buf[..n as usize]);
    }
    close(fd as usize);
}

fn cmd_mkdir(args: &[u8]) {
    if args.is_empty() { eprint(b"mkdir: missing argument\n"); return; }
    if mkdir(args, 0o755) < 0 { eprint(b"mkdir: failed\n"); }
}

fn cmd_rm(args: &[u8]) {
    if args.is_empty() { eprint(b"rm: missing argument\n"); return; }
    if unlink(args) < 0 { eprint(b"rm: failed\n"); }
}

fn cmd_echo(args: &[u8]) {
    print(args);
    print(b"\n");
}

fn cmd_help(_args: &[u8]) {
    print(b"Commands:\n");
    print(b"  ls [path]     list directory\n");
    print(b"  cd [path]     change directory\n");
    print(b"  pwd           print working directory\n");
    print(b"  cat <file>    print file contents\n");
    print(b"  mkdir <path>  create directory\n");
    print(b"  rm <path>     remove file\n");
    print(b"  echo <text>   print text\n");
    print(b"  help          this message\n");
    print(b"  exit          quit shell\n");
}

#[no_mangle]
pub extern "C" fn _start(api: *const KernelApi) -> ! {
    unsafe { API = api };
    serial_print("\n");
    serial_print("========================================\n");
    serial_print("  CubeOS Shell v0.1 (csh)\n");
    serial_print("========================================\n");
    serial_print("Type 'help' for commands, 'exit' to quit\n\n");

    let mut cmd_buf = [0u8; 256];
    let mut cmd_len = 0;

    let mut line = [0u8; MAX_LINE];
    loop {
        let mut cwd = [0u8; 256];
        let cn = getcwd(&mut cwd);
        if cn > 0 { print(&cwd[..cn as usize]); }
        print(b"$ ");

        // Читаем команду
        cmd_len = 0;
        loop {
            let sc = match keyboard_read() {
                Some(sc) => sc,
                None => continue,
            };

            // Игнорируем отпускание клавиш
            if sc & 0x80 != 0 {
                continue;
            }

            if let Some(ch) = scancode_to_ascii(sc) {
                if ch == b'\n' {
                    out_byte(b'\n');
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
                    out_byte(ch);
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
                // ANSI clears a serial terminal; the framebuffer console clears through the kernel API
                serial_only("\x1B[2J\x1B[H");
                console_clear();
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
                eprint(b"unknown command: ");
                eprint(cmd);
                eprint(b"\n");
            }
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    eprint(b"[csh] panic\n");
    loop {}
}

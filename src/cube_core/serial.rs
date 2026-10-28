// src/cube_core/serial.rs — вывод в COM1 (115200 baud, 8N1)
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};

const COM1: u16 = 0x3F8;
static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val) }
}

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") v) }
    v
}

/// Инициализация COM1 (вызывать один раз при старте)
pub fn init() {
    outb(COM1 + 1, 0x00); // Отключаем прерывания
    outb(COM1 + 3, 0x80); // Включаем DLAB (установка делителя)
    outb(COM1 + 0, 0x01); // Делитель lo = 1 (115200 baud)
    outb(COM1 + 1, 0x00); // Делитель hi = 0
    outb(COM1 + 3, 0x03); // 8 бит, без чётности, 1 стоп-бит
    outb(COM1 + 2, 0xC7); // FIFO: включить, очистить, порог 14 байт
    outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
    INITIALIZED.store(true, Ordering::Release);
}

pub fn send_byte(byte: u8) {
    unsafe {
        core::arch::asm!(
            "2:",
            "mov dx, 0x3FD",      // Порт LSR (Line Status Register)
            "in al, dx",          // Читаем статус
            "test al, 0x20",      // Проверяем бит 5 (буфер передатчика пуст)
            "jz 2b",              // Если 0, крутимся в цикле (2b = jump to label 2 backward)

            "mov dx, 0x3F8",      // Порт данных COM1
            "mov al, {byte}",     // Загружаем наш байт
            "out dx, al",         // Отправляем байт

            byte = in(reg_byte) byte,
            out("dx") _,
            out("al") _,
            options(nomem, nostack)
        );
    }
}

/// Отправка строки
pub fn send_string(s: &str) {
    for byte in s.bytes() {
        if byte == b'\n' {
            send_byte(b'\r'); // CR перед LF для корректного отображения
        }
        send_byte(byte);
    }
}

/// Структура для использования с `write!` макросом
pub struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        send_string(s);
        Ok(())
    }
}

/// Удобный макрос для вывода в Serial
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        {
            use core::fmt::Write;
            let _ = write!($crate::serial::SerialWriter, $($arg)*);
        }
    };
}

#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => {
        {
            $crate::kprint!("{}\n", format_args!($($arg)*));
        }
    };
}

/// Логи с префиксом уровня
pub fn log_info(msg: &str) {
    send_string("[INFO ] ");
    send_string(msg);
    send_string("\n");
}

pub fn log_ok(msg: &str) {
    send_string("[  OK ] ");
    send_string(msg);
    send_string("\n");
}

pub fn log_warn(msg: &str) {
    send_string("[ WARN] ");
    send_string(msg);
    send_string("\n");
}

pub fn log_error(msg: &str) {
    send_string("[ERROR] ");
    send_string(msg);
    send_string("\n");
}

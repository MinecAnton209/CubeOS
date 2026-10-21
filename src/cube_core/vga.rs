// src/cube_core/vga.rs — текстовый VGA 80x25
use core::fmt::{self, Write};

const VGA_BUFFER: usize = 0xB8000;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Color {
    Black = 0, Blue = 1, Green = 2, Cyan = 3,
    Red = 4, Magenta = 5, Brown = 6, LightGray = 7,
    DarkGray = 8, LightBlue = 9, LightGreen = 10, LightCyan = 11,
    LightRed = 12, Pink = 13, Yellow = 14, White = 15,
}

pub struct VgaWriter {
    buffer: *mut u16,
    column: usize,
    row: usize,
    color: u8,
}

impl VgaWriter {
    pub const fn new() -> Self {
        Self {
            buffer: VGA_BUFFER as *mut u16,
            column: 0,
            row: 0,
            color: (Color::LightGray as u8) << 4 | Color::Black as u8,
        }
    }

    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.color = (bg as u8) << 4 | (fg as u8);
    }

    pub fn clear(&mut self) {
        for y in 0..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                unsafe {
                    *self.buffer.add(y * VGA_WIDTH + x) = (self.color as u16) << 8 | 0x20;
                }
            }
        }
        self.column = 0;
        self.row = 0;
    }

    fn scroll(&mut self) {
        for y in 1..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                unsafe {
                    let src = y * VGA_WIDTH + x;
                    let dst = (y - 1) * VGA_WIDTH + x;
                    *self.buffer.add(dst) = *self.buffer.add(src);
                }
            }
        }
        let last_row = (VGA_HEIGHT - 1) * VGA_WIDTH;
        for x in 0..VGA_WIDTH {
            unsafe {
                *self.buffer.add(last_row + x) = (self.color as u16) << 8 | 0x20;
            }
        }
    }

    pub fn put_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.column = 0;
                self.row += 1;
            }
            '\r' => self.column = 0,
            '\x08' => {
                // Backspace
                if self.column > 0 {
                    self.column -= 1;
                    unsafe {
                        let offset = self.row * VGA_WIDTH + self.column;
                        *self.buffer.add(offset) = (self.color as u16) << 8 | 0x20;
                    }
                }
            }
            c => {
                if self.column >= VGA_WIDTH {
                    self.column = 0;
                    self.row += 1;
                }
                if self.row >= VGA_HEIGHT {
                    self.scroll();
                    self.row = VGA_HEIGHT - 1;
                }
                let offset = self.row * VGA_WIDTH + self.column;
                unsafe {
                    *self.buffer.add(offset) = (self.color as u16) << 8 | (c as u16);
                }
                self.column += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.put_char(c);
        }
    }
}

impl Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// Глобальный VGA writer (используется для ранних логов до Serial)
use core::sync::atomic::{AtomicBool, Ordering};
static VGA_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    let mut vga = VgaWriter::new();
    vga.clear();
    VGA_INITIALIZED.store(true, Ordering::Release);
}

pub fn write_early(s: &str) {
    if VGA_INITIALIZED.load(Ordering::Acquire) {
        let mut vga = VgaWriter::new();
        vga.write_string(s);
    }
}

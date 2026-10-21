// src/cube_core/graphics.rs

use crate::cube_core::font::{FONT, FONT_WIDTH, FONT_HEIGHT};

pub struct Graphics {
    pub fb: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

impl Graphics {
    pub fn new(addr: u64, width: usize, height: usize, pitch: usize) -> Self {
        Self { fb: addr as *mut u32, width, height, pitch }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let offset = y * (self.pitch / 4) + x;
            unsafe { *self.fb.add(offset) = color; }
        }
    }

    pub fn clear(&mut self, color: u32) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.put_pixel(x, y, color);
            }
        }
    }

    pub fn draw_text(&mut self, x: usize, y: usize, text: &str, fg: u32, bg: u32) {
        for (i, ch) in text.chars().enumerate() {
            self.draw_char(x + i * FONT_WIDTH, y, ch, fg, bg);
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, ch: char, fg: u32, bg: u32) {
        if ch < ' ' || ch > '~' { return; }
        let idx = (ch as usize) - 0x20;
        let base = idx * FONT_HEIGHT;

        for row in 0..FONT_HEIGHT {
            let byte = FONT[base + row];
            for col in 0..FONT_WIDTH {
                let px = x + col;
                let py = y + row;
                if byte & (1 << (7 - col)) != 0 {
                    self.put_pixel(px, py, fg);
                } else {
                    self.put_pixel(px, py, bg);
                }
            }
        }
    }

    pub fn draw_hex(&mut self, x: usize, y: usize, mut num: usize, fg: u32, bg: u32) {
        let mut buf = [b'0'; 16];
        let mut i = 16;
        if num == 0 {
            i = 15;
            buf[15] = b'0';
        } else {
            while num > 0 && i > 0 {
                i -= 1;
                let d = (num & 0xF) as u8;
                buf[i] = if d < 10 { b'0' + d } else { b'A' + d - 10 };
                num >>= 4;
            }
        }
        let s = core::str::from_utf8(&buf[i..]).unwrap_or("0");
        self.draw_text(x, y, s, fg, bg);
    }
}

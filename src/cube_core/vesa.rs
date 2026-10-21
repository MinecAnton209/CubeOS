// src/cube_core/vesa.rs
// VESA Framebuffer поддержка

use crate::font::FONT;

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 16;

pub struct Framebuffer {
    pub addr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

impl Framebuffer {
    pub fn new(addr: *mut u32, width: usize, height: usize, pitch: usize) -> Self {
        Self { addr, width, height, pitch }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let offset = y * (self.pitch / 4) + x;
            unsafe {
                *self.addr.add(offset) = color;
            }
        }
    }

    pub fn clear(&mut self, color: u32) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.put_pixel(x, y, color);
            }
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, ch: char, fg: u32, bg: u32) {
        if ch < ' ' || ch > '~' {
            return;
        }
        let char_index = (ch as usize) - 0x20;
        let base = char_index * FONT_HEIGHT;

        for row in 0..FONT_HEIGHT {
            let byte = FONT[base + row];
            for col in 0..FONT_WIDTH {
                let px = x + col;
                let py = y + row;
                if px < self.width && py < self.height {
                    if byte & (1 << (7 - col)) != 0 {
                        self.put_pixel(px, py, fg);
                    } else {
                        self.put_pixel(px, py, bg);
                    }
                }
            }
        }
    }

    pub fn draw_text(&mut self, x: usize, y: usize, text: &str, fg: u32, bg: u32) {
        for (i, ch) in text.chars().enumerate() {
            self.draw_char(x + i * FONT_WIDTH, y, ch, fg, bg);
        }
    }
}

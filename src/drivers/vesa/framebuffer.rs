// src/drivers/vesa/framebuffer.rs

use core::ptr;

pub struct Framebuffer {
    pub addr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
}

impl Framebuffer {
    pub fn new(addr: *mut u32, width: usize, height: usize, pitch: usize) -> Self {
        Self {
            addr,
            width,
            height,
            pitch,
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            let offset = y * (self.pitch / 4) + x;
            unsafe {
                ptr::write_volatile(self.addr.add(offset), color);
            }
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.put_pixel(x + dx, y + dy, color);
            }
        }
    }

    pub fn clear(&mut self, color: u32) {
        let total_pixels = (self.height * self.width) as usize;
        for i in 0..total_pixels {
            unsafe {
                ptr::write_volatile(self.addr.add(i), color);
            }
        }
    }
}

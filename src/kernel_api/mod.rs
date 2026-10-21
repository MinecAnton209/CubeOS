// src/kernel_api/mod.rs

#![no_std]

pub struct KernelApi {
    // Поля для передачи информации драйверам
}

impl KernelApi {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn get_framebuffer(&self) -> Option<FramebufferInfo> {
        // Получение информации о фреймбуфере
        None
    }
}

pub struct FramebufferInfo {
    pub addr: u64,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub bpp: u8,
}

// src/mod.rs
#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(static_mut_refs)]

pub mod cube_core;
pub mod drivers;
pub mod fs;
pub mod kernel_api;
pub mod lib;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::cube_core::panic::panic_handler(info)
}

#[no_mangle]
pub extern "C" fn rust_main(magic: u32, info_addr: u32) -> ! {
    crate::cube_core::main::rust_main(magic, info_addr)
}

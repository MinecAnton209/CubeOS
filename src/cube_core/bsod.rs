// src/cube_core/bsod.rs
// BSOD для CubeOS v1.0

use crate::drivers::vesa::Graphics;

/// Синий BSOD для критических ошибок ядра
pub fn bsod(gfx: &mut Graphics, stop_code: &str, crash_addr: u64) -> ! {
    let bg: u32 = 0x00002A88; // Синий
    let fg: u32 = 0x00FFFFFF; // Белый

    gfx.clear(bg);

    gfx.draw_text(60, 30, "CubeOS", fg, bg);
    gfx.draw_text(40, 60, ":( Your PC ran into a problem and needs to restart.", fg, bg);

    let y_code = 100;
    gfx.draw_text(60, y_code, "Stop code: ", fg, bg);
    gfx.draw_text(60 + 11 * 8, y_code, stop_code, fg, bg);

    let y_info = 140;
    gfx.draw_text(60, y_info, "Crash address: 0x", fg, bg);
    gfx.draw_hex(60 + 18 * 8, y_info, crash_addr as usize, fg, bg);

    gfx.draw_text(60, y_info + 30, "Collecting crash dump...", fg, bg);
    gfx.draw_text(60, y_info + 60, "Press button on your PC to reboot.", fg, bg);

    loop {
        unsafe { core::arch::asm!("cli; hlt") }
    }
}

/// Зелёный BSOD для ошибок приложений
pub fn bsod_app(gfx: &mut Graphics, app_name: &str, error: &str) -> ! {
    let bg: u32 = 0x00006937; // Зелёный
    let fg: u32 = 0x00FFFFFF; // Белый

    gfx.clear(bg);

    gfx.draw_text(60, 30, "CubeOS", fg, bg);
    gfx.draw_text(40, 60, "Application Error", fg, bg);

    gfx.draw_text(60, 100, "Application: ", fg, bg);
    gfx.draw_text(60 + 13 * 8, 100, app_name, fg, bg);

    gfx.draw_text(60, 130, "Error: ", fg, bg);
    gfx.draw_text(60 + 7 * 8, 130, error, fg, bg);

    gfx.draw_text(60, 170, "This application has been terminated.", fg, bg);
    gfx.draw_text(60, 200, "Press button on your PC to reboot.", fg, bg);

    loop {
        unsafe { core::arch::asm!("cli; hlt") }
    }
}

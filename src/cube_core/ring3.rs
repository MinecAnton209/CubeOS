// src/cube_core/ring3.rs
use core::arch::asm;

pub unsafe fn enter_ring3(entry_point: u64, user_stack: u64) -> ! {
    crate::serial::send_string("[RING3] entry=0x");
    let mut hex = [0u8; 16];
    for i in 0..16 {
        let n = ((entry_point >> ((15 - i) * 4)) & 0xF) as u8;
        hex[i] = if n < 10 { b'0' + n } else { b'A' + n - 10 };
    }
    crate::serial::send_string(core::str::from_utf8(&hex).unwrap_or("?"));
    crate::serial::send_string("\n");

    let user_cs: u64 = 0x18 | 3;   // 0x1B
    let user_ds: u64 = 0x20 | 3;   // 0x23
    let rflags:  u64 = 0x3202;     // IF=1, IOPL=3, bit1=1

    // Грузим user-селекторы в DS/ES/FS/GS (в Ring 0 это разрешено:
    // max(CPL,RPL)=3 <= DPL=3).
    asm!(
        "mov ds, {s:x}",
        "mov es, {s:x}",
        "mov fs, {s:x}",
        "mov gs, {s:x}",
        s = in(reg) user_ds as u16,
    );

    asm!(
        "push {ss}",
        "push {rsp}",
        "push {flags}",
        "push {cs}",
        "push {rip}",
        "iretq",
        ss    = in(reg) user_ds,
         rsp   = in(reg) user_stack,
         flags = in(reg) rflags,
         cs    = in(reg) user_cs,
         rip   = in(reg) entry_point,
         options(noreturn)
    );
}

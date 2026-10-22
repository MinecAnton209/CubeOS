#![no_std]
#![feature(abi_x86_interrupt)]
#![allow(static_mut_refs)]

mod serial;
mod vga;
mod multiboot;
mod idt;
mod interrupts;
mod memory;
mod sync;
mod panic;
mod fs;
mod vesa_log;
mod ata;
mod elf;
mod loader;

use multiboot::MultibootInfo;
use core::arch::asm;
use fs::FSManager;
use ata::AtaDrive;

fn get_cpu_info() -> ([u8; 12], u32) {
    let mut vendor = [0u8; 12];
    let mut eax: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov [{vendor}], ebx",
            "mov [{vendor} + 4], edx",
            "mov [{vendor} + 8], ecx",
            "pop rbx",
            inout("eax") 0u32 => eax,
                         out("ecx") _,
                         out("edx") _,
                         vendor = in(reg) vendor.as_mut_ptr(),
        );
        core::arch::asm!(
            "push rbx", "cpuid", "pop rbx",
            inout("eax") 1u32 => eax,
                         out("ecx") _, out("edx") _,
        );
    }
    (vendor, eax)
}

fn write_num(buf: &mut [u8], mut n: usize) -> &str {
    let mut i = buf.len();
    if n == 0 { i -= 1; buf[i] = b'0'; }
    else { while n > 0 && i > 0 { i -= 1; buf[i] = (n % 10) as u8 + b'0'; n /= 10; } }
    core::str::from_utf8(&buf[i..]).unwrap_or("?")
}

fn write_hex(buf: &mut [u8], n: u64) -> &str {
    for i in 0..buf.len() {
        let nibble = ((n >> ((buf.len() - 1 - i) * 4)) & 0xF) as u8;
        buf[i] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
    }
    core::str::from_utf8(buf).unwrap_or("????????")
}

/// Единая функция логирования: пишет в Serial + VESA (если инициализирован)
pub fn log_both(vesa_log: &mut Option<vesa_log::VesaLogger>, level: &str, msg: &str) {
    // Всегда в Serial
    match level {
        "OK"    => serial::log_ok(msg),
        "INFO"  => serial::log_info(msg),
        "WARN"  => serial::log_warn(msg),
        "ERROR" => serial::log_error(msg),
        _       => serial::send_string(msg),
    }
    // И в VESA, если логгер доступен
    if let Some(ref mut logger) = vesa_log {
        logger.log(level, msg);
    }
}

#[no_mangle]
pub extern "C" fn rust_main(magic: u32, info_addr: u32) -> ! {
    // 1. СНАЧАЛА парсим Multiboot!
    let mb_info = unsafe { MultibootInfo::parse(info_addr) };
    let mut buf = [0u8; 20];

    // 2. Инициализация Serial и VGA
    serial::init();
    serial::send_string("\n========================================\n");
    serial::send_string("  CubeCORE v1.0 — Kernel Boot Sequence\n");
    serial::send_string("========================================\n");
    vga::init();

    // 3. Проверка Magic
    if magic != 0x36d76289 {
        serial::log_error("Invalid Multiboot2 magic!");
        serial::send_string("  Got: 0x");
        serial::send_string(write_hex(&mut buf, magic as u64));
        serial::send_string("\n");
        loop { unsafe { core::arch::asm!("cli; hlt") } }
    }
    serial::log_ok("Multiboot2 magic valid");

    // 4. Инициализация VESA логгера (живёт всю функцию как Option)
    let mut vesa_log = if let Some(fb) = mb_info.framebuffer {
        if fb.addr != 0 && fb.width > 0 && fb.height > 0 {
            let mut logger = vesa_log::VesaLogger::new(fb.addr, fb.width, fb.height, fb.pitch);
            logger.write_string("========================================\n");
            logger.write_string("  CubeCORE v1.0 — Kernel Boot Sequence\n");
            logger.write_string("========================================\n\n");
            logger.log("OK", "Framebuffer initialized");
            Some(logger)
        } else {
            None
        }
    } else {
        None
    };

    // 5. CPU Info
    let (vendor, eax) = get_cpu_info();
    let family = ((eax >> 8) & 0xF) + ((eax >> 20) & 0xFF);
    let model = ((eax >> 4) & 0xF) + (((eax >> 16) & 0xF) << 4);
    let stepping = eax & 0xF;

    serial::send_string("[INFO ] CPU: ");
    serial::send_string(core::str::from_utf8(&vendor).unwrap_or("Unknown"));
    serial::send_string(", Family "); serial::send_string(write_num(&mut buf, family as usize));
    serial::send_string(", Model "); serial::send_string(write_num(&mut buf, model as usize));
    serial::send_string(", Stepping "); serial::send_string(write_num(&mut buf, stepping as usize));
    serial::send_string("\n");

    if let Some(ref mut logger) = vesa_log {
        logger.write_string("[INFO ] CPU: ");
        logger.write_string(core::str::from_utf8(&vendor).unwrap_or("Unknown"));
        logger.write_string(", Family "); logger.write_string(write_num(&mut buf, family as usize));
        logger.write_string(", Model "); logger.write_string(write_num(&mut buf, model as usize));
        logger.write_string(", Stepping "); logger.write_string(write_num(&mut buf, stepping as usize));
        logger.write_string("\n");
    }

    // 6. Multiboot Info
    serial::send_string("[INFO ] Multiboot2 info at 0x");
    serial::send_string(write_hex(&mut buf, info_addr as u64));
    serial::send_string(", size ");
    serial::send_string(write_num(&mut buf, mb_info.total_size as usize));
    serial::send_string(" bytes\n");

    if let Some(ref mut logger) = vesa_log {
        logger.write_string("[INFO ] Multiboot2 info at 0x");
        logger.write_string(write_hex(&mut buf, info_addr as u64));
        logger.write_string(", size ");
        logger.write_string(write_num(&mut buf, mb_info.total_size as usize));
        logger.write_string(" bytes\n");
    }

    // 7. Memory Map
    serial::send_string("[INFO ] Memory map: ");
    serial::send_string(write_num(&mut buf, mb_info.memory_map.len()));
    serial::send_string(" entries\n");

    if let Some(ref mut logger) = vesa_log {
        logger.write_string("[INFO ] Memory map: ");
        logger.write_string(write_num(&mut buf, mb_info.memory_map.len()));
        logger.write_string(" entries\n");
    }

    for (idx, entry) in mb_info.memory_map.iter().enumerate() {
        serial::send_string("  [");
        serial::send_string(write_num(&mut buf, idx));
        serial::send_string("] ");

        let type_str = match entry.typ {
            1 => "Available ", 2 => "Reserved  ", 3 => "ACPI Rec  ",
            4 => "ACPI NVS  ", 5 => "Bad Mem   ", _ => "Unknown   ",
        };
        serial::send_string(type_str);
        serial::send_string(" 0x");
        serial::send_string(write_hex(&mut buf, entry.base_addr));
        serial::send_string(" (");
        serial::send_string(write_num(&mut buf, (entry.length / 1024) as usize));
        serial::send_string(" KB)\n");

        if let Some(ref mut logger) = vesa_log {
            logger.write_string("  [");
            logger.write_string(write_num(&mut buf, idx));
            logger.write_string("] ");
            logger.write_string(type_str);
            logger.write_string(" 0x");
            logger.write_string(write_hex(&mut buf, entry.base_addr));
            logger.write_string(" (");
            logger.write_string(write_num(&mut buf, (entry.length / 1024) as usize));
            logger.write_string(" KB)\n");
        }
    }

    let total_mb = mb_info.total_memory() / 1024 / 1024;
    let avail_mb = mb_info.available_memory() / 1024 / 1024;
    serial::send_string("[INFO ] Total RAM: ");
    serial::send_string(write_num(&mut buf, total_mb as usize));
    serial::send_string(" MB, Available: ");
    serial::send_string(write_num(&mut buf, avail_mb as usize));
    serial::send_string(" MB\n");

    if let Some(ref mut logger) = vesa_log {
        logger.write_string("[INFO ] Total RAM: ");
        logger.write_string(write_num(&mut buf, total_mb as usize));
        logger.write_string(" MB, Available: ");
        logger.write_string(write_num(&mut buf, avail_mb as usize));
        logger.write_string(" MB\n");
    }

    // 8. Memory Manager
    memory::init(mb_info.memory_map);
    serial::send_string("[INFO ] PMM: ");
    serial::send_string(write_num(&mut buf, memory::free_pages()));
    serial::send_string(" free pages (");
    serial::send_string(write_num(&mut buf, memory::free_memory_mb() as usize));
    serial::send_string(" MB)\n");
    serial::log_ok("Physical Memory Manager initialized");

    if let Some(ref mut logger) = vesa_log {
        logger.write_string("[INFO ] PMM: ");
        logger.write_string(write_num(&mut buf, memory::free_pages()));
        logger.write_string(" free pages (");
        logger.write_string(write_num(&mut buf, memory::free_memory_mb() as usize));
        logger.write_string(" MB)\n");
        logger.log("OK", "Physical Memory Manager initialized");
    }

    // 9. IDT, PIC, Keyboard
    unsafe { idt::init_idt(); }
    serial::log_ok("IDT initialized (256 entries)");
    if let Some(ref mut logger) = vesa_log { logger.log("OK", "IDT initialized (256 entries)"); }

    interrupts::remap_pic();
    interrupts::mask_all_irqs();
    serial::log_ok("PIC remapped (IRQ 0-15 → vectors 0x20-0x2F)");
    if let Some(ref mut logger) = vesa_log { logger.log("OK", "PIC remapped"); }

    interrupts::enable_irq(1);
    serial::log_ok("Keyboard enabled (IRQ1)");
    if let Some(ref mut logger) = vesa_log { logger.log("OK", "Keyboard enabled"); }

    unsafe { core::arch::asm!("sti"); }
    serial::log_ok("Interrupts enabled (IF=1)");
    if let Some(ref mut logger) = vesa_log { logger.log("OK", "Interrupts enabled"); }

    // ATA и FAT32 инициализация
    unsafe { asm!("cli"); }
    let mut ata = AtaDrive::new();
    let mut fs = FSManager::new();
    ata.init();

    // === ATA инициализация ===
    log_both(&mut vesa_log, "INFO", "Initializing ATA controller...");

    if ata.exists {
        log_both(&mut vesa_log, "OK", "ATA controller detected");

        // Инициализируем ATA-драйвер полностью
        let mut ata = ata::AtaDrive::new();
        ata.init();

        if ata.exists {
            log_both(&mut vesa_log, "OK", "ATA drive initialized");

            // Монтируем FAT32 (второй раздел, индекс 1)
            let mut fs = fs::FSManager::new();
            if fs.mount_fat32(&ata, 1) {
                log_both(&mut vesa_log, "OK", "FAT32 filesystem mounted");

                // Читаем конфиг
                let mut config_buf = [0u8; 512];
                if let Some(size) = fs.read_file(&ata, "/SYSTEM/CONFIG/BOOT.CFG", &mut config_buf) {
                    if let Ok(text) = core::str::from_utf8(&config_buf[..size]) {
                        log_both(&mut vesa_log, "OK", "Boot config loaded");

                        // Парсим конфиг и ищем cmdline
                        let mut cmdline = None;
                        let mut start = 0;

                        for (i, &byte) in text.as_bytes().iter().enumerate() {
                            if byte == b'\n' || byte == b'\r' || i == text.len() - 1 {
                                let line_end = if i == text.len() - 1 { i + 1 } else { i };
                                let line = &text[start..line_end];
                                let line = line.trim();

                                // Ищем строку cmdline = "..."
                                if line.starts_with("cmdline") {
                                    if let Some(eq_pos) = line.find('=') {
                                        let value = line[eq_pos + 1..].trim();
                                        let value = value.trim_matches('"');
                                        if !value.is_empty() {
                                            cmdline = Some(value);
                                        }
                                    }
                                }

                                // Выводим строку конфига
                                if !line.is_empty() && !line.starts_with('#') {
                                    log_both(&mut vesa_log, "CFG", line);
                                }

                                start = i + 1;
                                if byte == b'\r' && i + 1 < text.len() && text.as_bytes()[i + 1] == b'\n' {
                                    start = i + 2;
                                }
                            }
                        }

                        // Запускаем программу из cmdline
                        if let Some(cmd) = cmdline {
                            log_both(&mut vesa_log, "INFO", "Launching program from cmdline...");
                            if loader::exec(&ata, &fs, cmd) {
                                log_both(&mut vesa_log, "OK", "Program launched successfully");
                            } else {
                                log_both(&mut vesa_log, "ERROR", "Failed to launch program");
                            }
                        } else {
                            log_both(&mut vesa_log, "INFO", "No cmdline in config, skipping program launch");
                        }
                    }
                    if let Ok(text) = core::str::from_utf8(&config_buf[..size]) {
                        for line in text.lines() {
                            log_both(&mut vesa_log, "CFG", line);
                        }
                    }
                } else {
                    log_both(&mut vesa_log, "WARN", "boot.cfg not found");
                }
            } else {
                log_both(&mut vesa_log, "ERROR", "Failed to mount FAT32");
            }
        } else {
            log_both(&mut vesa_log, "WARN", "No ATA drive detected");
        }
    }

    serial::send_string("\n[  OK ] CubeCORE v1.0 boot complete!\n");
    serial::send_string("========================================\n\n");

    if let Some(ref mut logger) = vesa_log {
        logger.write_string("\n[  OK ] CubeCORE v1.0 boot complete!\n");
        logger.write_string("========================================\n\n");
    }

    // === ЭТАП 15: Загрузка /bin/csh ===
    log_both(&mut vesa_log, "INFO", "Loading /bin/csh...");
    if loader::exec(&ata, &fs, "/bin/csh") {
        log_both(&mut vesa_log, "OK", "Shell launched");
    } else {
        log_both(&mut vesa_log, "WARN", "Failed to launch shell");
    }

    // Главный цикл
    loop {
        if let Some(sc) = interrupts::keyboard_pop() {
            serial::send_string("[INPUT] Scancode: 0x");
            let mut hex_buf = [0u8; 2];
            hex_buf[0] = if (sc >> 4) < 10 { b'0' + (sc >> 4) } else { b'A' + (sc >> 4) - 10 };
            hex_buf[1] = if (sc & 0xF) < 10 { b'0' + (sc & 0xF) } else { b'A' + (sc & 0xF) - 10 };
            serial::send_string(core::str::from_utf8(&hex_buf).unwrap());
            serial::send_string("\n");

            if let Some(ref mut logger) = vesa_log {
                logger.write_string("[INPUT] Scancode: 0x");
                logger.write_string(core::str::from_utf8(&hex_buf).unwrap());
                logger.write_string("\n");
            }
        }
        unsafe { core::arch::asm!("hlt") }
    }
}

// src/cube_core/loader.rs — загрузчик программ
use crate::elf;
use crate::fs::FSManager;
use crate::ata::AtaDrive;

static mut PROGRAM_BUF: [u8; 65536] = [0; 65536];

pub fn exec(ata: &AtaDrive, fs: &FSManager, path: &str) -> bool {
    crate::serial::send_string("[LOADER] Loading: ");
    crate::serial::send_string(path);
    crate::serial::send_string("\n");

    unsafe {
        let size = match fs.read_file(ata, path, &mut PROGRAM_BUF) {
            Some(s) => {
                crate::serial::send_string("[LOADER] Read ");
                let mut num_buf = [0u8; 20];
                let mut n = s;
                let mut i = 20;
                while n > 0 { i -= 1; num_buf[i] = (n % 10) as u8 + b'0'; n /= 10; }
                crate::serial::send_string(core::str::from_utf8(&num_buf[i..]).unwrap_or("?"));
                crate::serial::send_string(" bytes\n");
                s
            },
            None => {
                crate::serial::send_string("[LOADER] ERROR: File not found!\n");
                return false;
            }
        };

        if size == 0 {
            crate::serial::send_string("[LOADER] ERROR: File is empty!\n");
            return false;
        }

        // Проверяем ELF magic
        if PROGRAM_BUF[0] != 0x7F || PROGRAM_BUF[1] != b'E' || PROGRAM_BUF[2] != b'L' || PROGRAM_BUF[3] != b'F' {
            crate::serial::send_string("[LOADER] ERROR: Not a valid ELF file!\n");
            return false;
        }

        crate::serial::send_string("[LOADER] Valid ELF detected\n");

        let program = match elf::load_elf(&PROGRAM_BUF[..size]) {
            Some(p) => p,
            None => {
                crate::serial::send_string("[LOADER] ERROR: ELF load failed\n");
                return false;
            }
        };

        crate::serial::send_string("[LOADER] Entry: 0x");
        let mut hex_buf = [0u8; 16];
        for i in 0..16 {
            let nibble = ((program.entry_point >> ((15 - i) * 4)) & 0xF) as u8;
            hex_buf[i] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
        }
        crate::serial::send_string(core::str::from_utf8(&hex_buf).unwrap_or("?"));
        crate::serial::send_string("\n");

        crate::serial::send_string("[LOADER] Jumping...\n");

        let entry: extern "C" fn() = core::mem::transmute(program.entry_point);
        entry();
    }

    false
}

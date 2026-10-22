// src/cube_core/elf.rs — парсер ELF64
use core::mem::size_of;

// ELF magic
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

// Типы сегментов
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;

// Флаги сегментов
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

// ELF64 заголовок
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

// Program header (сегмент)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

impl Elf64Header {
    /// Проверяет валидность ELF заголовка
    pub fn validate(&self) -> bool {
        // Проверяем magic
        if self.e_ident[0..4] != ELF_MAGIC {
            return false;
        }
        // Проверяем 64-bit
        if self.e_ident[4] != 2 {
            return false;
        }
        // Проверяем little-endian
        if self.e_ident[5] != 1 {
            return false;
        }
        // Проверяем x86-64
        if self.e_machine != 0x3E {
            return false;
        }
        // Проверяем исполняемый файл
        if self.e_type != 2 && self.e_type != 3 {
            // ET_EXEC = 2, ET_DYN = 3 (PIE)
            return false;
        }
        true
    }
}

/// Информация о загруженной программе
pub struct LoadedProgram {
    pub entry_point: u64,
    pub base_addr: u64,
    pub size: u64,
}

/// Загружает ELF-файл из буфера в память
/// Возвращает точку входа и базовый адрес
pub fn load_elf(data: &[u8]) -> Option<LoadedProgram> {
    if data.len() < size_of::<Elf64Header>() {
        return None;
    }

    let header: &Elf64Header = unsafe { &*(data.as_ptr() as *const Elf64Header) };

    if !header.validate() {
        crate::serial::send_string("[ELF] Invalid ELF header\n");
        return None;
    }

    crate::serial::send_string("[ELF] Entry point: 0x");
    let mut buf = [0u8; 20];
    for j in 0..16 {
        let nibble = ((header.e_entry >> ((15 - j) * 4)) & 0xF) as u8;
        buf[j] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
    }
    crate::serial::send_string(core::str::from_utf8(&buf).unwrap_or("????????????????"));
    crate::serial::send_string("\n");

    crate::serial::send_string("[ELF] Program headers: ");
    let mut n = header.e_phnum as usize;
    let mut idx = 20;
    while n > 0 { idx -= 1; buf[idx] = (n % 10) as u8 + b'0'; n /= 10; }
    crate::serial::send_string(core::str::from_utf8(&buf[idx..]).unwrap_or("?"));
    crate::serial::send_string("\n");

    let mut base_addr = u64::MAX;
    let mut end_addr = 0u64;

    for ph_idx in 0..header.e_phnum as usize {  // ← переименовали i в ph_idx
        let ph_offset = header.e_phoff as usize + ph_idx * header.e_phentsize as usize;
        if ph_offset + size_of::<Elf64Phdr>() > data.len() {
            crate::serial::send_string("[ELF] Program header out of bounds\n");
            return None;
        }

        let phdr: &Elf64Phdr = unsafe { &*((data.as_ptr().add(ph_offset)) as *const Elf64Phdr) };

        if phdr.p_type != PT_LOAD {
            continue;
        }

        crate::serial::send_string("[ELF] Loading segment: vaddr=0x");
        for j in 0..16 {
            let nibble = ((phdr.p_vaddr >> ((15 - j) * 4)) & 0xF) as u8;
            buf[j] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
        }
        crate::serial::send_string(core::str::from_utf8(&buf).unwrap_or("????????????????"));
        crate::serial::send_string(", filesz=");
        n = phdr.p_filesz as usize;
        idx = 20;
        while n > 0 { idx -= 1; buf[idx] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[idx..]).unwrap_or("?"));
        crate::serial::send_string(", memsz=");
        n = phdr.p_memsz as usize;
        idx = 20;
        while n > 0 { idx -= 1; buf[idx] = (n % 10) as u8 + b'0'; n /= 10; }
        crate::serial::send_string(core::str::from_utf8(&buf[idx..]).unwrap_or("?"));
        crate::serial::send_string("\n");

        let file_end = phdr.p_offset as usize + phdr.p_filesz as usize;
        if file_end > data.len() {
            crate::serial::send_string("[ELF] Segment extends beyond file\n");
            return None;
        }

        let dst = phdr.p_vaddr as *mut u8;
        let src = unsafe { data.as_ptr().add(phdr.p_offset as usize) };  // ← обернули в unsafe

        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, phdr.p_filesz as usize);

            if phdr.p_memsz > phdr.p_filesz {
                let bss_start = dst.add(phdr.p_filesz as usize);
                let bss_size = (phdr.p_memsz - phdr.p_filesz) as usize;
                core::ptr::write_bytes(bss_start, 0, bss_size);
            }
        }

        if phdr.p_vaddr < base_addr {
            base_addr = phdr.p_vaddr;
        }
        let seg_end = phdr.p_vaddr + phdr.p_memsz;
        if seg_end > end_addr {
            end_addr = seg_end;
        }
    }

    if base_addr == u64::MAX {
        crate::serial::send_string("[ELF] No PT_LOAD segments found\n");
        return None;
    }

    crate::serial::send_string("[ELF] Loaded: 0x");
    for j in 0..16 {
        let nibble = ((base_addr >> ((15 - j) * 4)) & 0xF) as u8;
        buf[j] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
    }
    crate::serial::send_string(core::str::from_utf8(&buf).unwrap_or("????????????????"));
    crate::serial::send_string(" - 0x");
    for j in 0..16 {
        let nibble = ((end_addr >> ((15 - j) * 4)) & 0xF) as u8;
        buf[j] = if nibble < 10 { b'0' + nibble } else { b'A' + nibble - 10 };
    }
    crate::serial::send_string(core::str::from_utf8(&buf).unwrap_or("????????????????"));
    crate::serial::send_string("\n");

    Some(LoadedProgram {
        entry_point: header.e_entry,
         base_addr,
         size: end_addr - base_addr,
    })
}

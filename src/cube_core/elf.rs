// src/cube_core/elf.rs
use core::mem::size_of;
use crate::checks;

pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub const PT_LOAD: u32 = 1;

// Флаги сегментов
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

// Linker-provided end of the kernel image, including its stack region.
extern "C" {
    static _kernel_end: u8;
}

// boot.asm identity-maps the first 4 GiB, so user segments must stay below this address.
const LOAD_REGION_END: u64 = 0x1_0000_0000;

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

#[repr(C, packed)]
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

pub struct LoadedProgram {
    pub entry_point: u64,
    pub base_addr: u64,
    pub size: u64,
}

fn kernel_end() -> u64 {
    unsafe { &_kernel_end as *const u8 as u64 }
}

fn read_phdr(data: &[u8], header: &Elf64Header, idx: usize) -> Option<Elf64Phdr> {
    let offset = header.e_phoff.checked_add(idx as u64 * header.e_phentsize as u64)?;
    if !checks::range_fits(offset, size_of::<Elf64Phdr>() as u64, data.len() as u64) {
        return None;
    }
    // Program headers are not guaranteed to be aligned inside the file buffer.
    Some(unsafe { core::ptr::read_unaligned(data.as_ptr().add(offset as usize) as *const Elf64Phdr) })
}

/// Загружает ELF-файл из буфера в память
/// Возвращает точку входа и базовый адрес
pub fn load_elf(data: &[u8]) -> Option<LoadedProgram> {
    if data.len() < size_of::<Elf64Header>() {
        return None;
    }

    let header = unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Elf64Header) };

    if header.e_ident[0..4] != ELF_MAGIC {
        return None;
    }

    if (header.e_phentsize as usize) < size_of::<Elf64Phdr>() {
        crate::serial::send_string("[ELF] Program header size too small\n");
        return None;
    }

    let region_start = kernel_end();
    let file_len = data.len() as u64;

    // Validate every segment and the entry point before writing any memory,
    // so a malformed file cannot leave the kernel half-overwritten.
    let mut base_addr = u64::MAX;
    let mut end_addr = 0u64;
    let mut entry_ok = false;

    for ph_idx in 0..header.e_phnum as usize {
        let phdr = read_phdr(data, &header, ph_idx)?;
        if phdr.p_type != PT_LOAD {
            continue;
        }

        if !checks::elf_segment_fits(
            phdr.p_vaddr,
            phdr.p_offset,
            phdr.p_filesz,
            phdr.p_memsz,
            file_len,
            region_start,
            LOAD_REGION_END,
        ) {
            crate::serial::send_string("[ELF] Segment rejected: outside load region or malformed\n");
            return None;
        }

        if phdr.p_flags & PF_X != 0 && checks::in_segment(header.e_entry, phdr.p_vaddr, phdr.p_memsz) {
            entry_ok = true;
        }

        base_addr = base_addr.min(phdr.p_vaddr);
        end_addr = end_addr.max(phdr.p_vaddr + phdr.p_memsz);
    }

    if base_addr == u64::MAX {
        crate::serial::send_string("[ELF] No PT_LOAD segments found\n");
        return None;
    }

    if !entry_ok {
        crate::serial::send_string("[ELF] Entry point is not inside an executable segment\n");
        return None;
    }

    for ph_idx in 0..header.e_phnum as usize {
        let phdr = read_phdr(data, &header, ph_idx)?;
        if phdr.p_type != PT_LOAD {
            continue;
        }

        unsafe {
            let dst = phdr.p_vaddr as *mut u8;
            let src = data.as_ptr().add(phdr.p_offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, phdr.p_filesz as usize);

            if phdr.p_memsz > phdr.p_filesz {
                let bss_start = dst.add(phdr.p_filesz as usize);
                core::ptr::write_bytes(bss_start, 0, (phdr.p_memsz - phdr.p_filesz) as usize);
            }
        }
    }

    crate::serial::send_string("[ELF] Loaded segments\n");

    Some(LoadedProgram {
        entry_point: header.e_entry,
        base_addr,
        size: end_addr - base_addr,
    })
}


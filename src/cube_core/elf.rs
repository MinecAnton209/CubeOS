// src/cube_core/elf.rs
use core::mem::size_of;
use core::ptr;

pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub const PT_LOAD: u32 = 1;

#[repr(C, packed)]
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

pub fn load_elf(data: &[u8]) -> Option<LoadedProgram> {
    if data.len() < size_of::<Elf64Header>() {
        return None;
    }

    let header = unsafe { ptr::read_unaligned(data.as_ptr() as *const Elf64Header) };

    if header.e_ident[0..4] != ELF_MAGIC {
        return None;
    }

    let phdr_size = header.e_phentsize as usize;
    let phdr_num = header.e_phnum as usize;
    let phdr_offset = header.e_phoff as usize;

    for i in 0..phdr_num {
        let offset = phdr_offset + i * phdr_size;
        if offset + size_of::<Elf64Phdr>() > data.len() {
            return None;
        }

        let phdr = unsafe {
            ptr::read_unaligned(data.as_ptr().add(offset) as *const Elf64Phdr)
        };

        // Загружаем ТОЛЬКО сегменты типа PT_LOAD (1)
        if phdr.p_type == PT_LOAD {
            crate::serial::send_string("[ELF] Loading PT_LOAD segment...\n");

            // 1. ВАЖНО: Мапим страницы памяти под этот сегмент!
            // Нужно пройтись от p_vaddr до (p_vaddr + p_memsz) с шагом 4096 (или 2MB)
            let start_addr = phdr.p_vaddr;
            let end_addr = phdr.p_vaddr + phdr.p_memsz;

            // Здесь ТВОЯ функция маппинга страниц с флагом USER!
            // map_memory_range(start_addr, end_addr, PAGE_USER | PAGE_WRITABLE);

            // 2. Копируем данные из файла в память
            let src_offset = phdr.p_offset as usize;
            let filesz = phdr.p_filesz as usize;

            if src_offset + filesz <= data.len() {
                unsafe {
                    ptr::copy_nonoverlapping(
                        data.as_ptr().add(src_offset),
                                             phdr.p_vaddr as *mut u8,
                                             filesz
                    );

                    // 3. Если memsz > filesz, зануляем оставшуюся часть (.bss)
                    let memsz = phdr.p_memsz as usize;
                    if memsz > filesz {
                        ptr::write_bytes(
                            (phdr.p_vaddr + phdr.p_filesz) as *mut u8,
                                         0,
                                         memsz - filesz
                        );
                    }
                }
            }
        }
    }

    Some(LoadedProgram {
        entry_point: header.e_entry,
         base_addr: 0x01000000,
         size: 0,
    })
}


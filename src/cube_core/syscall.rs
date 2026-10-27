// src/cube_core/syscall.rs
use core::arch::asm;

const IA32_EFER:  u32 = 0xC0000080;
const IA32_STAR:  u32 = 0xC0000081;
const IA32_LSTAR: u32 = 0xC0000082;
const IA32_FMASK: u32 = 0xC0000084;

// Номера syscall (должны совпадать с cube_syscall)
pub const SYS_READ:     u64 = 0;
pub const SYS_WRITE:    u64 = 1;
pub const SYS_OPEN:     u64 = 2;
pub const SYS_CLOSE:    u64 = 3;
pub const SYS_EXIT:     u64 = 5;
pub const SYS_GETDENTS: u64 = 11;
pub const SYS_GETCWD:   u64 = 12;
pub const SYS_CHDIR:    u64 = 13;
pub const SYS_MKDIR:    u64 = 14;
pub const SYS_UNLINK:   u64 = 34;
pub const SYS_DEBUG:    u64 = 78;

// open() flags
const O_RDONLY:    u64 = 0o0;
const O_WRONLY:    u64 = 0o1;
const O_RDWR:      u64 = 0o2;
const O_DIRECTORY: u64 = 0o200000;

// file types
const DT_REG: u8 = 8;
const DT_DIR: u8 = 4;

static mut KERNEL_RSP: u64 = 0;
static mut USER_RSP:   u64 = 0;

// Таблица fd
const MAX_FD: usize = 16;

#[derive(Copy, Clone)]
enum FdKind {
    Free,
    File { cluster: u32, size: u32, offset: u32 },
    Dir  { cluster: u32 },
}

#[derive(Copy, Clone)]
struct Fd { kind: FdKind }

static mut FDS: [Fd; MAX_FD] = [Fd { kind: FdKind::Free }; MAX_FD];

// Текущая директория
static mut CWD_CLUSTER: u32 = 0;
static mut CWD_PATH:    [u8; 256] = [0; 256];
static mut CWD_PATH_LEN: usize = 1;

// Ссылки на FS и ATA
static mut FS_PTR:  *const crate::fs::FSManager = core::ptr::null();
static mut ATA_PTR: *const crate::ata::AtaDrive  = core::ptr::null();

unsafe fn fs() -> &'static crate::fs::FSManager { &*FS_PTR }
unsafe fn ata() -> &'static crate::ata::AtaDrive { &*ATA_PTR }

pub fn set_fs_refs(fs: *const crate::fs::FSManager, ata: *const crate::ata::AtaDrive) {
    unsafe {
        FS_PTR = fs;
        ATA_PTR = ata;
        if let Some(ref f) = (*fs).fat32 {
            CWD_CLUSTER = f.bpb.root_cluster;
        }
        CWD_PATH[0] = b'/';
        CWD_PATH_LEN = 1;
    }
}

unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi);
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | lo as u64
}

pub fn init_syscall(kernel_rsp: u64) {
    unsafe {
        KERNEL_RSP = kernel_rsp;

        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1);

        wrmsr(IA32_STAR, (0x08u64 << 48) | (0x18u64 << 32));
        wrmsr(IA32_LSTAR, syscall_entry as u64);
        wrmsr(IA32_FMASK, 0x700);

        crate::serial::send_string("[SYSCALL] syscall/sysret configured\n");
        crate::serial::send_string("[TSS] rsp0=");
        print_u64(kernel_rsp);
        crate::serial::send_string("\n");
    }
}

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        // сохранить user RSP, переключиться на kernel RSP
        "mov [rip + {user_rsp}], rsp",
        "mov rsp, [rip + {kernel_rsp}]",

        // user RIP/RFLAGS → на стек (для iretq)
        "push rcx",
        "push r11",

        // сохранить аргументы и caller-saved
        "push rdi", "push rsi", "push rdx",
        "push r8", "push r9", "push r10",

        // kernel DS/ES
        "push rax",
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",
        "pop rax",

        // System V: (num=rdi, a1=rsi, a2=rdx, a3=rcx)
        "mov rcx, rdx",
        "mov rdx, rsi",
        "mov rsi, rdi",
        "mov rdi, rax",
        "call {handler}",

        // восстановить user DS/ES
        "push rax",
        "mov ax, 0x23",
        "mov ds, ax",
        "mov es, ax",
        "pop rax",

        // восстановить регистры
        "pop r10", "pop r9", "pop r8",
        "pop rdx", "pop rsi", "pop rdi",
        "pop r11",   // user RFLAGS
        "pop rcx",   // user RIP

        // iretq-фрейм: SS, RSP, RFLAGS, CS, RIP
        "push 0x23",
        "push qword ptr [rip + {user_rsp}]",
        "push r11",
        "push 0x1B",
        "push rcx",
        "iretq",

        user_rsp   = sym USER_RSP,
        kernel_rsp = sym KERNEL_RSP,
        handler    = sym syscall_dispatch,
    );
}

// === Вспомогательные ===

// Проверка, что указатель находится в пользовательском пространстве (canonical user addresses)
// Для x86_64 canonical user addresses: 0x0000_7FFF_FFFF_FFFF и ниже
fn is_user_address(ptr: u64) -> bool {
    ptr <= 0x00007FFFFFFFFFFF
}

unsafe fn copy_user_str(ptr: u64, len: u64, out: &mut [u8]) -> Option<usize> {
    if ptr == 0 || len == 0 || len as usize > out.len() {
        return None;
    }
    // Защита от обращения к памяти ядра из Ring 3
    if !is_user_address(ptr) || !is_user_address(ptr + len - 1) {
        return None;
    }
    let src = ptr as *const u8;
    for i in 0..len as usize {
        out[i] = *src.add(i);
    }
    Some(len as usize)
}

fn part_len(p: &[u8; 32]) -> usize {
    p.iter().position(|&b| b == 0).unwrap_or(32)
}

unsafe fn resolve_path(path: &[u8]) -> Option<(u32, [u8; 11])> {
    let fs = fs();
    let fat = fs.fat32.as_ref()?;

    if path == b"/" {
        return Some((fat.bpb.root_cluster, *b"           "));
    }

    let mut dir_cluster = if path.first() == Some(&b'/') {
        fat.bpb.root_cluster
    } else {
        CWD_CLUSTER
    };

    let mut parts: [[u8; 32]; 16] = [[0; 32]; 16];
    let mut nparts = 0;
    let mut cur_len = 0;
    for &b in path {
        if b == b'/' {
            if cur_len > 0 { nparts += 1; cur_len = 0; }
        } else if cur_len < 32 && nparts < 16 {
            parts[nparts][cur_len] = b;
            cur_len += 1;
        }
    }
    if cur_len > 0 { nparts += 1; }
    if nparts == 0 { return None; }

    for i in 0..nparts.saturating_sub(1) {
        let l = part_len(&parts[i]);
        let comp = &parts[i][..l];
        let name83 = crate::fs::fat32::Fat32::name_to_83(core::str::from_utf8(comp).ok()?)?;
        let mut entries = [crate::fs::fat32::FatDirEntry::new(); 64];
        let count = fat.list_dir_entries(ata(), dir_cluster, &mut entries);
        let mut found = false;
        for j in 0..count {
            if entries[j].name == name83 && entries[j].is_directory() {
                dir_cluster = entries[j].cluster();
                found = true;
                break;
            }
        }
        if !found { return None; }
    }

    let last = nparts - 1;
    let l = part_len(&parts[last]);
    let comp = &parts[last][..l];
    let name83 = crate::fs::fat32::Fat32::name_to_83(core::str::from_utf8(comp).ok()?)?;
    Some((dir_cluster, name83))
}

unsafe fn fd_alloc() -> Option<usize> {
    for i in 3..MAX_FD {
        if let FdKind::Free = FDS[i].kind {
            return Some(i);
        }
    }
    None
}

fn print_u64(mut n: u64) {
    let mut b = [0u8; 20];
    let mut i = 20;
    if n == 0 { i -= 1; b[i] = b'0'; }
    else { while n > 0 { i -= 1; b[i] = (n % 10) as u8 + b'0'; n /= 10; } }
    crate::serial::send_string(core::str::from_utf8(&b[i..]).unwrap_or("?"));
}

fn scancode_to_ascii(sc: u8) -> Option<u8> {
    if sc & 0x80 != 0 { return None; }
    const MAP: [u8; 0x3A] = [
        0, 0, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
        b'-', b'=', 0, 0,
        b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p',
        b'[', b']', b'\n', 0,
        b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l',
        b';', b'\'', b'`', 0,
        b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm',
        b',', b'.', b'/', 0,
        b'*', 0, b' ',
    ];
    if (sc as usize) < MAP.len() {
        let c = MAP[sc as usize];
        if c != 0 { Some(c) } else { None }
    } else { None }
}

// === Главный dispatcher ===

#[no_mangle]
extern "C" fn syscall_dispatch(num: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    match num {
        SYS_WRITE => {
            if a1 <= 2 && a2 != 0 && a3 > 0 && a3 <= 0x10000 {
                let ptr = a2 as *const u8;
                for i in 0..a3 as usize {
                    unsafe { crate::serial::send_byte(*ptr.add(i)); }
                }
                a3
            } else { 0 }
        }
        SYS_READ => {
            let fd = a1 as usize;
            if a2 == 0 || a3 == 0 { return u64::MAX; }

            // stdin — клавиатура (без sti/hlt!)
            if fd == 0 {
                let buf = a2 as *mut u8;
                let max = a3 as usize;
                let mut n = 0usize;
                // Читаем только то, что уже есть в буфере, не блокируя ядро
                while n < max {
                    if let Some(sc) = crate::interrupts::keyboard_pop() {
                        if let Some(ch) = scancode_to_ascii(sc) {
                            unsafe { *buf.add(n) = ch; }
                            n += 1;
                            unsafe { crate::serial::send_byte(ch); }
                            if ch == b'\n' { break; }
                        }
                    } else {
                        // Данных нет, выходим. Шелл сам решит, что делать (повторить или отдать квант)
                        break;
                    }
                }
                return n as u64;
            }

            // Чтение из файлов
            if fd >= MAX_FD { return u64::MAX; }

            unsafe {
                match FDS[fd].kind {
                    FdKind::File { cluster, size, offset } => {
                        let remaining = size.saturating_sub(offset) as usize;
                        let to_read = (a3 as usize).min(remaining);
                        if to_read == 0 { return 0; }
                        let fat = match fs().fat32.as_ref() {
                            Some(f) => f,
                            None => return u64::MAX,
                        };
                        let mut tmp = [0u8; 65536];
                        if fat.read_file(ata(), cluster, &mut tmp, size).is_none() {
                            return u64::MAX;
                        }
                        let dst = a2 as *mut u8;
                        for i in 0..to_read {
                            *dst.add(i) = tmp[offset as usize + i];
                        }
                        if let FdKind::File { ref mut offset, .. } = FDS[fd].kind {
                            *offset += to_read as u32;
                        }
                        to_read as u64
                    }
                    FdKind::Dir { .. } => 0,
                    FdKind::Free => u64::MAX,
                }
            }
        }
        SYS_OPEN => {
            // a1 = flags, a2 = path_ptr, a3 = path_len
            if a2 == 0 || a3 == 0 { return u64::MAX; }
            let mut pathbuf = [0u8; 256];
            let pathlen = match unsafe { copy_user_str(a2, a3, &mut pathbuf) } {
                Some(n) => n, None => return u64::MAX,
            };
            let path = &pathbuf[..pathlen];
            unsafe {
                let resolved = match resolve_path(path) { Some(r) => r, None => return u64::MAX };
                let (dir_cluster, name83) = resolved;
                let fat = match fs().fat32.as_ref() { Some(f) => f, None => return u64::MAX };

                if dir_cluster == fat.bpb.root_cluster && name83 == *b"           " {
                    if let Some(fd) = fd_alloc() {
                        FDS[fd].kind = FdKind::Dir { cluster: fat.bpb.root_cluster };
                        return fd as u64;
                    }
                    return u64::MAX;
                }

                let mut entries = [crate::fs::fat32::FatDirEntry::new(); 64];
                let count = fat.list_dir_entries(ata(), dir_cluster, &mut entries);
                for j in 0..count {
                    if entries[j].name == name83 {
                        if entries[j].is_directory() {
                            if a1 & O_DIRECTORY != 0 || a1 & O_RDONLY != 0 {
                                if let Some(fd) = fd_alloc() {
                                    FDS[fd].kind = FdKind::Dir { cluster: entries[j].cluster() };
                                    return fd as u64;
                                }
                            }
                        } else {
                            if let Some(fd) = fd_alloc() {
                                FDS[fd].kind = FdKind::File {
                                    cluster: entries[j].cluster(),
                                    size: entries[j].size,
                                    offset: 0,
                                };
                                return fd as u64;
                            }
                        }
                        return u64::MAX;
                    }
                }
                u64::MAX
            }
        }
        SYS_CLOSE => {
            if a1 < MAX_FD as u64 {
                unsafe { FDS[a1 as usize].kind = FdKind::Free; return 0; }
            }
            u64::MAX
        }
        SYS_GETDENTS => {
            let fd = a1 as usize;
            if fd >= MAX_FD || a2 == 0 || a3 == 0 { return u64::MAX; }
            unsafe {
                let cluster = match FDS[fd].kind {
                    FdKind::Dir { cluster } => cluster,
                    _ => return u64::MAX,
                };
                let fat = match fs().fat32.as_ref() { Some(f) => f, None => return u64::MAX };
                let mut entries = [crate::fs::fat32::FatDirEntry::new(); 64];
                let count = fat.list_dir_entries(ata(), cluster, &mut entries);
                let dst = a2 as *mut u8;
                let max = a3 as usize;
                let mut written = 0usize;
                for j in 0..count {
                    let e = &entries[j];
                    let mut name = [0u8; 13];
                    let nlen = e.name_to_buf(&mut name);
                    let reclen = (19 + nlen + 1 + 7) & !7;
                    if written + reclen > max { break; }

                    let ent = dst.add(written);
                    *(ent as *mut u64) = e.cluster() as u64;
                    *((ent.add(8)) as *mut u64) = j as u64;
                    *((ent.add(16)) as *mut u16) = reclen as u16;
                    *(ent.add(18)) = if e.is_directory() { DT_DIR } else { DT_REG };
                    for i in 0..nlen { *(ent.add(19 + i)) = name[i]; }
                    *(ent.add(19 + nlen)) = 0;
                    written += reclen;
                }
                written as u64
            }
        }
        SYS_GETCWD => {
            if a2 == 0 || a3 == 0 { return u64::MAX; }
            unsafe {
                let dst = a2 as *mut u8;
                let len = a3 as usize;
                let to_copy = CWD_PATH_LEN.min(len);
                for i in 0..to_copy { *dst.add(i) = CWD_PATH[i]; }
                to_copy as u64
            }
        }
        SYS_CHDIR => {
            if a2 == 0 || a3 == 0 { return u64::MAX; }
            let mut pathbuf = [0u8; 256];
            let pathlen = match unsafe { copy_user_str(a2, a3, &mut pathbuf) } {
                Some(n) => n, None => return u64::MAX,
            };
            let path = &pathbuf[..pathlen];
            unsafe {
                if path == b".." {
                    let fat = match fs().fat32.as_ref() { Some(f) => f, None => return u64::MAX };
                    let mut entries = [crate::fs::fat32::FatDirEntry::new(); 64];
                    let count = fat.list_dir_entries(ata(), CWD_CLUSTER, &mut entries);
                    for j in 0..count {
                        if entries[j].name == *b"..         " {
                            CWD_CLUSTER = if entries[j].cluster() == 0 {
                                fat.bpb.root_cluster
                            } else { entries[j].cluster() };
                            let mut i = CWD_PATH_LEN;
                            while i > 1 && CWD_PATH[i - 1] != b'/' { i -= 1; }
                            CWD_PATH_LEN = i;
                            return 0;
                        }
                    }
                    return u64::MAX;
                }
                let resolved = match resolve_path(path) { Some(r) => r, None => return u64::MAX };
                let (dir_cluster, name83) = resolved;
                let fat = match fs().fat32.as_ref() { Some(f) => f, None => return u64::MAX };
                let mut entries = [crate::fs::fat32::FatDirEntry::new(); 64];
                let count = fat.list_dir_entries(ata(), dir_cluster, &mut entries);
                for j in 0..count {
                    if entries[j].name == name83 && entries[j].is_directory() {
                        CWD_CLUSTER = entries[j].cluster();
                        if CWD_PATH_LEN + 1 + pathlen <= CWD_PATH.len() {
                            CWD_PATH[CWD_PATH_LEN] = b'/';
                            for i in 0..pathlen { CWD_PATH[CWD_PATH_LEN + 1 + i] = path[i]; }
                            CWD_PATH_LEN += 1 + pathlen;
                        }
                        return 0;
                    }
                }
                u64::MAX
            }
        }
        SYS_MKDIR => {
            // a1 = path_ptr, a2 = len, a3 = mode
            if a1 == 0 || a2 == 0 { return u64::MAX; }
            let mut pathbuf = [0u8; 256];
            let pathlen = match unsafe { copy_user_str(a1, a2, &mut pathbuf) } {
                Some(n) => n, None => return u64::MAX,
            };
            let path = &pathbuf[..pathlen];
            unsafe {
                let resolved = match resolve_path(path) { Some(r) => r, None => return u64::MAX };
                let (dir_cluster, name83) = resolved;
                let fat = match fs().fat32.as_ref() { Some(f) => f, None => return u64::MAX };
                if fat.create_dir(ata(), dir_cluster, &name83).is_some() { 0 } else { u64::MAX }
            }
        }
        SYS_UNLINK => {
            if a1 == 0 || a2 == 0 { return u64::MAX; }
            let mut pathbuf = [0u8; 256];
            let pathlen = match unsafe { copy_user_str(a1, a2, &mut pathbuf) } {
                Some(n) => n, None => return u64::MAX,
            };
            let path = &pathbuf[..pathlen];
            unsafe {
                let resolved = match resolve_path(path) { Some(r) => r, None => return u64::MAX };
                let (dir_cluster, name83) = resolved;
                let fat = match fs().fat32.as_ref() { Some(f) => f, None => return u64::MAX };
                if fat.delete_file(ata(), dir_cluster, &name83) { 0 } else { u64::MAX }
            }
        }
        SYS_DEBUG => {
            if a2 != 0 && a3 > 0 && a3 <= 0x10000 {
                let ptr = a2 as *const u8;
                crate::serial::send_string("[USER] ");
                for i in 0..a3 as usize {
                    unsafe { crate::serial::send_byte(*ptr.add(i)); }
                }
                crate::serial::send_string("\n");
                a3
            } else { 0 }
        }
        SYS_EXIT => {
            crate::serial::send_string("\n[SYSCALL] EXIT, code=");
            print_u64(a1);
            crate::serial::send_string("\n");
            unsafe {
                asm!(
                    "mov rsp, {rsp}",
                     "jmp {rip}",
                     rsp = in(reg) KERNEL_RSP,
                     rip = in(reg) crate::kernel_main_loop as u64,
                     options(noreturn),
                );
            }
        }
        _ => {
            crate::serial::send_string("[SYSCALL] unknown num=");
            print_u64(num);
            crate::serial::send_string("\n");
            0
        }
    }
}

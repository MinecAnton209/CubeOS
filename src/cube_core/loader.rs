use crate::ata::AtaDrive;
use crate::fs::FSManager;
use crate::vmm::{AddressSpace, USER_LIMIT};
use crate::sched::{self, Qos};

const MAX_PROGRAM_SIZE: usize = 65536;

#[repr(C, align(4096))]
struct ProgramBuf([u8; MAX_PROGRAM_SIZE]);

#[link_section = ".program_buf"]
static mut PROGRAM_BUF: ProgramBuf = ProgramBuf([0; MAX_PROGRAM_SIZE]);

#[repr(C, align(16))]
struct KernelStack([u8; 16384]);

#[link_section = ".kernel_stack"]
static mut KERNEL_STACK: KernelStack = KernelStack([0; 16384]);

const USER_STACK_TOP: u64 = USER_LIMIT - 0x10000;   // 0x7FFF_FFFF_0000
const USER_STACK_PAGES: u64 = 4;

pub fn exec_user(
    ata: &AtaDrive,
    fs: &FSManager,
    path: &str,
    qos: Qos,
) -> Option<u64> {
    unsafe {
        let size = fs.read_file(ata, path, &mut PROGRAM_BUF.0)?;
        if size < 4 || PROGRAM_BUF.0[0..4] != *b"\x7fELF" {
            crate::serial::send_string("[LOADER] Invalid ELF magic\n");
            return None;
        }

        // TSS.rsp0 — kernel stack для IRQ/exception из user
        let ks_top = (&raw const KERNEL_STACK as *const u8 as u64) + 16384;
        crate::tss::init_tss(ks_top);
        crate::serial::send_string("[LOADER] TSS.rsp0 set\n");

        let mut aspace = AddressSpace::new()?;
        crate::serial::send_string("[LOADER] AddressSpace created\n");

        let program = crate::elf::load_elf(&PROGRAM_BUF.0[..size], &mut aspace)?;
        crate::serial::send_string("[LOADER] ELF mapped at 0x");
        let mut hb = [0u8; 16];
        for i in 0..16 {
            let n = ((program.entry_point >> ((15 - i) * 4)) & 0xF) as u8;
            hb[i] = if n < 10 { b'0' + n } else { b'A' + n - 10 };
        }
        crate::serial::send_string(core::str::from_utf8(&hb).unwrap_or("?"));
        crate::serial::send_string("\n");

        for i in 0..USER_STACK_PAGES {
            let v = USER_STACK_TOP - (i + 1) * 4096;
            if aspace.map_user_zeroed(v, true).is_none() {
                crate::serial::send_string("[LOADER] Failed to map user stack\n");
                return None;
            }
        }
        crate::serial::send_string("[LOADER] User stack mapped\n");

        let kernel_stack = crate::memory::alloc_page()?;
        core::ptr::write_bytes(kernel_stack as *mut u8, 0, 4096);

        let name = path.rsplit('/').next().unwrap_or("user");
        let pml4 = aspace.pml4_phys();

        sched::spawn_user(
            pml4,
            program.entry_point,
            USER_STACK_TOP - 16,
            kernel_stack,
            qos,
            name.as_bytes(),
        )
    }
}

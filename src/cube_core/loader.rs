use crate::ata::AtaDrive;
use crate::fs::FSManager;
use crate::elf::{self, Program, Segment};
use crate::mm; // Предполагаем, что у вас есть модуль управления памятью для отображения страниц

const MAX_PROGRAM_SIZE: usize = 524288; // Увеличено до 512 КБ для больших шеллов
const USER_STACK_SIZE: usize = 16384;
const USER_STACK_ADDR: u64 = 0x7FFF0000;
const ELF_VADDR_BASE: u64 = 0x1000000; // Ожидаемый базовый адрес линковки

#[repr(C, align(4096))]
struct ProgramBuf([u8; MAX_PROGRAM_SIZE]);

#[link_section = ".program_buf"]
static mut PROGRAM_BUF: ProgramBuf = ProgramBuf([0; MAX_PROGRAM_SIZE]);

#[repr(C, align(16))]
struct KernelStack([u8; 16384]);

#[link_section = ".kernel_stack"]
static mut KERNEL_STACK: KernelStack = KernelStack([0; 16384]);

// Функция для копирования в физическую память (если используется identity mapping)
// Если у вас полноценный VMM, используйте маппер из mm:: вместо прямого копирования
unsafe fn copy_to_phys(dst: u64, src: *const u8, len: usize) {
    core::ptr::copy_nonoverlapping(src, dst as *mut u8, len);
}

// Passed to the program in rdi; the layout must match KernelApi in the userland program.
#[repr(C)]
pub struct KernelApi {
    pub putc: extern "C" fn(u8),
    pub clear: extern "C" fn(),
}

static KERNEL_API: KernelApi = KernelApi {
    putc: crate::vesa_log::console_putc,
    clear: crate::vesa_log::console_clear,
};

pub fn exec(ata: &AtaDrive, fs: &FSManager, path: &str) -> bool {
    crate::serial::send_string("[LOADER] Loading: ");
    crate::serial::send_string(path);
    crate::serial::send_string("\n");

    unsafe {
        // 1. Читаем ВЕСЬ файл в буфер
        let file_size = match fs.read_file(ata, path, &mut PROGRAM_BUF.0) {
            Some(s) => s,
            None => {
                crate::serial::send_string("[LOADER] ERROR: File not found!\n");
                return false;
            }
        };

        if file_size > MAX_PROGRAM_SIZE {
            crate::serial::send_string("[LOADER] ERROR: File too large!\n");
            return false;
        }

        if file_size < 4 || PROGRAM_BUF.0[0] != 0x7F || PROGRAM_BUF.0[1] != b'E'
            || PROGRAM_BUF.0[2] != b'L' || PROGRAM_BUF.0[3] != b'F' {
                crate::serial::send_string("[LOADER] ERROR: Invalid ELF!\n");
                return false;
            }

            // 2. Парсим ELF (ваш текущий код elf::load_elf)
            let program = match elf::load_elf(&PROGRAM_BUF.0[..file_size]) {
                Some(p) => p,
                None => {
                    crate::serial::send_string("[LOADER] ERROR: ELF parse failed\n");
                    return false;
                }
            };

            // 3. КОПИРУЕМ СЕГМЕНТЫ В ПАМЯТЬ ПО АДРЕСАМ ELF
            // Это критический шаг, которого не хватало
            for seg in program.segments {
                if seg.file_size == 0 {
                    continue;
                }

        crate::vesa_log::console_reset();
        let entry: extern "C" fn(*const KernelApi) = core::mem::transmute(program.entry_point);
        entry(&KERNEL_API);
    }
}

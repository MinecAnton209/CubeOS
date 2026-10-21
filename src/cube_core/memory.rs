use crate::multiboot::MemoryMapEntry;

const MAX_PAGES: usize = 1024 * 1024;
const PAGE_SIZE: usize = 4096;

static mut BITMAP: [u64; MAX_PAGES / 64] = [0; MAX_PAGES / 64];
static mut TOTAL_PAGES: usize = 0;
static mut FREE_PAGES: usize = 0;

pub fn init(memory_map: &[MemoryMapEntry]) {
    unsafe {
        let mut _total_bytes: u64 = 0;
        let mut free_bytes: u64 = 0;

        for entry in memory_map {
            _total_bytes += entry.length;
            if entry.typ == 1 {
                let start_page = (entry.base_addr / PAGE_SIZE as u64) as usize;
                let end_page = ((entry.base_addr + entry.length) / PAGE_SIZE as u64) as usize;

                for page in start_page..end_page.min(MAX_PAGES) {
                    let idx = page / 64;
                    let bit = page % 64;
                    BITMAP[idx] |= 1 << bit;
                    free_bytes += PAGE_SIZE as u64;
                }
            }
        }

        TOTAL_PAGES = MAX_PAGES;
        FREE_PAGES = (free_bytes / PAGE_SIZE as u64) as usize;

        for page in 0..512 {
            let idx = page / 64;
            let bit = page % 64;
            if BITMAP[idx] & (1 << bit) != 0 {
                BITMAP[idx] &= !(1 << bit);
                FREE_PAGES -= 1;
            }
        }
    }
}

pub fn total_memory_mb() -> u64 {
    unsafe { (TOTAL_PAGES * PAGE_SIZE / 1024 / 1024) as u64 }
}

pub fn free_memory_mb() -> u64 {
    unsafe { (FREE_PAGES * PAGE_SIZE / 1024 / 1024) as u64 }
}

pub fn free_pages() -> usize {
    unsafe { FREE_PAGES }
}

pub fn alloc_page() -> Option<u64> {
    unsafe {
        for i in 0..(MAX_PAGES / 64) {
            if BITMAP[i] != 0 {
                let bit = BITMAP[i].trailing_zeros() as usize;
                let page = i * 64 + bit;
                BITMAP[i] &= !(1 << bit);
                FREE_PAGES -= 1;
                return Some((page * PAGE_SIZE) as u64);
            }
        }
        None
    }
}

pub fn free_page(addr: u64) {
    let page = (addr / PAGE_SIZE as u64) as usize;
    if page >= MAX_PAGES { return; }
    unsafe {
        let idx = page / 64;
        let bit = page % 64;
        if BITMAP[idx] & (1 << bit) == 0 {
            BITMAP[idx] |= 1 << bit;
            FREE_PAGES += 1;
        }
    }
}

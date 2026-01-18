use crate::mm::layout::{align_up, PAGE_SIZE};

static mut BOOT_START: u64 = 0;
static mut BOOT_CURRENT: u64 = 0;
static mut BOOT_END: u64 = 0;

pub fn init(start: u64, end: u64) {
    // Initialize a simple bump allocator used before the heap is ready.
    unsafe {
        let aligned = align_up(start, PAGE_SIZE as u64);
        BOOT_START = aligned;
        BOOT_CURRENT = aligned;
        BOOT_END = end;
    }
}

pub fn alloc(size: usize, align: usize) -> Option<u64> {
    // Allocate a physically contiguous chunk from the boot region.
    unsafe {
        let align = align.max(1) as u64;
        let current = align_up(BOOT_CURRENT, align);
        let next = current.saturating_add(size as u64);
        if current == 0 || next > BOOT_END {
            return None;
        }
        BOOT_CURRENT = next;
        Some(current)
    }
}

pub fn alloc_pages(pages: usize) -> Option<u64> {
    // Convenience wrapper for page-sized allocations.
    alloc(pages * PAGE_SIZE, PAGE_SIZE)
}

pub fn used_range() -> (u64, u64) {
    // Return the range consumed by boot allocations (for reserving frames).
    unsafe { (BOOT_START, BOOT_CURRENT) }
}

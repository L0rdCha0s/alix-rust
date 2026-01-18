use crate::kernel::user;
use crate::mm::frame;
use crate::mm::layout::{phys_to_virt, PAGE_SIZE};
use core::alloc::{GlobalAlloc, Layout};
use linked_list_allocator::LockedHeap;

pub const HEAP_SIZE: usize = 8 * 1024 * 1024;

pub struct GlobalAllocator {
    kernel: LockedHeap,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            kernel: LockedHeap::empty(),
        }
    }

    pub fn init_kernel_heap(&self) {
        // Back the kernel heap with a contiguous run of frames.
        let pages = HEAP_SIZE / PAGE_SIZE;
        let paddr = match frame::alloc_contiguous(pages) {
            Some(addr) => addr,
            None => return,
        };
        let vaddr = phys_to_virt(paddr) as *mut u8;
        unsafe {
            self.kernel.lock().init(vaddr, HEAP_SIZE);
        }
    }
}

#[global_allocator]
static GLOBAL_ALLOC: GlobalAllocator = GlobalAllocator::new();

pub fn init() {
    // Initialize the global allocator after paging is live.
    GLOBAL_ALLOC.init_kernel_heap();
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Use syscall-backed allocator for EL0, kernel heap for EL1.
        if current_el() == 0 {
            let ptr = user::alloc(layout.size(), layout.align());
            if ptr == 0 || ptr == u64::MAX {
                core::ptr::null_mut()
            } else {
                ptr as *mut u8
            }
        } else {
            self.kernel.alloc(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Route frees to the correct allocator based on exception level.
        if current_el() == 0 {
            let _ = user::free(ptr as u64, layout.size(), layout.align());
        } else {
            self.kernel.dealloc(ptr, layout)
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Route reallocs to the correct allocator based on exception level.
        if current_el() == 0 {
            let new_ptr = user::realloc(ptr as u64, layout.size(), new_size, layout.align());
            if new_ptr == 0 || new_ptr == u64::MAX {
                core::ptr::null_mut()
            } else {
                new_ptr as *mut u8
            }
        } else {
            self.kernel.realloc(ptr, layout, new_size)
        }
    }
}

#[alloc_error_handler]
fn oom(_: Layout) -> ! {
    // OOM is fatal in the kernel; park the CPU.
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
    }
}

#[inline(always)]
fn current_el() -> u8 {
    // Read CurrentEL to detect whether we're running in EL0 or EL1.
    let el: u64;
    unsafe {
        core::arch::asm!("mrs {0}, CurrentEL", out(reg) el, options(nomem, nostack, preserves_flags));
    }
    ((el >> 2) & 0x3) as u8
}

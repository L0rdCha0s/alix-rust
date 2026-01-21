use crate::mm::bootalloc;
use crate::mm::layout::{align_up, PAGE_SIZE};
use crate::mm::region::{NormalizedMap, RegionKind};
use crate::util::sync::SpinLock;

#[cfg(feature = "rpi5")]
const RP1_UART_FALLBACK: usize = 0x1c00_0300_00;

#[cfg(feature = "rpi5")]
#[inline(always)]
unsafe fn early_uart_putc(b: u8) {
    (RP1_UART_FALLBACK as *mut u32).write_volatile(b as u32);
}

#[cfg(feature = "rpi5")]
fn early_uart_print(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            unsafe { early_uart_putc(b'\r'); }
        }
        unsafe { early_uart_putc(b); }
    }
}

pub struct FrameAllocator {
    frame_count: usize,
    bitmap: &'static mut [u64],
}

static FRAME_ALLOC: SpinLock<Option<FrameAllocator>> = SpinLock::new(None);

pub fn init(map: &NormalizedMap) {
    // Build a bitmap allocator covering all physical frames in the system.
    let mut max_end = 0u64;
    for region in map.regions() {
        if region.kind == RegionKind::UsableRam && region.end > max_end {
            max_end = region.end;
        }
    }
    if max_end == 0 {
        return;
    }
    let frame_count = (align_up(max_end, PAGE_SIZE as u64) / PAGE_SIZE as u64) as usize;
    let bits = frame_count;
    let words = (bits + 63) / 64;
    let bytes = words * 8;
    #[cfg(feature = "rpi5")]
    early_uart_print("F0\n");
    let bitmap_paddr = match bootalloc::alloc(bytes, 8) {
        Some(addr) => addr,
        None => return,
    };
    #[cfg(feature = "rpi5")]
    early_uart_print("F1\n");
    let bitmap_ptr = bitmap_paddr as *mut u64;
    let bitmap = unsafe { core::slice::from_raw_parts_mut(bitmap_ptr, words) };
    for word in bitmap.iter_mut() {
        *word = u64::MAX;
    }
    #[cfg(feature = "rpi5")]
    early_uart_print("F2\n");
    let mut alloc = FrameAllocator {
        frame_count,
        bitmap,
    };
    for region in map.regions() {
        if region.kind != RegionKind::UsableRam {
            continue;
        }
        // Mark usable RAM frames as free.
        alloc.mark_free(region.start, region.end);
    }
    #[cfg(feature = "rpi5")]
    early_uart_print("F3\n");
    // Reserve frames used by the boot allocator itself.
    let (boot_start, boot_end) = bootalloc::used_range();
    alloc.mark_used(boot_start, boot_end);
    #[cfg(feature = "rpi5")]
    early_uart_print("F4\n");

    let mut guard = FRAME_ALLOC.lock();
    *guard = Some(alloc);
    #[cfg(feature = "rpi5")]
    early_uart_print("F5\n");
}

pub fn alloc_frame() -> Option<u64> {
    // Allocate a single 4 KiB frame and return its physical address.
    let mut guard = FRAME_ALLOC.lock();
    let alloc = guard.as_mut()?;
    alloc.alloc_frame()
}

pub fn alloc_contiguous(pages: usize) -> Option<u64> {
    // Allocate a contiguous run of frames (useful for DMA).
    let mut guard = FRAME_ALLOC.lock();
    let alloc = guard.as_mut()?;
    alloc.alloc_contiguous(pages)
}

pub fn free_frame(paddr: u64) {
    // Return a frame to the allocator.
    let mut guard = FRAME_ALLOC.lock();
    if let Some(alloc) = guard.as_mut() {
        alloc.free_frame(paddr);
    }
}

impl FrameAllocator {
    fn mark_free(&mut self, start: u64, end: u64) {
        // Clear bits for frames in the specified range.
        let mut idx = (start / PAGE_SIZE as u64) as usize;
        let end_idx = (end / PAGE_SIZE as u64) as usize;
        while idx < end_idx {
            self.clear_bit(idx);
            idx += 1;
        }
    }

    fn mark_used(&mut self, start: u64, end: u64) {
        // Set bits for frames in the specified range.
        let mut idx = (start / PAGE_SIZE as u64) as usize;
        let end_idx = (align_up(end, PAGE_SIZE as u64) / PAGE_SIZE as u64) as usize;
        while idx < end_idx {
            self.set_bit(idx);
            idx += 1;
        }
    }

    fn alloc_frame(&mut self) -> Option<u64> {
        // Find the first free bit and claim it.
        let mut idx = 0usize;
        while idx < self.frame_count {
            if !self.test_bit(idx) {
                self.set_bit(idx);
                return Some((idx as u64) * PAGE_SIZE as u64);
            }
            idx += 1;
        }
        None
    }

    fn alloc_contiguous(&mut self, pages: usize) -> Option<u64> {
        // Naive contiguous search across the bitmap.
        if pages == 0 {
            return None;
        }
        let mut idx = 0usize;
        while idx + pages <= self.frame_count {
            let mut ok = true;
            let mut check = idx;
            while check < idx + pages {
                if self.test_bit(check) {
                    ok = false;
                    break;
                }
                check += 1;
            }
            if ok {
                for bit in idx..idx + pages {
                    self.set_bit(bit);
                }
                return Some((idx as u64) * PAGE_SIZE as u64);
            }
            idx += 1;
        }
        None
    }

    fn free_frame(&mut self, paddr: u64) {
        // Clear the bit corresponding to this frame.
        let idx = (paddr / PAGE_SIZE as u64) as usize;
        if idx < self.frame_count {
            self.clear_bit(idx);
        }
    }

    #[inline(always)]
    fn test_bit(&self, idx: usize) -> bool {
        let word = idx / 64;
        let bit = idx % 64;
        (self.bitmap[word] & (1u64 << bit)) != 0
    }

    #[inline(always)]
    fn set_bit(&mut self, idx: usize) {
        let word = idx / 64;
        let bit = idx % 64;
        self.bitmap[word] |= 1u64 << bit;
    }

    #[inline(always)]
    fn clear_bit(&mut self, idx: usize) {
        let word = idx / 64;
        let bit = idx % 64;
        self.bitmap[word] &= !(1u64 << bit);
    }
}

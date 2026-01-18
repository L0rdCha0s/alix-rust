#![allow(dead_code)]

pub mod bootalloc;
pub mod dtb;
pub mod frame;
pub mod heap;
pub mod layout;
pub mod paging;
pub mod region;

use crate::drivers::uart;
use crate::mm::layout::{align_up, PAGE_SIZE};
use crate::mm::region::{MemoryMap, RegionKind};
use crate::platform::board;

extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
    static __stack_start: u8;
    static __stack_end: u8;
}

pub fn init(dtb_pa: u64) {
    // Build a raw memory map from DTB + known regions, then normalize it.
    let mut map = MemoryMap::new();
    let dtb_info = dtb::parse(dtb_pa, &mut map);

    let kernel_start = unsafe { &__kernel_start as *const u8 as u64 };
    let kernel_end = unsafe { &__kernel_end as *const u8 as u64 };
    let stack_start = unsafe { &__stack_start as *const u8 as u64 };
    let stack_end = unsafe { &__stack_end as *const u8 as u64 };

    map.add_region(kernel_start, kernel_end.saturating_sub(kernel_start), RegionKind::KernelImage);
    map.add_region(stack_start, stack_end.saturating_sub(stack_start), RegionKind::BootStack);

    if let Some(info) = dtb_info {
        map.add_region(dtb_pa, info.total_size as u64, RegionKind::BootInfo);
    } else {
        uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(uart, "DTB parse failed or missing (dtb_pa={:#x})", dtb_pa);
        });
        #[cfg(feature = "qemu")]
        {
            // QEMU fallback when DTB is not available.
            map.add_region(0, board::QEMU_RAM_SIZE, RegionKind::UsableRam);
        }
    }

    #[cfg(feature = "qemu")]
    {
        map.add_region(
            board::PERIPHERAL_BASE as u64,
            0x0100_0000,
            RegionKind::Mmio,
        );
    }
    #[cfg(feature = "rpi5")]
    {
        map.add_region(board::SOC_BASE as u64, 0x0100_0000, RegionKind::Mmio);
    }

    let normalized = map.normalize();

    // Log the normalized map before allocating.
    log_map(&normalized);
    log_summary(&normalized);

    let boot_start = align_up(kernel_end, PAGE_SIZE as u64);
    let mut boot_end = 0u64;
    for region in normalized.usable_regions() {
        if region.start <= boot_start && boot_start < region.end {
            boot_end = region.end;
            break;
        }
    }
    if boot_end == 0 {
        for region in normalized.usable_regions() {
            boot_end = region.end;
            break;
        }
    }
    if boot_end == 0 {
        return;
    }

    // Boot allocator is used for early allocations before the heap is ready.
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: bootalloc init start={:#x} end={:#x}", boot_start, boot_end);
    });
    bootalloc::init(boot_start, boot_end);
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: bootalloc ready");
    });
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: frame allocator init");
    });
    frame::init(&normalized);
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: frame allocator ready");
    });
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: paging init");
    });
    // Build identity-mapped page tables and enable the MMU.
    paging::init(&normalized);
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: paging ready");
    });
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: heap init");
    });
    // Initialize the kernel heap allocator.
    heap::init();
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: heap ready");
    });
}

fn log_map(map: &crate::mm::region::NormalizedMap) {
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "Memory map:");
        for region in map.regions() {
            let kind = match region.kind {
                RegionKind::UsableRam => "usable",
                RegionKind::Reserved => "reserved",
                RegionKind::Mmio => "mmio",
                RegionKind::KernelImage => "kernel",
                RegionKind::BootStack => "stack",
                RegionKind::BootInfo => "bootinfo",
            };
            let _ = writeln!(
                uart,
                "  {:#010x}-{:#010x} {}",
                region.start,
                region.end,
                kind
            );
        }
    });
}

fn log_summary(map: &crate::mm::region::NormalizedMap) {
    // Aggregate totals by region type for a quick sanity check.
    let mut usable = 0u64;
    let mut reserved = 0u64;
    let mut mmio = 0u64;
    let mut kernel = 0u64;
    let mut bootinfo = 0u64;
    let mut stack = 0u64;
    for region in map.regions() {
        let size = region.end.saturating_sub(region.start);
        match region.kind {
            RegionKind::UsableRam => usable += size,
            RegionKind::Reserved => reserved += size,
            RegionKind::Mmio => mmio += size,
            RegionKind::KernelImage => kernel += size,
            RegionKind::BootInfo => bootinfo += size,
            RegionKind::BootStack => stack += size,
        }
    }
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(
            uart,
            "Memory summary: usable={} MiB reserved={} MiB mmio={} MiB kernel={} KiB stack={} KiB bootinfo={} KiB",
            usable / (1024 * 1024),
            reserved / (1024 * 1024),
            mmio / (1024 * 1024),
            kernel / 1024,
            stack / 1024,
            bootinfo / 1024
        );
    });
}

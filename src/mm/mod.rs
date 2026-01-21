#![allow(dead_code)]

pub mod bootalloc;
pub mod dtb;
pub mod frame;
pub mod heap;
pub mod layout;
pub mod paging;
pub mod region;

use crate::drivers::uart;
use crate::arch::aarch64::mmu;
use crate::mm::layout::{align_down, align_up, KERNEL_PHYS_BASE, PAGE_SIZE};
use crate::mm::region::{MemoryMap, RegionKind};
use crate::platform::board;

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

#[cfg(feature = "rpi5")]
#[inline(always)]
fn early_uart_delay() {
    // Coarse delay to avoid TX FIFO overflow when we don't poll FR.
    for _ in 0..200_000 {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) }
    }
}

#[cfg(feature = "rpi5")]
fn early_uart_print_slow(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            unsafe { early_uart_putc(b'\r'); }
            early_uart_delay();
        }
        unsafe { early_uart_putc(b); }
        early_uart_delay();
    }
}

#[cfg(feature = "rpi5")]
fn early_uart_hex_u64(value: u64) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    early_uart_print_slow("0x");
    let mut shift = 60u32;
    while shift <= 60 {
        let nibble = ((value >> shift) & 0xF) as usize;
        unsafe { early_uart_putc(HEX[nibble]); }
        early_uart_delay();
        if shift == 0 {
            break;
        }
        shift -= 4;
    }
}

#[cfg(feature = "rpi5")]
fn early_uart_kind(kind: RegionKind) {
    match kind {
        RegionKind::UsableRam => early_uart_print("usable"),
        RegionKind::Reserved => early_uart_print("reserved"),
        RegionKind::Mmio => early_uart_print("mmio"),
        RegionKind::KernelImage => early_uart_print("kernel"),
        RegionKind::BootStack => early_uart_print("stack"),
        RegionKind::BootInfo => early_uart_print("bootinfo"),
    }
}

#[cfg(feature = "rpi5")]
fn log_map_raw(map: &crate::mm::region::NormalizedMap) {
    early_uart_print_slow("Memory map:\n");
    for region in map.regions() {
        early_uart_print_slow("  ");
        early_uart_hex_u64(region.start);
        early_uart_print_slow("-");
        early_uart_hex_u64(region.end);
        early_uart_print_slow(" ");
        early_uart_kind(region.kind);
        early_uart_print_slow("\n");
    }
}

#[cfg(feature = "rpi5")]
fn log_summary_raw(map: &crate::mm::region::NormalizedMap) {
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
    early_uart_print_slow("Memory summary: usable=");
    early_uart_hex_u64(usable);
    early_uart_print_slow(" reserved=");
    early_uart_hex_u64(reserved);
    early_uart_print_slow(" mmio=");
    early_uart_hex_u64(mmio);
    early_uart_print_slow(" kernel=");
    early_uart_hex_u64(kernel);
    early_uart_print_slow(" stack=");
    early_uart_hex_u64(stack);
    early_uart_print_slow(" bootinfo=");
    early_uart_hex_u64(bootinfo);
    early_uart_print_slow("\n");
}

#[repr(C)]
struct KernelPhysInfo {
    kernel_start: u64,
    kernel_end: u64,
    stack_start: u64,
    stack_end: u64,
    boot_end: u64,
}

extern "C" {
    static __kernel_phys_info: KernelPhysInfo;
}

pub fn init(dtb_pa: u64) {
    #[cfg(feature = "rpi5")]
    early_uart_print("M0\n");

    // Build a raw memory map from DTB + known regions, then normalize it.
    let mut map = MemoryMap::new();
    let dtb_info = dtb::parse(dtb_pa, &mut map);

    #[cfg(feature = "rpi5")]
    early_uart_print("M1\n");

    #[cfg(feature = "rpi5")]
    {
        if let Some(info) = dtb::find_uart(dtb_pa) {
            // Map the UART MMIO window and stash its base for init after paging.
            uart::set_base(info.addr as usize);
            uart::set_clock_hz(info.clock_hz);
            uart::set_reg_shift(info.reg_shift);
            uart::set_reg_io_width(info.reg_io_width);
            uart::set_skip_init(info.skip_init);
            let mmio_base = align_down(info.addr, 0x20_0000);
            paging::set_extra_mmio(mmio_base, 0x20_0000);
        }
    }

    #[cfg(feature = "rpi5")]
    early_uart_print("M2\n");

    let info = unsafe { &__kernel_phys_info };
    let kernel_start = info.kernel_start;
    let kernel_end = info.kernel_end;
    let stack_start = info.stack_start;
    let stack_end = info.stack_end;
    let boot_start = KERNEL_PHYS_BASE;
    let boot_end = info.boot_end;

    if boot_end > boot_start {
        map.add_region(boot_start, boot_end.saturating_sub(boot_start), RegionKind::KernelImage);
    }
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
            board::PERIPHERAL_SIZE as u64,
            RegionKind::Mmio,
        );
    }
    #[cfg(feature = "rpi5")]
    {
        map.add_region(
            board::SOC_BASE as u64,
            board::SOC_MMIO_SIZE as u64,
            RegionKind::Mmio,
        );
    }

    let normalized = map.normalize();

    #[cfg(feature = "rpi5")]
    early_uart_print("M3\n");

    // Log the normalized map before allocating.
    #[cfg(feature = "rpi5")]
    {
        log_map_raw(&normalized);
        log_summary_raw(&normalized);
    }
    #[cfg(feature = "qemu")]
    {
        log_map(&normalized);
        log_summary(&normalized);
    }

    #[cfg(feature = "rpi5")]
    early_uart_print("M4\n");

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

    #[cfg(feature = "rpi5")]
    early_uart_print("M5\n");

    #[cfg(feature = "rpi5")]
    {
        early_uart_print_slow("mm: enable caches\n");
        mmu::enable_caches();
        early_uart_print_slow("mm: caches on\n");
    }

    // Boot allocator is used for early allocations before the heap is ready.
    #[cfg(feature = "rpi5")]
    {
        early_uart_print_slow("mm: bootalloc init start=");
        early_uart_hex_u64(boot_start);
        early_uart_print_slow(" end=");
        early_uart_hex_u64(boot_end);
        early_uart_print_slow("\n");
    }
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: bootalloc init start={:#x} end={:#x}", boot_start, boot_end);
    });
    bootalloc::init(boot_start, boot_end);
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: bootalloc ready\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: bootalloc ready");
    });
    #[cfg(feature = "rpi5")]
    early_uart_print("M6\n");
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: frame allocator init\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: frame allocator init");
    });
    frame::init(&normalized);
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: frame allocator ready\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: frame allocator ready");
    });
    #[cfg(feature = "rpi5")]
    early_uart_print("M7\n");
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: paging init\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: paging init");
    });
    // Build identity-mapped page tables and enable the MMU.
    paging::init(&normalized);
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: paging ready\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: paging ready");
    });
    #[cfg(feature = "rpi5")]
    early_uart_print("M8\n");
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: heap init\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: heap init");
    });
    // Initialize the kernel heap allocator.
    heap::init();
    #[cfg(feature = "rpi5")]
    early_uart_print_slow("mm: heap ready\n");
    #[cfg(feature = "qemu")]
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "mm: heap ready");
    });
    #[cfg(feature = "rpi5")]
    early_uart_print("M9\n");
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

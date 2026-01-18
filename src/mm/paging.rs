#![allow(static_mut_refs)]

use crate::arch::aarch64::mmu;
use crate::mm::layout::{align_down, align_up};
use crate::mm::region::{NormalizedMap, RegionKind};
use crate::platform::board;

const L2_TABLES: usize = 64;
const L0_ENTRIES: usize = 512;
const L1_ENTRIES: usize = 512;
const L2_ENTRIES: usize = 512;

const BLOCK_SIZE: u64 = 0x20_0000; // 2 MiB

#[repr(align(4096))]
struct PageTable([u64; 512]);

impl PageTable {
    const fn new() -> Self {
        Self([0; 512])
    }

    fn zero(&mut self) {
        for entry in self.0.iter_mut() {
            *entry = 0;
        }
    }
}

static mut L0_TABLE: PageTable = PageTable::new();
static mut L1_TABLE: PageTable = PageTable::new();
static mut L2_POOL: [PageTable; L2_TABLES] = [const { PageTable::new() }; L2_TABLES];
static mut NEXT_L2: usize = 0;

const DESC_BLOCK: u64 = 0b01;
const DESC_TABLE: u64 = 0b11;
const AF_BIT: u64 = 1 << 10;
const UXN_BIT: u64 = 1 << 54;
const PXN_BIT: u64 = 1 << 53;

const ATTR_DEVICE: u64 = 0;
const ATTR_NORMAL: u64 = 1;

const AP_EL1_RW: u64 = 0b00;
const AP_EL0_RW: u64 = 0b01;

const SH_NONE: u64 = 0b00;
const SH_INNER: u64 = 0b11;

#[cfg(feature = "qemu")]
const AP_KERNEL: u64 = AP_EL1_RW;
#[cfg(not(feature = "qemu"))]
const AP_KERNEL: u64 = AP_EL0_RW;

pub fn init(map: &NormalizedMap) {
    unsafe {
        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(uart, "paging: build tables");
        });
        L0_TABLE.zero();
        L1_TABLE.zero();
        NEXT_L2 = 0;
        let l1_pa = &L1_TABLE as *const _ as u64;
        L0_TABLE.0[0] = table_desc(l1_pa);

        for region in map.regions() {
            if region.kind == RegionKind::Mmio {
                continue;
            }
            // Map all non-MMIO regions as normal memory.
            map_range(
                region.start,
                region.end,
                ATTR_NORMAL,
                AP_KERNEL,
                SH_INNER,
                false,
            );
        }

        // Map device MMIO ranges (UART, mailbox, etc).
        map_mmio();
        #[cfg(feature = "qemu")]
        {
            // Map the VC-reserved RAM window so framebuffer accesses do not fault.
            map_range(
                0x3c00_0000,
                0x4000_0000,
                ATTR_NORMAL,
                AP_EL1_RW,
                SH_INNER,
                true,
            );
        }

        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let root_pa = &L0_TABLE as *const _ as u64;
            let l0_pa = &L0_TABLE as *const _ as u64;
            let l1_pa = &L1_TABLE as *const _ as u64;
            let l2_pa = &L2_POOL[0] as *const _ as u64;
            let l0e0 = L0_TABLE.0[0];
            let l1e0 = L1_TABLE.0[0];
            let l2e0 = L2_POOL[0].0[0];
            let _ = writeln!(
                uart,
                "paging: enable mmu root={:#x} l0@{:#x} l1@{:#x} l2@{:#x} l0[0]={:#x} l1[0]={:#x} l2[0]={:#x}",
                root_pa, l0_pa, l1_pa, l2_pa, l0e0, l1e0, l2e0
            );
        });
        let root_pa = &L0_TABLE as *const _ as u64;
        mmu::enable_mmu(root_pa);
        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(uart, "paging: mmu enabled");
        });
    }
}

unsafe fn map_mmio() {
    #[cfg(feature = "qemu")]
    {
        let base = board::PERIPHERAL_BASE as u64;
        // Peripheral MMIO range (UART, GPIO, mailbox).
        map_range(
            base,
            base + 0x0100_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
        // Local interrupt controller window used by per-core timer setup.
        map_range(
            0x4000_0000,
            0x4020_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
    }
    #[cfg(feature = "rpi5")]
    {
        let base = board::SOC_BASE as u64;
        map_range(
            base,
            base + 0x0100_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
    }
}

unsafe fn map_range(start: u64, end: u64, attr: u64, ap: u64, sh: u64, xn: bool) {
    // Map a range using 2 MiB blocks (L2 blocks).
    if end <= start {
        return;
    }
    let mut addr = align_down(start, BLOCK_SIZE);
    let end = align_up(end, BLOCK_SIZE);
    while addr < end {
        map_block(addr, attr, ap, sh, xn);
        addr += BLOCK_SIZE;
    }
}

unsafe fn map_block(addr: u64, attr: u64, ap: u64, sh: u64, xn: bool) {
    // Allocate or reuse the L2 table for the corresponding L1 entry.
    let l1_idx = ((addr >> 30) & 0x1ff) as usize;
    let l2_idx = ((addr >> 21) & 0x1ff) as usize;
    let l2 = get_l2_table(l1_idx);
    let desc = block_desc(addr, attr, ap, sh, xn);
    l2.0[l2_idx] = desc;
}

unsafe fn get_l2_table(l1_idx: usize) -> &'static mut PageTable {
    // Ensure a valid L2 table exists for this L1 slot.
    if L1_TABLE.0[l1_idx] & 0b11 == DESC_TABLE {
        let pa = L1_TABLE.0[l1_idx] & 0x0000_FFFF_FFFF_F000;
        return &mut *(pa as *mut PageTable);
    }
    let idx = NEXT_L2;
    if idx >= L2_TABLES {
        loop {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
    NEXT_L2 += 1;
    let table = &mut L2_POOL[idx];
    table.zero();
    let pa = table as *mut _ as u64;
    L1_TABLE.0[l1_idx] = table_desc(pa);
    table
}

fn table_desc(pa: u64) -> u64 {
    (pa & 0x0000_FFFF_FFFF_F000) | DESC_TABLE
}

fn block_desc(pa: u64, attr: u64, ap: u64, sh: u64, xn: bool) -> u64 {
    let mut desc = DESC_BLOCK;
    desc |= (attr & 0x7) << 2;
    desc |= (ap & 0x3) << 6;
    desc |= (sh & 0x3) << 8;
    desc |= AF_BIT;
    desc |= pa & 0x0000_FFFF_FFE0_0000;
    if xn {
        desc |= UXN_BIT | PXN_BIT;
    }
    desc
}

#![allow(static_mut_refs)]

use crate::arch::aarch64::mmu;
use crate::mm::layout::{align_down, align_up, phys_to_virt, virt_to_phys, KERNEL_VIRT_BASE};
use crate::mm::region::{NormalizedMap, RegionKind};
use crate::platform::board;

const L2_TABLES: usize = 64;

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

// Kernel (TTBR1) tables
static mut K_L0: PageTable = PageTable::new();
static mut K_L1: PageTable = PageTable::new();
static mut K_L2_POOL: [PageTable; L2_TABLES] = [const { PageTable::new() }; L2_TABLES];
static mut K_NEXT_L2: usize = 0;

// User (TTBR0) tables
static mut U_L0: PageTable = PageTable::new();
static mut U_L1: PageTable = PageTable::new();
static mut U_L2_POOL: [PageTable; L2_TABLES] = [const { PageTable::new() }; L2_TABLES];
static mut U_NEXT_L2: usize = 0;

static mut KERNEL_ROOT_PA: u64 = 0;
static mut USER_ROOT_PA: u64 = 0;

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

pub fn init(map: &NormalizedMap) {
    unsafe {
        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(uart, "paging: build tables");
        });

        // Initialize kernel tables (TTBR1) for higher-half mapping.
        K_L0.zero();
        K_L1.zero();
        K_NEXT_L2 = 0;
        let k_l1_pa = virt_to_phys(&K_L1 as *const _ as usize);
        K_L0.0[0] = table_desc(k_l1_pa);
        K_L0.0[511] = table_desc(k_l1_pa);

        // Initialize user tables (TTBR0) for identity mapping.
        U_L0.zero();
        U_L1.zero();
        U_NEXT_L2 = 0;
        let u_l1_pa = virt_to_phys(&U_L1 as *const _ as usize);
        U_L0.0[0] = table_desc(u_l1_pa);

        for region in map.regions() {
            if region.kind == RegionKind::Mmio {
                continue;
            }
            let size = region.end.saturating_sub(region.start);
            // Kernel higher-half mapping of RAM.
            map_range_with(
                &mut K_L1,
                &mut K_L2_POOL,
                &mut K_NEXT_L2,
                KERNEL_VIRT_BASE + region.start,
                region.start,
                size,
                ATTR_NORMAL,
                AP_EL1_RW,
                SH_INNER,
                false,
            );
            // User identity mapping of RAM.
            map_range_with(
                &mut U_L1,
                &mut U_L2_POOL,
                &mut U_NEXT_L2,
                region.start,
                region.start,
                size,
                ATTR_NORMAL,
                AP_EL0_RW,
                SH_INNER,
                false,
            );
        }

        map_mmio();

        KERNEL_ROOT_PA = virt_to_phys(&K_L0 as *const _ as usize);
        USER_ROOT_PA = virt_to_phys(&U_L0 as *const _ as usize);

        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(
                uart,
                "paging: enable mmu ttbr0={:#x} ttbr1={:#x}",
                USER_ROOT_PA, KERNEL_ROOT_PA
            );
        });
        mmu::enable_mmu(USER_ROOT_PA, KERNEL_ROOT_PA);
        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(uart, "paging: mmu enabled");
        });
    }
}

pub fn user_root_pa() -> u64 {
    unsafe { USER_ROOT_PA }
}

pub fn kernel_root_pa() -> u64 {
    unsafe { KERNEL_ROOT_PA }
}

unsafe fn map_mmio() {
    // Map MMIO into both TTBR0 (identity) and TTBR1 (higher-half).
    #[cfg(feature = "qemu")]
    {
        let base = board::PERIPHERAL_BASE as u64;
        map_range_with(
            &mut U_L1,
            &mut U_L2_POOL,
            &mut U_NEXT_L2,
            base,
            base,
            0x0100_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
        map_range_with(
            &mut K_L1,
            &mut K_L2_POOL,
            &mut K_NEXT_L2,
            KERNEL_VIRT_BASE + base,
            base,
            0x0100_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );

        map_range_with(
            &mut U_L1,
            &mut U_L2_POOL,
            &mut U_NEXT_L2,
            0x4000_0000,
            0x4000_0000,
            0x0020_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
        map_range_with(
            &mut K_L1,
            &mut K_L2_POOL,
            &mut K_NEXT_L2,
            KERNEL_VIRT_BASE + 0x4000_0000,
            0x4000_0000,
            0x0020_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );

        // VC reserved RAM window used by framebuffer.
        map_range_with(
            &mut U_L1,
            &mut U_L2_POOL,
            &mut U_NEXT_L2,
            0x3c00_0000,
            0x3c00_0000,
            0x0400_0000,
            ATTR_NORMAL,
            AP_EL1_RW,
            SH_INNER,
            true,
        );
        map_range_with(
            &mut K_L1,
            &mut K_L2_POOL,
            &mut K_NEXT_L2,
            KERNEL_VIRT_BASE + 0x3c00_0000,
            0x3c00_0000,
            0x0400_0000,
            ATTR_NORMAL,
            AP_EL1_RW,
            SH_INNER,
            true,
        );
    }
    #[cfg(feature = "rpi5")]
    {
        let base = board::SOC_BASE as u64;
        map_range_with(
            &mut U_L1,
            &mut U_L2_POOL,
            &mut U_NEXT_L2,
            base,
            base,
            0x0100_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
        map_range_with(
            &mut K_L1,
            &mut K_L2_POOL,
            &mut K_NEXT_L2,
            KERNEL_VIRT_BASE + base,
            base,
            0x0100_0000,
            ATTR_DEVICE,
            AP_EL1_RW,
            SH_NONE,
            true,
        );
    }
}

unsafe fn map_range_with(
    l1: &mut PageTable,
    l2_pool: &mut [PageTable; L2_TABLES],
    next_l2: &mut usize,
    vstart: u64,
    pstart: u64,
    size: u64,
    attr: u64,
    ap: u64,
    sh: u64,
    xn: bool,
) {
    if size == 0 {
        return;
    }
    let mut vaddr = align_down(vstart, BLOCK_SIZE);
    let mut paddr = align_down(pstart, BLOCK_SIZE);
    let end = align_up(vstart + size, BLOCK_SIZE);
    while vaddr < end {
        map_block_with(l1, l2_pool, next_l2, vaddr, paddr, attr, ap, sh, xn);
        vaddr += BLOCK_SIZE;
        paddr += BLOCK_SIZE;
    }
}

unsafe fn map_block_with(
    l1: &mut PageTable,
    l2_pool: &mut [PageTable; L2_TABLES],
    next_l2: &mut usize,
    vaddr: u64,
    paddr: u64,
    attr: u64,
    ap: u64,
    sh: u64,
    xn: bool,
) {
    let l1_idx = ((vaddr >> 30) & 0x1ff) as usize;
    let l2_idx = ((vaddr >> 21) & 0x1ff) as usize;
    let l2 = get_l2_table_with(l1, l2_pool, next_l2, l1_idx);
    let desc = block_desc(paddr, attr, ap, sh, xn);
    l2.0[l2_idx] = desc;
}

unsafe fn get_l2_table_with<'a>(
    l1: &'a mut PageTable,
    l2_pool: &'a mut [PageTable; L2_TABLES],
    next_l2: &'a mut usize,
    l1_idx: usize,
) -> &'a mut PageTable {
    if l1.0[l1_idx] & 0b11 == DESC_TABLE {
        let pa = l1.0[l1_idx] & 0x0000_FFFF_FFFF_F000;
        let va = phys_to_virt(pa);
        return &mut *(va as *mut PageTable);
    }
    let idx = *next_l2;
    if idx >= L2_TABLES {
        loop {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
    *next_l2 += 1;
    let table = &mut l2_pool[idx];
    table.zero();
    let pa = virt_to_phys(table as *const _ as usize);
    l1.0[l1_idx] = table_desc(pa);
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

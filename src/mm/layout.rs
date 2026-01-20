pub const PAGE_SIZE: usize = 4096;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

pub const KERNEL_PHYS_BASE: u64 = 0x80000;
// Use a canonical high-half VA (48-bit) with ample room for physmap.
pub const KERNEL_VIRT_BASE: u64 = 0xFFFF_8000_0000_0000;
pub const PHYS_MAP_BASE: u64 = KERNEL_VIRT_BASE;

#[inline(always)]
pub const fn align_up(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}

#[inline(always)]
pub const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

#[inline(always)]
pub const fn phys_to_virt(paddr: u64) -> usize {
    (paddr + PHYS_MAP_BASE) as usize
}

#[inline(always)]
pub const fn virt_to_phys(vaddr: usize) -> u64 {
    (vaddr as u64).wrapping_sub(PHYS_MAP_BASE)
}

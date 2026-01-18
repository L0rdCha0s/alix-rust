use core::arch::asm;

#[cfg(feature = "qemu")]
const IPS: u64 = 0b010; // 40-bit
#[cfg(feature = "rpi5")]
const IPS: u64 = 0b010; // 40-bit

#[cfg(feature = "qemu")]
const VADDR_BITS: u64 = 48;
#[cfg(feature = "rpi5")]
const VADDR_BITS: u64 = 48;

pub fn enable_mmu(root_pa: u64) {
    // Configure MAIR/TCR/TTBR0 and enable the MMU for the provided page tables.
    unsafe {
        let mut sctlr: u64;
        asm!("mrs {0}, sctlr_el1", out(reg) sctlr, options(nostack, preserves_flags));
        sctlr &= !(1 << 0); // M
        sctlr &= !(1 << 2); // C
        sctlr &= !(1 << 12); // I
        asm!("msr sctlr_el1, {0}", in(reg) sctlr, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));

        let mair = 0x00u64 | (0xFFu64 << 8); // attr0=device, attr1=normal WBWA
        asm!("msr mair_el1, {0}", in(reg) mair, options(nostack, preserves_flags));

        let t0sz = 64u64 - VADDR_BITS;
        let mut tcr = (t0sz) // T0SZ
            | (0b01u64 << 8) // IRGN0 WBWA
            | (0b01u64 << 10) // ORGN0 WBWA
            | (0b11u64 << 12) // SH0 inner-shareable
            | (0b00u64 << 14) // TG0 4K
            | (IPS << 32);
        tcr |= 1u64 << 23; // EPD1: disable TTBR1 walks
        asm!("msr tcr_el1, {0}", in(reg) tcr, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));

        asm!("msr ttbr0_el1, {0}", in(reg) root_pa, options(nostack, preserves_flags));
        asm!("msr ttbr1_el1, {0}", in(reg) 0u64, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));

        asm!("dsb ish", options(nostack, preserves_flags));
        asm!("tlbi vmalle1", options(nostack, preserves_flags));
        asm!("dsb ish", "isb", options(nostack, preserves_flags));

        sctlr |= 1 << 0; // M (enable MMU)
        asm!("msr sctlr_el1, {0}", in(reg) sctlr, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));
    }
}

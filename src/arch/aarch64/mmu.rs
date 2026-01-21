use core::arch::asm;

#[cfg(feature = "qemu")]
const IPS: u64 = 0b010; // 40-bit
#[cfg(feature = "rpi5")]
const IPS: u64 = 0b010; // 40-bit

#[cfg(feature = "qemu")]
const VADDR_BITS: u64 = 48;
#[cfg(feature = "rpi5")]
const VADDR_BITS: u64 = 48;

#[cfg(feature = "rpi5")]
#[inline(always)]
fn early_uart_putc(b: u8) {
    const RP1_UART_FALLBACK: usize = 0x1c00_0300_00;
    unsafe { (RP1_UART_FALLBACK as *mut u32).write_volatile(b as u32) };
}

#[cfg(feature = "rpi5")]
#[inline(always)]
fn early_uart_delay() {
    for _ in 0..500 {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) }
    }
}

#[cfg(feature = "rpi5")]
fn early_mark(tag: u8) {
    early_uart_putc(b'E');
    early_uart_delay();
    early_uart_putc(tag);
    early_uart_delay();
    early_uart_putc(b'\r');
    early_uart_delay();
    early_uart_putc(b'\n');
    early_uart_delay();
}

pub fn enable_mmu(ttbr0_pa: u64, ttbr1_pa: u64) {
    // Configure MAIR/TCR/TTBR0/TTBR1 and enable the MMU.
    unsafe {
        #[cfg(feature = "rpi5")]
        early_mark(b'0');
        let mut sctlr: u64;
        asm!("mrs {0}, sctlr_el1", out(reg) sctlr, options(nostack, preserves_flags));
        // Keep current cache state; page tables were built with caches enabled.

        let mair = 0x00u64 | (0xFFu64 << 8); // attr0=device, attr1=normal WBWA
        asm!("msr mair_el1, {0}", in(reg) mair, options(nostack, preserves_flags));

        let t0sz = 64u64 - VADDR_BITS;
        let t1sz = 64u64 - VADDR_BITS;
        let tcr = (t0sz) // T0SZ
            | (t1sz << 16) // T1SZ
            | (0b01u64 << 8) // IRGN0 WBWA
            | (0b01u64 << 10) // ORGN0 WBWA
            | (0b11u64 << 12) // SH0 inner-shareable
            | (0b00u64 << 14) // TG0 4K
            | (0b01u64 << 24) // IRGN1 WBWA
            | (0b01u64 << 26) // ORGN1 WBWA
            | (0b11u64 << 28) // SH1 inner-shareable
            | (0b10u64 << 30) // TG1 4K
            | (IPS << 32);
        asm!("msr tcr_el1, {0}", in(reg) tcr, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));
        #[cfg(feature = "rpi5")]
        early_mark(b'1');

        asm!("msr ttbr0_el1, {0}", in(reg) ttbr0_pa, options(nostack, preserves_flags));
        asm!("msr ttbr1_el1, {0}", in(reg) ttbr1_pa, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));
        #[cfg(feature = "rpi5")]
        early_mark(b'2');

        asm!("dsb ish", options(nostack, preserves_flags));
        asm!("tlbi vmalle1", options(nostack, preserves_flags));
        asm!("dsb ish", "isb", options(nostack, preserves_flags));
        #[cfg(feature = "rpi5")]
        early_mark(b'3');

        sctlr |= 1 << 0; // M (ensure MMU on)
        asm!("msr sctlr_el1, {0}", in(reg) sctlr, options(nostack, preserves_flags));
        asm!("isb", options(nostack, preserves_flags));
        #[cfg(feature = "rpi5")]
        early_mark(b'4');
    }
}

pub fn set_ttbr0(ttbr0_pa: u64) {
    unsafe {
        asm!("msr ttbr0_el1, {0}", in(reg) ttbr0_pa, options(nostack, preserves_flags));
        asm!("tlbi vmalle1", options(nostack, preserves_flags));
        asm!("dsb ish", "isb", options(nostack, preserves_flags));
    }
}

pub fn set_ttbr1(ttbr1_pa: u64) {
    unsafe {
        asm!("msr ttbr1_el1, {0}", in(reg) ttbr1_pa, options(nostack, preserves_flags));
        asm!("tlbi vmalle1", options(nostack, preserves_flags));
        asm!("dsb ish", "isb", options(nostack, preserves_flags));
    }
}

pub fn enable_caches() {
    unsafe {
        let mut sctlr: u64;
        asm!("mrs {0}, sctlr_el1", out(reg) sctlr, options(nostack, preserves_flags));
        sctlr |= 1 << 2;  // C
        sctlr |= 1 << 12; // I
        asm!("msr sctlr_el1, {0}", in(reg) sctlr, options(nostack, preserves_flags));
        asm!("dsb ish", "isb", options(nostack, preserves_flags));
    }
}

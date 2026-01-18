#[cfg(feature = "qemu")]
use crate::drivers::mmio::{read32, write32};

#[cfg(feature = "qemu")]
const LOCAL_BASE: usize = 0x4000_0000;
#[cfg(feature = "qemu")]
const TIMER_INT_CTRL_OFFSET: usize = 0x40;
#[cfg(feature = "qemu")]
const IRQ_SOURCE_OFFSET: usize = 0x60;
#[cfg(feature = "qemu")]
const CORE_STRIDE: usize = 0x4;
#[cfg(feature = "qemu")]
const CNTP_IRQ_BIT: u32 = 1 << 1; // CNTPNS

#[cfg(feature = "qemu")]
pub fn enable_generic_timer_irq(core: usize) {
    // Route the generic timer interrupt to the specified core (QEMU).
    let addr = LOCAL_BASE + TIMER_INT_CTRL_OFFSET + (core * CORE_STRIDE);
    unsafe {
        write32(addr, CNTP_IRQ_BIT);
    }
}

#[cfg(feature = "qemu")]
pub fn generic_timer_pending(core: usize) -> bool {
    // Check if the generic timer IRQ is pending for this core.
    let addr = LOCAL_BASE + IRQ_SOURCE_OFFSET + (core * CORE_STRIDE);
    unsafe { (read32(addr) & CNTP_IRQ_BIT) != 0 }
}

#[cfg(not(feature = "qemu"))]
#[allow(dead_code)]
pub fn enable_generic_timer_irq(_core: usize) {}

#[cfg(not(feature = "qemu"))]
#[allow(dead_code)]
pub fn generic_timer_pending(_core: usize) -> bool {
    false
}

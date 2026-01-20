use crate::drivers::mmio::{read32, write32};
use crate::platform::board::{GICC_BASE, GICD_BASE};

const GICD_CTLR: usize = 0x000;
const GICD_IGROUPR0: usize = 0x080;
const GICD_ISENABLER0: usize = 0x100;
const GICD_ICENABLER0: usize = 0x180;
const GICD_IPRIORITYR: usize = 0x400;

const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_BPR: usize = 0x008;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;

const SPURIOUS_IRQ: u32 = 1023;
const TIMER_PPI: u32 = 30; // CNTPNS

pub fn init_dist() {
    // Initialize the distributor (CPU0 only).
    unsafe {
        write32(GICD_BASE + GICD_CTLR, 0);
        // Mark SGIs/PPIs as non-secure group 1.
        write32(GICD_BASE + GICD_IGROUPR0, 0xFFFF_FFFF);
        // Disable all SGIs/PPIs before enabling the timer.
        write32(GICD_BASE + GICD_ICENABLER0, 0xFFFF_FFFF);
        // Set priority for the timer PPI.
        set_priority(TIMER_PPI, 0x80);
        // Enable group0+group1.
        write32(GICD_BASE + GICD_CTLR, 0x3);
    }
}

pub fn init_cpu() {
    // Initialize the per-CPU interface.
    unsafe {
        write32(GICC_BASE + GICC_CTLR, 0);
        write32(GICC_BASE + GICC_PMR, 0xFF);
        write32(GICC_BASE + GICC_BPR, 0);
        // Enable group0+group1 at the CPU interface.
        write32(GICC_BASE + GICC_CTLR, 0x3);
    }
    // Banked SGI/PPI configuration for this CPU.
    unsafe {
        write32(GICD_BASE + GICD_IGROUPR0, 0xFFFF_FFFF);
    }
    set_priority(TIMER_PPI, 0x80);
    enable_timer_ppi();
}

pub fn ack_irq() -> Option<u32> {
    let iar = unsafe { read32(GICC_BASE + GICC_IAR) };
    let id = iar & 0x3ff;
    if id == SPURIOUS_IRQ {
        None
    } else {
        Some(id)
    }
}

pub fn end_irq(id: u32) {
    unsafe {
        write32(GICC_BASE + GICC_EOIR, id);
    }
}

fn enable_timer_ppi() {
    unsafe {
        // Enable PPI for the generic timer (banked per CPU).
        write32(GICD_BASE + GICD_ISENABLER0, 1u32 << TIMER_PPI);
    }
}

fn set_priority(irq: u32, prio: u8) {
    let reg = GICD_BASE + GICD_IPRIORITYR + ((irq & !3) as usize);
    let shift = (irq & 3) * 8;
    unsafe {
        let mut val = read32(reg);
        val &= !(0xFF << shift);
        val |= (prio as u32) << shift;
        write32(reg, val);
    }
}

pub fn timer_irq_id() -> u32 {
    TIMER_PPI
}

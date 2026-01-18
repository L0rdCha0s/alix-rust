use core::sync::atomic::{compiler_fence, Ordering};

use crate::drivers::mmio::{read32, write32};
use crate::platform::board::{MBOX_BASE, VC_MEM_BASE, VC_MEM_MASK};

const MBOX_READ: usize = 0x00;
const MBOX_STATUS: usize = 0x18;
const MBOX_WRITE: usize = 0x20;

const MBOX_STATUS_FULL: u32 = 1 << 31;
const MBOX_STATUS_EMPTY: u32 = 1 << 30;

const MBOX_CH_PROPERTY: u32 = 8;
const MBOX_RESPONSE_OK: u32 = 0x8000_0000;
const SPIN_LIMIT: usize = 1_000_000;

pub fn call(buffer: *mut u32) -> bool {
    let addr = buffer as usize;
    if (addr & 0xF) != 0 {
        return false;
    }

    let bus_addr = arm_to_vc(addr) | MBOX_CH_PROPERTY;

    compiler_fence(Ordering::SeqCst);

    unsafe {
        let mut spins = 0usize;
        while read32(MBOX_BASE + MBOX_STATUS) & MBOX_STATUS_FULL != 0 {
            spins += 1;
            if spins >= SPIN_LIMIT {
                return false;
            }
        }
        write32(MBOX_BASE + MBOX_WRITE, bus_addr);

        let mut loops = 0usize;
        loop {
            while read32(MBOX_BASE + MBOX_STATUS) & MBOX_STATUS_EMPTY != 0 {
                spins += 1;
                if spins >= SPIN_LIMIT {
                    return false;
                }
            }
            let resp = read32(MBOX_BASE + MBOX_READ);
            if (resp & 0xF) == MBOX_CH_PROPERTY && (resp & !0xF) == (bus_addr & !0xF) {
                let status = core::ptr::read_volatile(buffer.add(1));
                return status == MBOX_RESPONSE_OK;
            }
            loops += 1;
            if loops >= SPIN_LIMIT {
                return false;
            }
        }
    }
}

pub fn vc_to_arm(addr: u32) -> usize {
    (addr & VC_MEM_MASK) as usize
}

fn arm_to_vc(addr: usize) -> u32 {
    (addr as u32 & VC_MEM_MASK) | VC_MEM_BASE
}

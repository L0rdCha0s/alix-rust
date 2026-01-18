use core::arch::asm;

use crate::process;
use crate::timer;
use crate::trap::TrapFrame;

pub fn init_per_cpu(tick_ms: u64) {
    timer::init_tick(tick_ms);
    enable_irq();
}

pub fn enable_irq() {
    unsafe {
        asm!("msr daifclr, #2", options(nomem, nostack, preserves_flags));
    }
}

#[no_mangle]
pub extern "C" fn irq_handler(frame: *mut TrapFrame) -> *mut TrapFrame {
    timer::tick();
    process::schedule_from_irq(frame)
}

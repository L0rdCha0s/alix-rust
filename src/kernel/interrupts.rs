use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arch::aarch64::timer;
use crate::arch::aarch64::trap::TrapFrame;
use crate::drivers::keyboard;
#[cfg(feature = "qemu")]
use crate::drivers::local_intc;
use crate::kernel::process;
use crate::kernel::smp;
use crate::drivers::uart;

const LOG_IRQ: bool = false;
const LOG_EVERY: usize = 200;
static IRQ_LOG_TICKS: [AtomicUsize; smp::MAX_CPUS] = [
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
];

pub fn init_per_cpu(tick_ms: u64) {
    // Initialize per-core timer IRQs and enable interrupt delivery.
    timer::init_tick(tick_ms);
    #[cfg(feature = "qemu")]
    local_intc::enable_generic_timer_irq(smp::cpu_id());
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "irq init cpu{}", smp::cpu_id());
    });
    enable_irq();
}

pub fn enable_irq() {
    // Clear DAIF.I to unmask IRQs.
    unsafe {
        asm!("msr daifclr, #2", options(nomem, nostack, preserves_flags));
    }
}

#[no_mangle]
pub extern "C" fn irq_handler(frame: *mut TrapFrame) -> *mut TrapFrame {
    // Timer IRQ handler: poll input, update ticks, and schedule.
    #[cfg(feature = "qemu")]
    {
        if !local_intc::generic_timer_pending(smp::cpu_id()) {
            if LOG_IRQ {
                let cpu = smp::cpu_id();
                let tick = IRQ_LOG_TICKS[cpu].fetch_add(1, Ordering::Relaxed);
                if tick % LOG_EVERY == 0 {
                    uart::with_uart(|uart| {
                        use core::fmt::Write;
                        let _ = writeln!(uart, "irq cpu{} pending=0", cpu);
                    });
                }
            }
            return frame;
        }
    }
    if LOG_IRQ {
        let cpu = smp::cpu_id();
        let tick = IRQ_LOG_TICKS[cpu].fetch_add(1, Ordering::Relaxed);
        if tick % LOG_EVERY == 0 {
            uart::with_uart(|uart| {
                use core::fmt::Write;
                let _ = writeln!(uart, "irq cpu{} pending=1", cpu);
            });
        }
    }
    keyboard::poll();
    timer::tick();
    process::schedule_from_irq(frame)
}

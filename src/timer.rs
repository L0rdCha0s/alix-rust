use core::arch::asm;

static mut TICK_TICKS: u64 = 0;

#[inline(always)]
fn counter() -> u64 {
    let value: u64;
    unsafe {
        asm!("mrs {0}, cntpct_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

#[inline(always)]
fn frequency() -> u64 {
    let value: u64;
    unsafe {
        asm!("mrs {0}, cntfrq_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

pub fn init_tick(ms: u64) {
    let ticks = (frequency() * ms) / 1000;
    unsafe {
        TICK_TICKS = ticks.max(1);
        set_timer(TICK_TICKS);
    }
}

pub fn tick() {
    unsafe {
        if TICK_TICKS != 0 {
            set_timer(TICK_TICKS);
        }
    }
}

#[inline(always)]
unsafe fn set_timer(ticks: u64) {
    asm!(
        "msr cntp_tval_el0, {0}",
        "msr cntp_ctl_el0, {1}",
        in(reg) ticks,
        in(reg) 1u64,
        options(nomem, nostack, preserves_flags)
    );
}

pub fn delay_ms(ms: u64) {
    let ticks = (frequency() * ms) / 1000;
    let start = counter();
    while counter().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

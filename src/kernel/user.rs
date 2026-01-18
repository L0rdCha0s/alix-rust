use core::arch::asm;

static mut USER_ENTRY: Option<extern "C" fn() -> !> = None;
static mut USER_STACK_TOP: usize = 0;

pub const SYSCALL_WRITE: u64 = 1;
pub const SYSCALL_SLEEP_MS: u64 = 2;

pub fn init(entry: extern "C" fn() -> !, stack_top: usize) {
    unsafe {
        USER_ENTRY = Some(entry);
        USER_STACK_TOP = stack_top & !0xF;
    }
}

#[no_mangle]
pub extern "C" fn user_start() -> ! {
    unsafe {
        let entry = USER_ENTRY.expect("user entry not set") as usize;
        let sp = USER_STACK_TOP;
        asm!(
            "msr sp_el0, {sp}",
            "msr elr_el1, {entry}",
            "msr spsr_el1, xzr",
            "eret",
            sp = in(reg) sp,
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}

pub fn write(s: &str) -> u64 {
    unsafe { syscall_write(s.as_ptr(), s.len()) }
}

pub fn sleep_ms(ms: u64) -> u64 {
    unsafe { syscall_sleep_ms(ms) }
}

unsafe fn syscall_write(ptr: *const u8, len: usize) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") SYSCALL_WRITE,
        in("x0") ptr,
        in("x1") len,
        lateout("x0") ret,
        options(nostack)
    );
    ret
}

unsafe fn syscall_sleep_ms(ms: u64) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") SYSCALL_SLEEP_MS,
        in("x0") ms,
        lateout("x0") ret,
        options(nostack)
    );
    ret
}

#![allow(dead_code)]

use core::arch::asm;

static mut USER_ENTRY: Option<extern "C" fn() -> !> = None;
static mut USER_STACK_TOP: usize = 0;

pub const SYSCALL_OPEN: u64 = 1;
pub const SYSCALL_READ: u64 = 2;
pub const SYSCALL_WRITE: u64 = 3;
pub const SYSCALL_CLOSE: u64 = 4;
pub const SYSCALL_SLEEP_MS: u64 = 5;

pub const O_READ: u64 = 1 << 0;
pub const O_WRITE: u64 = 1 << 1;
pub const O_APPEND: u64 = 1 << 2;

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

pub fn open(path: &str, flags: u64) -> u64 {
    unsafe { syscall_open(path.as_ptr(), path.len(), flags) }
}

pub fn read(fd: u64, buf: &mut [u8]) -> u64 {
    unsafe { syscall_read(fd, buf.as_mut_ptr(), buf.len()) }
}

pub fn write(fd: u64, s: &str) -> u64 {
    unsafe { syscall_write(fd, s.as_ptr(), s.len()) }
}

pub fn write_bytes(fd: u64, buf: &[u8]) -> u64 {
    unsafe { syscall_write(fd, buf.as_ptr(), buf.len()) }
}

pub fn close(fd: u64) -> u64 {
    unsafe { syscall_close(fd) }
}

pub fn sleep_ms(ms: u64) -> u64 {
    unsafe { syscall_sleep_ms(ms) }
}

unsafe fn syscall_open(ptr: *const u8, len: usize, flags: u64) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") SYSCALL_OPEN,
        in("x0") ptr,
        in("x1") len,
        in("x2") flags,
        lateout("x0") ret,
        options(nostack)
    );
    ret
}

unsafe fn syscall_read(fd: u64, ptr: *mut u8, len: usize) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") SYSCALL_READ,
        in("x0") fd,
        in("x1") ptr,
        in("x2") len as u64,
        lateout("x0") ret,
        options(nostack)
    );
    ret
}

unsafe fn syscall_write(fd: u64, ptr: *const u8, len: usize) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") SYSCALL_WRITE,
        in("x0") fd,
        in("x1") ptr,
        in("x2") len as u64,
        lateout("x0") ret,
        options(nostack)
    );
    ret
}

unsafe fn syscall_close(fd: u64) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") SYSCALL_CLOSE,
        in("x0") fd,
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

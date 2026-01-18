use core::arch::asm;

use crate::arch::aarch64::timer;
use crate::arch::aarch64::trap::TrapFrame;
use crate::kernel::process;
use crate::kernel::vfs;

pub const SYSCALL_OPEN: u64 = 1;
pub const SYSCALL_READ: u64 = 2;
pub const SYSCALL_WRITE: u64 = 3;
pub const SYSCALL_CLOSE: u64 = 4;
pub const SYSCALL_SLEEP_MS: u64 = 5;

#[no_mangle]
pub extern "C" fn sync_handler(frame: *mut TrapFrame) -> *mut TrapFrame {
    let esr: u64;
    unsafe {
        asm!("mrs {0}, esr_el1", out(reg) esr, options(nomem, nostack, preserves_flags));
    }
    let ec = (esr >> 26) & 0x3f;
    if ec != 0x15 {
        return frame;
    }

    let tf = unsafe { &mut *frame };
    let syscall = tf.x[8];
    match syscall {
        SYSCALL_OPEN => {
            let ptr = tf.x[0] as *const u8;
            let len = tf.x[1] as usize;
            let flags = vfs::OpenFlags::from_bits(tf.x[2]);
            if ptr.is_null() || len == 0 {
                tf.x[0] = u64::MAX;
                return frame;
            }
            let path = unsafe { core::slice::from_raw_parts(ptr, len) };
            let desc = match vfs::open_bytes(path, flags) {
                Some(desc) => desc,
                None => {
                    tf.x[0] = u64::MAX;
                    return frame;
                }
            };
            match process::alloc_fd_current(desc) {
                Some(fd) => tf.x[0] = fd as u64,
                None => tf.x[0] = u64::MAX,
            }
        }
        SYSCALL_READ => {
            let fd = tf.x[0] as usize;
            let ptr = tf.x[1] as *mut u8;
            let len = tf.x[2] as usize;
            if ptr.is_null() || len == 0 {
                tf.x[0] = 0;
                return frame;
            }
            let desc = match process::get_fd_current(fd) {
                Some(desc) => desc,
                None => {
                    tf.x[0] = u64::MAX;
                    return frame;
                }
            };
            let buf = unsafe { core::slice::from_raw_parts_mut(ptr, len) };
            let read = vfs::read(&desc, buf);
            tf.x[0] = read as u64;
        }
        SYSCALL_WRITE => {
            let fd = tf.x[0] as usize;
            let ptr = tf.x[1] as *const u8;
            let len = tf.x[2] as usize;
            if ptr.is_null() || len == 0 {
                tf.x[0] = 0;
                return frame;
            }
            let desc = match process::get_fd_current(fd) {
                Some(desc) => desc,
                None => {
                    tf.x[0] = u64::MAX;
                    return frame;
                }
            };
            let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
            let wrote = vfs::write(&desc, bytes);
            tf.x[0] = wrote as u64;
        }
        SYSCALL_CLOSE => {
            let fd = tf.x[0] as usize;
            if process::close_fd_current(fd) {
                tf.x[0] = 0;
            } else {
                tf.x[0] = u64::MAX;
            }
        }
        SYSCALL_SLEEP_MS => {
            let ms = tf.x[0] as u64;
            timer::delay_ms(ms);
            tf.x[0] = 0;
        }
        _ => {
            tf.x[0] = u64::MAX;
        }
    }

    frame
}

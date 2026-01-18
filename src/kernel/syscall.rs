use core::arch::asm;

use crate::arch::aarch64::timer;
use crate::arch::aarch64::trap::TrapFrame;
use crate::kernel::process;
use crate::kernel::vfs;
use alloc::alloc::{alloc, dealloc, realloc, Layout};

pub const SYSCALL_OPEN: u64 = 1;
pub const SYSCALL_READ: u64 = 2;
pub const SYSCALL_WRITE: u64 = 3;
pub const SYSCALL_CLOSE: u64 = 4;
pub const SYSCALL_SLEEP_MS: u64 = 5;
pub const SYSCALL_ALLOC: u64 = 6;
pub const SYSCALL_REALLOC: u64 = 7;
pub const SYSCALL_FREE: u64 = 8;

#[no_mangle]
pub extern "C" fn sync_handler(frame: *mut TrapFrame) -> *mut TrapFrame {
    // Handle synchronous exceptions; dispatch syscalls or log faults.
    let esr: u64;
    unsafe {
        asm!("mrs {0}, esr_el1", out(reg) esr, options(nomem, nostack, preserves_flags));
    }
    let ec = (esr >> 26) & 0x3f;
    if ec != 0x15 {
        // Non-SVC exception: dump ESR/FAR/ELR and halt.
        let far: u64;
        unsafe {
            asm!("mrs {0}, far_el1", out(reg) far, options(nomem, nostack, preserves_flags));
        }
        let elr = unsafe { (*frame).elr };
        crate::drivers::uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(
                uart,
                "sync fault: ec={:#x} esr={:#x} far={:#x} elr={:#x}",
                ec, esr, far, elr
            );
        });
        loop {
            unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
        }
    }

    let tf = unsafe { &mut *frame };
    let syscall = tf.x[8];
    // Syscall ABI: x8 = number, x0..x3 = args, x0 = return.
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
        SYSCALL_ALLOC => {
            let size = tf.x[0] as usize;
            let align = tf.x[1] as usize;
            if size == 0 {
                tf.x[0] = 0;
                return frame;
            }
            let layout = match Layout::from_size_align(size, align.max(1)) {
                Ok(layout) => layout,
                Err(_) => {
                    tf.x[0] = u64::MAX;
                    return frame;
                }
            };
            let ptr = unsafe { alloc(layout) };
            tf.x[0] = if ptr.is_null() { u64::MAX } else { ptr as u64 };
        }
        SYSCALL_REALLOC => {
            let ptr = tf.x[0] as *mut u8;
            let old_size = tf.x[1] as usize;
            let new_size = tf.x[2] as usize;
            let align = tf.x[3] as usize;
            let layout = match Layout::from_size_align(old_size.max(1), align.max(1)) {
                Ok(layout) => layout,
                Err(_) => {
                    tf.x[0] = u64::MAX;
                    return frame;
                }
            };
            if ptr.is_null() {
                if new_size == 0 {
                    tf.x[0] = 0;
                    return frame;
                }
                let new_layout = match Layout::from_size_align(new_size, align.max(1)) {
                    Ok(layout) => layout,
                    Err(_) => {
                        tf.x[0] = u64::MAX;
                        return frame;
                    }
                };
                let new_ptr = unsafe { alloc(new_layout) };
                tf.x[0] = if new_ptr.is_null() { u64::MAX } else { new_ptr as u64 };
                return frame;
            }
            if new_size == 0 {
                unsafe { dealloc(ptr, layout) };
                tf.x[0] = 0;
                return frame;
            }
            let new_ptr = unsafe { realloc(ptr, layout, new_size) };
            tf.x[0] = if new_ptr.is_null() { u64::MAX } else { new_ptr as u64 };
        }
        SYSCALL_FREE => {
            let ptr = tf.x[0] as *mut u8;
            let size = tf.x[1] as usize;
            let align = tf.x[2] as usize;
            if ptr.is_null() || size == 0 {
                tf.x[0] = 0;
                return frame;
            }
            let layout = match Layout::from_size_align(size, align.max(1)) {
                Ok(layout) => layout,
                Err(_) => {
                    tf.x[0] = u64::MAX;
                    return frame;
                }
            };
            unsafe { dealloc(ptr, layout) };
            tf.x[0] = 0;
        }
        _ => {
            tf.x[0] = u64::MAX;
        }
    }

    frame
}

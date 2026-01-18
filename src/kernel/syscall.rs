use core::arch::asm;

use crate::arch::aarch64::timer;
use crate::arch::aarch64::trap::TrapFrame;
use crate::drivers::framebuffer;

pub const SYSCALL_WRITE: u64 = 1;
pub const SYSCALL_SLEEP_MS: u64 = 2;

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
        SYSCALL_WRITE => {
            let ptr = tf.x[0] as *const u8;
            let len = tf.x[1] as usize;
            if !ptr.is_null() && len > 0 {
                let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
                let wrote = framebuffer::try_with_console(|console| {
                    for &b in bytes {
                        console.write_byte(b);
                    }
                });
                if !wrote {
                    tf.x[0] = 0;
                    return frame;
                }
            }
            tf.x[0] = len as u64;
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

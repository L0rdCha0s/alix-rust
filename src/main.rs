#![no_std]
#![no_main]

use core::arch::global_asm;

mod arch;
mod drivers;
mod gfx;
mod kernel;
mod platform;
mod util;

use crate::arch::aarch64::timer;
use crate::drivers::{framebuffer, uart};
use crate::kernel::{interrupts, process, smp, user, vfs};

global_asm!(include_str!("arch/aarch64/boot.S"));
global_asm!(include_str!("arch/aarch64/exception.S"));

const USER_STACK_SIZE: usize = 0x4000;
#[repr(align(16))]
struct UserStack([u8; USER_STACK_SIZE]);
static mut USER_STACK: UserStack = UserStack([0; USER_STACK_SIZE]);

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart::init();

    process::init();
    vfs::init();

    #[cfg(feature = "qemu")]
    loop {
        if try_init_console() {
            break;
        }
        timer::delay_ms(500);
    }

    #[cfg(not(feature = "qemu"))]
    {
        if !try_init_console() {
            uart::with_uart(|uart| {
                use core::fmt::Write;
                let _ = writeln!(uart, "Framebuffer init failed; using UART");
            });
        }
    }

    let fb = vfs::open_path("/dev/fb0", vfs::OpenFlags::new(false, true, false));
    let stdin = vfs::open_path("/dev/kbd0", vfs::OpenFlags::new(true, false, false));
    process::set_init_fd(vfs::FD_STDIN, stdin);
    process::set_init_fd(vfs::FD_STDOUT, fb);
    process::set_init_fd(vfs::FD_STDERR, fb);

    let user_sp = unsafe {
        core::ptr::addr_of!(USER_STACK.0)
            .cast::<u8>()
            .add(USER_STACK_SIZE) as usize
    };
    user::init(user_shell, user_sp);

    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "CPU{} online", smp::cpu_id());
        let _ = writeln!(uart, "Bringing up secondary cores...");
        for core in 1..smp::MAX_CPUS {
            let _ = writeln!(uart, "Releasing CPU{}", core);
        }
    });

    uart::with_uart(|uart| {
        use core::fmt::Write;
        for core in 0..smp::MAX_CPUS {
            if let Some(pid) = process::create("idle", idle_loop, 0) {
                let _ = writeln!(uart, "Created idle process {} for CPU{}", pid.0, core);
            }
        }
        if let Some(pid) = process::create_user("shell", user::user_start, 0) {
            let _ = writeln!(uart, "Created process {} (shell user)", pid.0);
        }
        process::for_each(|proc| {
            let mode = match proc.mode {
                process::ProcessMode::Kernel => "K",
                process::ProcessMode::User => "U",
            };
            let _ = writeln!(
                uart,
                "Process {}: {} [{}]",
                proc.id.0, proc.name, mode
            );
        });
    });

    smp::start_secondary_cores();

    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "Hello, world!");
    });

    interrupts::init_per_cpu(10);
    process::start_on_cpu(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    halt();
}

#[inline(always)]
fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
    }
}

#[no_mangle]
pub extern "C" fn idle_loop() -> ! {
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
    }
}

#[no_mangle]
pub extern "C" fn user_shell() -> ! {
    let stdout = vfs::FD_STDOUT as u64;
    let stdin = vfs::FD_STDIN as u64;
    let mut line = [0u8; 128];
    loop {
        let _ = user::write(stdout, "$ ");
        let mut len = 0usize;
        let mut saw_cr = false;
        loop {
            let mut byte = [0u8; 1];
            let read = user::read(stdin, &mut byte);
            if read == 0 || read == u64::MAX {
                let _ = user::sleep_ms(10);
                continue;
            }
            let mut b = byte[0];
            if b == b'\r' {
                saw_cr = true;
                b = b'\n';
            } else if b == b'\n' {
                if saw_cr {
                    saw_cr = false;
                    continue;
                }
            } else {
                saw_cr = false;
            }
            if b == b'\n' || b == b'\r' {
                let _ = user::write(stdout, "\n");
                if len > 0 {
                    let _ = user::write_bytes(stdout, &line[..len]);
                }
                let _ = user::write(stdout, "\n");
                break;
            }
            if b == 0x08 || b == 0x7f {
                if len > 0 {
                    len -= 1;
                    let _ = user::write(stdout, "\u{8} \u{8}");
                }
                continue;
            }
            if len < line.len() {
                line[len] = b;
                len += 1;
                let mut echo = [0u8; 1];
                echo[0] = b;
                let _ = user::write_bytes(stdout, &echo);
            }
        }
    }
}

fn try_init_console() -> bool {
    let modes = [(1280, 1024), (1024, 768), (1920, 1080)];
    for (w, h) in modes {
        uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(uart, "Framebuffer init attempt {}x{}", w, h);
        });
        match framebuffer::init_console_with_mode(w, h, 0x00FF_FFFF, 0x0000_0000) {
            Ok((ow, oh)) => {
                uart::with_uart(|uart| {
                    use core::fmt::Write;
                    let _ = writeln!(uart, "Framebuffer {}x{} -> {}x{}", w, h, ow, oh);
                });
                return true;
            }
            Err(err) => {
                uart::with_uart(|uart| {
                    use core::fmt::Write;
                    let _ = writeln!(uart, "Framebuffer {}x{} failed: {}", w, h, fb_err_str(err));
                });
            }
        }
    }
    false
}

fn fb_err_str(err: framebuffer::InitError) -> &'static str {
    match err {
        framebuffer::InitError::MailboxCallFailed => "mailbox call failed",
        framebuffer::InitError::NoFramebuffer => "no framebuffer address",
        framebuffer::InitError::NoPitch => "no pitch returned",
    }
}

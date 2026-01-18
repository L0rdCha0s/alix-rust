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
use crate::kernel::{interrupts, process, smp};

global_asm!(include_str!("arch/aarch64/boot.S"));
global_asm!(include_str!("arch/aarch64/exception.S"));

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart::init();

    process::init();

    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "CPU{} online", smp::cpu_id());
        let _ = writeln!(uart, "Bringing up secondary cores...");
        for core in 1..smp::MAX_CPUS {
            let _ = writeln!(uart, "Releasing CPU{}", core);
        }
    });

    smp::start_secondary_cores();

    uart::with_uart(|uart| {
        use core::fmt::Write;
        if let Some(pid) = process::create("procA", process_a, 0) {
            let _ = writeln!(uart, "Created process {} (A)", pid.0);
        }
        if let Some(pid) = process::create("procB", process_b, 0) {
            let _ = writeln!(uart, "Created process {} (B)", pid.0);
        }
        if let Some(pid) = process::create("procC", process_c, 0) {
            let _ = writeln!(uart, "Created process {} (C)", pid.0);
        }
        if let Some(pid) = process::create("procD", process_d, 0) {
            let _ = writeln!(uart, "Created process {} (D)", pid.0);
        }
        if let Some(pid) = process::create("procE", process_e, 0) {
            let _ = writeln!(uart, "Created process {} (E)", pid.0);
        }
        process::for_each(|proc| {
            let _ = writeln!(
                uart,
                "Process {}: {}",
                proc.id.0,
                proc.name
            );
        });
    });

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
pub extern "C" fn process_a() -> ! {
    run_letter('A', 200)
}

#[no_mangle]
pub extern "C" fn process_b() -> ! {
    run_letter('B', 350)
}

#[no_mangle]
pub extern "C" fn process_c() -> ! {
    run_letter('C', 500)
}

#[no_mangle]
pub extern "C" fn process_d() -> ! {
    run_letter('D', 650)
}

#[no_mangle]
pub extern "C" fn process_e() -> ! {
    run_letter('E', 800)
}

fn run_letter(letter: char, delay_ms: u64) -> ! {
    loop {
        let core = smp::cpu_id();
        framebuffer::with_console(|console| {
            use core::fmt::Write;
            let _ = writeln!(console, "CPU{}: {}", core, letter);
        });
        timer::delay_ms(delay_ms);
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

#![no_std]
#![no_main]

use core::arch::global_asm;

mod board;
mod font;
mod framebuffer;
mod interrupts;
mod mailbox;
mod mmio;
mod process;
mod smp;
mod sync;
mod timer;
mod trap;
mod uart;

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
        if let Some(pid) = process::create_on_cpu("cpu0", per_cpu_process, 0, 0) {
            let _ = writeln!(uart, "Created process {} on CPU0", pid.0);
        }
        if let Some(pid) = process::create_on_cpu("cpu1", per_cpu_process, 0, 1) {
            let _ = writeln!(uart, "Created process {} on CPU1", pid.0);
        }
        if let Some(pid) = process::create_on_cpu("cpu2", per_cpu_process, 0, 2) {
            let _ = writeln!(uart, "Created process {} on CPU2", pid.0);
        }
        if let Some(pid) = process::create_on_cpu("cpu3", per_cpu_process, 0, 3) {
            let _ = writeln!(uart, "Created process {} on CPU3", pid.0);
        }
        process::for_each(|proc| {
            let _ = writeln!(
                uart,
                "Process {}: {} (cpu={})",
                proc.id.0,
                proc.name,
                proc.cpu_affinity
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
pub extern "C" fn per_cpu_process() -> ! {
    let core = smp::cpu_id();
    let letter = match core {
        0 => 'A',
        1 => 'B',
        2 => 'C',
        3 => 'D',
        _ => '?',
    };

    loop {
        framebuffer::with_console(|console| {
            use core::fmt::Write;
            let _ = writeln!(console, "{}", letter);
        });
        timer::delay_ms(500);
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

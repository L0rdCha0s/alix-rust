#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::arch::global_asm;

mod arch;
mod drivers;
mod gfx;
mod kernel;
mod mm;
mod platform;
mod user;
mod util;

#[cfg(feature = "qemu")]
use crate::arch::aarch64::timer;
use crate::drivers::{framebuffer, uart};
use crate::kernel::{interrupts, process, smp, user as kuser, vfs};
use crate::user::shell;

global_asm!(include_str!("arch/aarch64/boot.S"));
global_asm!(include_str!("arch/aarch64/exception.S"));

#[cfg(feature = "rpi5")]
const RP1_UART_FALLBACK: usize = 0x1c00_0300_00;

#[cfg(feature = "rpi5")]
#[inline(always)]
unsafe fn early_uart_putc(b: u8) {
    (RP1_UART_FALLBACK as *mut u32).write_volatile(b as u32);
}

#[cfg(feature = "rpi5")]
fn early_uart_print(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            unsafe { early_uart_putc(b'\r'); }
        }
        unsafe { early_uart_putc(b); }
    }
}

const USER_STACK_SIZE: usize = 512 * 1024;
#[repr(align(16))]
struct UserStack([u8; USER_STACK_SIZE]);
static mut USER_STACK: UserStack = UserStack([0; USER_STACK_SIZE]);

#[no_mangle]
pub extern "C" fn kernel_main(dtb_pa: u64) -> ! {
    #[cfg(feature = "rpi5")]
    early_uart_print("K0\n");

    // Early UART for QEMU only; RPi5 UART base is discovered after DTB parse.
    #[cfg(feature = "qemu")]
    uart::init();

    #[cfg(feature = "rpi5")]
    {
        let mut found = false;
        if let Some(info) = mm::dtb::find_uart(dtb_pa) {
            uart::set_base(info.addr as usize);
            uart::set_clock_hz(info.clock_hz);
            uart::set_reg_shift(info.reg_shift);
            uart::set_reg_io_width(info.reg_io_width);
            uart::set_skip_init(info.skip_init);
            found = true;
        }
        if !found {
            // Fallback to RP1 UART0 base (matches firmware RP1_UART line).
            uart::set_base(RP1_UART_FALLBACK);
            uart::set_reg_shift(0);
            uart::set_reg_io_width(4);
            uart::set_skip_init(true);
        }
        uart::init();
        early_uart_print("K1\n");
    }

    // Parse DTB, build memory map, initialize allocators, and enable paging/MMU.
    mm::init(dtb_pa);
    #[cfg(feature = "rpi5")]
    early_uart_print("K2\n");

    // Bring up UART on real hardware after MMU mappings are installed (if not already).
    #[cfg(feature = "rpi5")]
    if !uart::is_ready() {
        uart::init();
    }

    // Process table + VFS must exist before spawning kernel/user processes.
    process::init();
    vfs::init();

    #[cfg(feature = "qemu")]
    loop {
        if try_init_console(dtb_pa) {
            break;
        }
        // QEMU framebuffer init can fail transiently; keep retrying.
        timer::delay_ms(500);
    }

    #[cfg(not(feature = "qemu"))]
    {
        if !try_init_console(dtb_pa) {
            uart::with_uart(|uart| {
                use core::fmt::Write;
                let _ = writeln!(uart, "Framebuffer init failed; using UART");
            });
        }
    }

    // Wire up standard FDs for the initial process tree.
    let fb = vfs::open_path("/dev/fb0", vfs::OpenFlags::new(false, true, false));
    let stdin = vfs::open_path("/dev/kbd0", vfs::OpenFlags::new(true, false, false));
    process::set_init_fd(vfs::FD_STDIN, stdin);
    process::set_init_fd(vfs::FD_STDOUT, fb);
    process::set_init_fd(vfs::FD_STDERR, fb);

    // User entry + stack setup for the shell process.
    let user_sp = unsafe {
        core::ptr::addr_of!(USER_STACK.0)
            .cast::<u8>()
            .add(USER_STACK_SIZE) as usize
    };
    kuser::init(shell::user_shell, user_sp);

    // Log core status before releasing secondary CPUs.
    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "CPU{} online", smp::cpu_id());
        let _ = writeln!(uart, "Bringing up secondary cores...");
        for core in 1..smp::MAX_CPUS {
            let _ = writeln!(uart, "Releasing CPU{}", core);
        }
    });

    // Create kernel idle loops and the user shell process.
    uart::with_uart(|uart| {
        use core::fmt::Write;
        if let Some(pid) = process::create_user("shell", kuser::user_start, 0) {
            let _ = writeln!(uart, "Created process {} (shell user)", pid.0);
        }
        for core in 0..smp::MAX_CPUS {
            if let Some(pid) = process::create("idle", idle_loop, 0) {
                let _ = writeln!(uart, "Created idle process {} for CPU{}", pid.0, core);
            }
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

    // Release secondary cores once processes are ready.
    smp::start_secondary_cores();

    uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "Hello, world!");
    });

    // Enable per-core timer IRQs and enter the scheduler on CPU0.
    interrupts::init_per_cpu(10);
    process::start_on_cpu(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    halt();
}

#[inline(always)]
fn halt() -> ! {
    // Low-power wait loop used for fatal errors or idle CPU state.
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
    }
}

#[no_mangle]
pub extern "C" fn idle_loop() -> ! {
    // Kernel idle process: sleep until an interrupt fires.
    loop {
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
    }
}

fn try_init_console(dtb_pa: u64) -> bool {
    // Prefer a firmware-provided simple framebuffer if present.
    if let Some(info) = mm::dtb::find_simplefb(dtb_pa) {
        uart::with_uart(|uart| {
            use core::fmt::Write;
            let _ = writeln!(
                uart,
                "simplefb: addr={:#x} size={:#x} {}x{} stride={}",
                info.addr, info.size, info.width, info.height, info.stride
            );
        });
        match framebuffer::init_console_from_simplefb(&info, 0x00FF_FFFF, 0x0000_0000) {
            Ok((ow, oh)) => {
                uart::with_uart(|uart| {
                    use core::fmt::Write;
                    let _ = writeln!(uart, "simplefb active {}x{}", ow, oh);
                });
                return true;
            }
            Err(err) => {
                uart::with_uart(|uart| {
                    use core::fmt::Write;
                    let _ = writeln!(uart, "simplefb init failed: {}", fb_err_str(err));
                });
            }
        }
    }

    // Try multiple mailbox framebuffer modes; return true on first success.
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
        framebuffer::InitError::InvalidSimpleFb => "invalid simplefb data",
    }
}

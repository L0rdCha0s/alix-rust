use core::arch::asm;

pub const MAX_CPUS: usize = 4;
const STACK_SIZE: usize = 0x4000;

#[allow(dead_code)]
#[repr(align(16))]
#[derive(Copy, Clone)]
struct Stack([u8; STACK_SIZE]);

#[no_mangle]
#[link_section = ".bss.stack"]
static mut __secondary_stacks: [Stack; MAX_CPUS] = [Stack([0; STACK_SIZE]); MAX_CPUS];

#[no_mangle]
#[link_section = ".data.secondary"]
pub static mut __secondary_table: [u64; MAX_CPUS] = [0; MAX_CPUS];

extern "C" {
    fn secondary_start() -> !;
}

pub fn cpu_id() -> usize {
    let mut id: usize;
    unsafe {
        asm!("mrs {0}, mpidr_el1", out(reg) id, options(nomem, nostack, preserves_flags));
    }
    id & 3
}

#[allow(dead_code)]
pub fn current_el() -> u8 {
    let mut el: usize;
    unsafe {
        asm!("mrs {0}, CurrentEL", out(reg) el, options(nomem, nostack, preserves_flags));
    }
    ((el >> 2) & 0x3) as u8
}

pub fn start_secondary_cores() {
    unsafe {
        for core in 1..MAX_CPUS {
            __secondary_table[core] = secondary_start as *const () as u64;
        }

        #[cfg(feature = "qemu")]
        {
            const SPIN_TABLE_BASE: usize = 0xD8;
            for core in 1..MAX_CPUS {
                let slot = (SPIN_TABLE_BASE + (core * 8)) as *mut u64;
                core::ptr::write_volatile(slot, secondary_start as *const () as u64);
            }
        }

        asm!("dsb sy", "sev", options(nomem, nostack, preserves_flags));
    }
}

#[no_mangle]
pub extern "C" fn secondary_rust_entry(_core_id: usize) -> ! {
    let core_id = cpu_id();
    crate::uart::with_uart(|uart| {
        use core::fmt::Write;
        let _ = writeln!(uart, "CPU{} online", core_id);
    });

    crate::interrupts::init_per_cpu(10);
    crate::process::start_on_cpu(core_id);
}

#![allow(dead_code)]

use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::drivers::mmio::{read32, write32};
use crate::platform::board::UART_BASE;
use crate::util::sync::SpinLock;

#[cfg(feature = "qemu")]
use crate::platform::board::GPIO_BASE;

const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_IBRD: usize = 0x24;
const UART_FBRD: usize = 0x28;
const UART_LCRH: usize = 0x2C;
const UART_CR: usize = 0x30;
const UART_IMSC: usize = 0x38;
const UART_ICR: usize = 0x44;

#[cfg(feature = "qemu")]
const GPFSEL1: usize = 0x04;
#[cfg(feature = "qemu")]
const GPPUD: usize = 0x94;
#[cfg(feature = "qemu")]
const GPPUDCLK0: usize = 0x98;

pub struct Uart;

static UART_LOCK: SpinLock<()> = SpinLock::new(());
#[cfg(feature = "qemu")]
static UART_BASE_ADDR: AtomicUsize = AtomicUsize::new(UART_BASE);
#[cfg(feature = "rpi5")]
static UART_BASE_ADDR: AtomicUsize = AtomicUsize::new(UART_BASE);
static UART_READY: AtomicBool = AtomicBool::new(false);
static UART_SKIP_INIT: AtomicBool = AtomicBool::new(false);
static UART_CLOCK_HZ: AtomicUsize = AtomicUsize::new(0);
static UART_REG_SHIFT: AtomicUsize = AtomicUsize::new(0);
static UART_REG_IO_WIDTH: AtomicUsize = AtomicUsize::new(4);

#[inline(always)]
fn uart_base() -> usize {
    UART_BASE_ADDR.load(Ordering::Relaxed)
}

pub fn set_base(base: usize) {
    UART_BASE_ADDR.store(base, Ordering::Relaxed);
}

pub fn set_clock_hz(clock: Option<u32>) {
    UART_CLOCK_HZ.store(clock.unwrap_or(0) as usize, Ordering::Relaxed);
}

pub fn set_reg_shift(shift: u32) {
    UART_REG_SHIFT.store(shift as usize, Ordering::Relaxed);
}

pub fn set_reg_io_width(width: u32) {
    UART_REG_IO_WIDTH.store(width as usize, Ordering::Relaxed);
}

pub fn set_skip_init(skip: bool) {
    UART_SKIP_INIT.store(skip, Ordering::Relaxed);
}

pub fn is_ready() -> bool {
    UART_READY.load(Ordering::Relaxed)
}

pub fn init() {
    // Initialize PL011 UART for early serial logging.
    let base = uart_base();
    if base == 0 {
        return;
    }
    if UART_SKIP_INIT.load(Ordering::Relaxed) {
        // Firmware configured the UART; just ensure TX/RX are enabled.
        unsafe {
            let cr = read32(reg_addr(base, UART_CR));
            let enable = (1 << 0) | (1 << 8) | (1 << 9);
            write32(reg_addr(base, UART_CR), cr | enable);
        }
        UART_READY.store(true, Ordering::Relaxed);
        return;
    }
    unsafe {
        // Disable UART0.
        write32(reg_addr(base, UART_CR), 0);

        #[cfg(feature = "qemu")]
        {
            // GPIO14/15 to ALT0 (TXD0/RXD0).
            let mut gpfsel1 = read32(GPIO_BASE + GPFSEL1);
            gpfsel1 &= !((7 << 12) | (7 << 15));
            gpfsel1 |= (4 << 12) | (4 << 15);
            write32(GPIO_BASE + GPFSEL1, gpfsel1);

            // Disable pull-up/down.
            write32(GPIO_BASE + GPPUD, 0);
            delay(150);
            write32(GPIO_BASE + GPPUDCLK0, (1 << 14) | (1 << 15));
            delay(150);
            write32(GPIO_BASE + GPPUDCLK0, 0);
        }

        // Clear interrupts.
        write32(reg_addr(base, UART_ICR), 0x7FF);

        // 115200 baud; prefer DTB-provided clock when available.
        let clock_hz = UART_CLOCK_HZ.load(Ordering::Relaxed) as u32;
        let (ibrd, fbrd) = if clock_hz != 0 {
            baud_divisors(clock_hz, 115_200)
        } else {
            (26, 3) // 48 MHz default.
        };
        write32(reg_addr(base, UART_IBRD), ibrd);
        write32(reg_addr(base, UART_FBRD), fbrd);

        // 8N1, enable FIFO.
        write32(reg_addr(base, UART_LCRH), (1 << 4) | (3 << 5));

        // Mask all interrupts.
        write32(reg_addr(base, UART_IMSC), 0);

        // Enable UART0, TX, RX.
        write32(reg_addr(base, UART_CR), (1 << 0) | (1 << 8) | (1 << 9));
    }
    UART_READY.store(true, Ordering::Relaxed);
}

pub fn write_byte(byte: u8) {
    // Blocking transmit of a single byte.
    if !is_ready() {
        return;
    }
    let base = uart_base();
    #[cfg(feature = "rpi5")]
    {
        // Firmware-initialized RP1 UART can be poked directly when skip-init is set.
        if UART_SKIP_INIT.load(Ordering::Relaxed) {
            unsafe { write32(reg_addr(base, UART_DR), byte as u32) };
            return;
        }
    }
    unsafe {
        #[cfg(feature = "rpi5")]
        let mut spins: u32 = 0;
        while (read32(reg_addr(base, UART_FR)) & (1 << 5)) != 0 {
            // wait for space in FIFO (cap spins to avoid hard hangs if FR is invalid)
            #[cfg(feature = "rpi5")]
            {
                spins += 1;
                if spins > 1_000_000 {
                    break;
                }
            }
        }
        write32(reg_addr(base, UART_DR), byte as u32);
    }
}

pub fn read_byte_nonblocking() -> Option<u8> {
    // Non-blocking read; returns None if RX FIFO is empty.
    if !is_ready() {
        return None;
    }
    let base = uart_base();
    unsafe {
        if (read32(reg_addr(base, UART_FR)) & (1 << 4)) != 0 {
            // RXFE: receive FIFO empty
            None
        } else {
            Some(read32(reg_addr(base, UART_DR)) as u8)
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                write_byte(b'\r');
            }
            write_byte(b);
        }
        Ok(())
    }
}

pub fn with_uart<F: FnOnce(&mut Uart)>(f: F) {
    // Serialize access to the UART to avoid interleaved output.
    if !is_ready() {
        return;
    }
    let _guard = UART_LOCK.lock();
    let mut uart = Uart;
    f(&mut uart);
}

#[inline(always)]
fn delay(count: u32) {
    for _ in 0..count {
        unsafe { core::arch::asm!("nop", options(nomem, nostack, preserves_flags)) }
    }
}

#[inline(always)]
fn reg_addr(base: usize, offset: usize) -> usize {
    let shift = UART_REG_SHIFT.load(Ordering::Relaxed);
    base + (offset << shift)
}

fn baud_divisors(clock_hz: u32, baud: u32) -> (u32, u32) {
    let baud = baud as u64;
    let clock = clock_hz as u64;
    if baud == 0 {
        return (0, 0);
    }
    let denom = 16 * baud;
    let ibrd = (clock / denom) as u32;
    let rem = clock % denom;
    let frac = ((rem * 64 + denom / 2) / denom) as u32;
    let fbrd = if frac > 63 { 63 } else { frac };
    (ibrd, fbrd)
}

// MMIO helpers live in mmio.rs

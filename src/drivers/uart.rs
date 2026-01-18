#![allow(dead_code)]

use core::fmt;

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

pub fn init() {
    unsafe {
        // Disable UART0.
        write32(UART_BASE + UART_CR, 0);

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
        write32(UART_BASE + UART_ICR, 0x7FF);

        // 115200 baud assuming 48 MHz UART clock.
        write32(UART_BASE + UART_IBRD, 26);
        write32(UART_BASE + UART_FBRD, 3);

        // 8N1, enable FIFO.
        write32(UART_BASE + UART_LCRH, (1 << 4) | (3 << 5));

        // Mask all interrupts.
        write32(UART_BASE + UART_IMSC, 0);

        // Enable UART0, TX, RX.
        write32(UART_BASE + UART_CR, (1 << 0) | (1 << 8) | (1 << 9));
    }
}

pub fn write_byte(byte: u8) {
    unsafe {
        while (read32(UART_BASE + UART_FR) & (1 << 5)) != 0 {
            // wait for space in FIFO
        }
        write32(UART_BASE + UART_DR, byte as u32);
    }
}

pub fn read_byte_nonblocking() -> Option<u8> {
    unsafe {
        if (read32(UART_BASE + UART_FR) & (1 << 4)) != 0 {
            // RXFE: receive FIFO empty
            None
        } else {
            Some(read32(UART_BASE + UART_DR) as u8)
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

// MMIO helpers live in mmio.rs

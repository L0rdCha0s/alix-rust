#[cfg(all(feature = "rpi5", feature = "qemu"))]
compile_error!("Select only one board feature: rpi5 (default) or qemu.");

#[cfg(not(any(feature = "rpi5", feature = "qemu")))]
compile_error!("Select a board feature: rpi5 (default) or qemu.");

// Raspberry Pi 5 (BCM2712)
#[cfg(feature = "rpi5")]
pub const SOC_BASE: usize = 0x107c_0000_00;
#[cfg(feature = "rpi5")]
pub const SOC_MMIO_SIZE: usize = 0x8000_0000;
#[cfg(feature = "rpi5")]
pub const GICD_BASE: usize = SOC_BASE + 0x7fff_9000;
#[cfg(feature = "rpi5")]
pub const GICC_BASE: usize = SOC_BASE + 0x7fff_c000;
#[cfg(feature = "rpi5")]
pub const MBOX_BASE: usize = SOC_BASE + 0x7c01_3880;
#[cfg(feature = "rpi5")]
#[allow(dead_code)]
pub const UART_BASE: usize = SOC_BASE + 0x7d00_1000;
#[cfg(feature = "rpi5")]
pub const VC_MEM_BASE: u32 = 0x0000_0000; // DMA is identity-mapped on BCM2712
#[cfg(feature = "rpi5")]
pub const VC_MEM_MASK: u32 = 0xFFFF_FFFF;

// QEMU (raspi3b)
#[cfg(feature = "qemu")]
pub const PERIPHERAL_BASE: usize = 0x3F00_0000;
#[cfg(feature = "qemu")]
pub const PERIPHERAL_SIZE: usize = 0x0100_0000;
#[cfg(feature = "qemu")]
pub const MBOX_BASE: usize = PERIPHERAL_BASE + 0x0000_B880;
#[cfg(feature = "qemu")]
pub const UART_BASE: usize = PERIPHERAL_BASE + 0x0020_1000;
#[cfg(feature = "qemu")]
pub const GPIO_BASE: usize = PERIPHERAL_BASE + 0x0020_0000;
#[cfg(feature = "qemu")]
pub const VC_MEM_BASE: u32 = 0x4000_0000; // VC bus alias for RAM on BCM283x
#[cfg(feature = "qemu")]
pub const VC_MEM_MASK: u32 = 0x3FFF_FFFF;
#[cfg(feature = "qemu")]
pub const QEMU_RAM_SIZE: u64 = 8 * 1024 * 1024 * 1024;

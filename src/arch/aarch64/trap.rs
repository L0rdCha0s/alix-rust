#[repr(C)]
pub struct TrapFrame {
    pub x: [u64; 31],
    pub pad: u64,
    pub elr: u64,
    pub spsr: u64,
    pub sp_el0: u64,
}

impl TrapFrame {
    pub fn new(entry: usize) -> Self {
        let frame = TrapFrame {
            x: [0; 31],
            pad: 0,
            elr: entry as u64,
            spsr: 0x5, // EL1h, interrupts enabled
            sp_el0: 0,
        };
        frame
    }
}

pub const TRAP_FRAME_SIZE: usize = core::mem::size_of::<TrapFrame>();

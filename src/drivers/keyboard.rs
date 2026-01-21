use crate::util::sync::SpinLock;

#[cfg(any(feature = "qemu", feature = "rpi5"))]
use crate::drivers::uart;

const BUF_SIZE: usize = 256;

struct RingBuffer {
    buf: [u8; BUF_SIZE],
    head: usize,
    #[allow(dead_code)]
    tail: usize,
    len: usize,
}

impl RingBuffer {
    const fn new() -> Self {
        Self {
            buf: [0; BUF_SIZE],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    #[allow(dead_code)]
    fn push(&mut self, b: u8) -> bool {
        if self.len == BUF_SIZE {
            return false;
        }
        self.buf[self.tail] = b;
        self.tail = (self.tail + 1) % BUF_SIZE;
        self.len += 1;
        true
    }

    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let b = self.buf[self.head];
        self.head = (self.head + 1) % BUF_SIZE;
        self.len -= 1;
        Some(b)
    }
}

static INPUT_BUF: SpinLock<RingBuffer> = SpinLock::new(RingBuffer::new());

pub fn poll() {
    // Poll the UART for input and push bytes into the ring buffer.
    #[cfg(any(feature = "qemu", feature = "rpi5"))]
    {
        let mut buf = match INPUT_BUF.try_lock() {
            Some(buf) => buf,
            None => return,
        };
        let mut spins = 0usize;
        loop {
            let mut byte = match uart::read_byte_nonblocking() {
                Some(b) => b,
                None => break,
            };
            if byte == b'\r' {
                byte = b'\n';
            }
            if !buf.push(byte) {
                break;
            }
            spins += 1;
            if spins >= BUF_SIZE {
                break;
            }
        }
    }
}

pub fn read(out: &mut [u8]) -> usize {
    // Read buffered input into the provided slice.
    poll();
    let mut buf = INPUT_BUF.lock();
    let mut count = 0;
    for slot in out.iter_mut() {
        match buf.pop() {
            Some(b) => {
                *slot = b;
                count += 1;
            }
            None => break,
        }
    }
    count
}

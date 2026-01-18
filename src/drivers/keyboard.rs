use crate::util::sync::SpinLock;

#[cfg(feature = "qemu")]
use crate::drivers::uart;

const BUF_SIZE: usize = 256;

struct RingBuffer {
    buf: [u8; BUF_SIZE],
    head: usize,
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
    #[cfg(feature = "qemu")]
    {
        let mut buf = match INPUT_BUF.try_lock() {
            Some(buf) => buf,
            None => return,
        };
        loop {
            let mut byte = match uart::read_byte_nonblocking() {
                Some(b) => b,
                None => break,
            };
            if byte == b'\r' {
                byte = b'\n';
            }
            let _ = buf.push(byte);
        }
    }
}

pub fn read(out: &mut [u8]) -> usize {
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

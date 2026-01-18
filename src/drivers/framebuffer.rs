use core::cell::UnsafeCell;
use core::fmt;
use core::ptr::{copy, write_volatile};

use crate::drivers::mailbox;
use crate::gfx::font;
use crate::util::sync::SpinLock;

const TAG_SET_PHYS_WH: u32 = 0x0004_8003;
const TAG_SET_VIRT_WH: u32 = 0x0004_8004;
const TAG_SET_VIRT_OFFSET: u32 = 0x0004_8009;
const TAG_SET_DEPTH: u32 = 0x0004_8005;
const TAG_SET_PIXEL_ORDER: u32 = 0x0004_8006;
const TAG_ALLOCATE_BUFFER: u32 = 0x0004_0001;
const TAG_GET_PITCH: u32 = 0x0004_0008;

const REQUEST: u32 = 0x0000_0000;

#[repr(C, align(16))]
struct MailboxBuffer {
    buf: UnsafeCell<[u32; 35]>,
}

unsafe impl Sync for MailboxBuffer {}

static MBOX: MailboxBuffer = MailboxBuffer {
    buf: UnsafeCell::new([0; 35]),
};

pub struct Framebuffer {
    ptr: *mut u8,
    width: u32,
    height: u32,
    pitch: u32,
}

unsafe impl Send for Framebuffer {}

#[derive(Copy, Clone)]
pub enum InitError {
    MailboxCallFailed,
    NoFramebuffer,
    NoPitch,
}

struct ConsoleState {
    console: Option<Console>,
}

static CONSOLE: SpinLock<ConsoleState> = SpinLock::new(ConsoleState { console: None });

pub struct Console {
    fb: Framebuffer,
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
    fg: u32,
    bg: u32,
}

impl Framebuffer {
    #[allow(dead_code)]
    pub fn init(width: u32, height: u32) -> Option<Self> {
        Self::init_with_mode(width, height).ok()
    }

    pub fn init_with_mode(width: u32, height: u32) -> Result<Self, InitError> {
        // Use mailbox property tags to allocate and configure the framebuffer.
        unsafe {
            let buf = &mut *MBOX.buf.get();

            buf[0] = (buf.len() * 4) as u32;
            buf[1] = REQUEST;

            buf[2] = TAG_SET_PHYS_WH;
            buf[3] = 8;
            buf[4] = 8;
            buf[5] = width;
            buf[6] = height;

            buf[7] = TAG_SET_VIRT_WH;
            buf[8] = 8;
            buf[9] = 8;
            buf[10] = width;
            buf[11] = height;

            buf[12] = TAG_SET_VIRT_OFFSET;
            buf[13] = 8;
            buf[14] = 8;
            buf[15] = 0;
            buf[16] = 0;

            buf[17] = TAG_SET_DEPTH;
            buf[18] = 4;
            buf[19] = 4;
            buf[20] = 32;

            buf[21] = TAG_SET_PIXEL_ORDER;
            buf[22] = 4;
            buf[23] = 4;
            buf[24] = 1; // RGB

            buf[25] = TAG_ALLOCATE_BUFFER;
            buf[26] = 8;
            buf[27] = 8;
            buf[28] = 4096;
            buf[29] = 0;

            buf[30] = TAG_GET_PITCH;
            buf[31] = 4;
            buf[32] = 4;
            buf[33] = 0;

            buf[34] = 0;

            if !mailbox::call(buf.as_mut_ptr()) {
                return Err(InitError::MailboxCallFailed);
            }

            let fb_bus = buf[28];
            let pitch = buf[33];
            if fb_bus == 0 {
                return Err(InitError::NoFramebuffer);
            }
            if pitch == 0 {
                return Err(InitError::NoPitch);
            }

            let fb_ptr = mailbox::vc_to_arm(fb_bus) as *mut u8;
            let out_width = if buf[10] != 0 { buf[10] } else { width };
            let out_height = if buf[11] != 0 { buf[11] } else { height };

            Ok(Self {
                ptr: fb_ptr,
                width: out_width,
                height: out_height,
                pitch,
            })
        }
    }

    pub fn clear(&mut self, color: u32) {
        // Fill the entire framebuffer with a solid color.
        for y in 0..self.height {
            for x in 0..self.width {
                self.put_pixel(x, y, color);
            }
        }
    }

    fn scroll_rows(&mut self, rows: usize, bg: u32) {
        // Scroll the framebuffer up by the specified number of rows.
        if rows == 0 {
            return;
        }
        let height = self.height as usize;
        if rows >= height {
            self.clear(bg);
            return;
        }

        let bytes_per_row = self.pitch as usize;
        let src = unsafe { self.ptr.add(rows * bytes_per_row) };
        let dst = self.ptr;
        let copy_bytes = (height - rows) * bytes_per_row;
        unsafe {
            copy(src, dst, copy_bytes);
        }

        let start = height - rows;
        for y in start..height {
            let row_ptr = unsafe { self.ptr.add(y * bytes_per_row) as *mut u32 };
            for x in 0..self.width {
                unsafe { write_volatile(row_ptr.add(x as usize), bg) };
            }
        }
    }

    #[allow(dead_code)]
    pub fn write_str(&mut self, mut x: usize, mut y: usize, s: &str, fg: u32, bg: u32) {
        // Render a string at the given pixel position.
        for b in s.bytes() {
            if b == b'\n' {
                y += font::FONT_HEIGHT;
                x = 0;
                continue;
            }
            self.draw_char(x, y, b, fg, bg);
            x += font::FONT_WIDTH;
        }
    }

    fn draw_char(&mut self, x: usize, y: usize, c: u8, fg: u32, bg: u32) {
        let glyph = font::glyph(c);
        for (row, bits) in glyph.iter().enumerate() {
            let y0 = y + row * 2;
            for dy in 0..2 {
                let py = y0 + dy;
                if py >= self.height as usize {
                    continue;
                }
                for col in 0..8 {
                    let px = x + col;
                    if px >= self.width as usize {
                        continue;
                    }
                    let on = (bits >> (7 - col)) & 1 != 0;
                    self.put_pixel(px as u32, py as u32, if on { fg } else { bg });
                }
            }
        }
    }

    fn put_pixel(&mut self, x: u32, y: u32, color: u32) {
        let offset = (y * self.pitch) + (x * 4);
        unsafe {
            write_volatile(self.ptr.add(offset as usize) as *mut u32, color);
        }
    }
}

impl Console {
    fn new(fb: Framebuffer, fg: u32, bg: u32) -> Self {
        let cols = (fb.width as usize) / font::FONT_WIDTH;
        let rows = (fb.height as usize) / font::FONT_HEIGHT;
        Self {
            fb,
            col: 0,
            row: 0,
            cols: cols.max(1),
            rows: rows.max(1),
            fg,
            bg,
        }
    }

    fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= self.rows {
            self.fb.scroll_rows(font::FONT_HEIGHT, self.bg);
            self.row = self.rows - 1;
        }
    }

    fn put_char(&mut self, c: u8) {
        if c == b'\n' {
            self.newline();
            return;
        }
        let x = self.col * font::FONT_WIDTH;
        let y = self.row * font::FONT_HEIGHT;
        self.fb.draw_char(x, y, c, self.fg, self.bg);
        self.col += 1;
        if self.col >= self.cols {
            self.newline();
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        self.put_char(b);
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            self.put_char(b);
        }
        Ok(())
    }
}

#[allow(dead_code)]
pub fn init_console(width: u32, height: u32, fg: u32, bg: u32) -> bool {
    init_console_with_mode(width, height, fg, bg).is_ok()
}

pub fn init_console_with_mode(
    width: u32,
    height: u32,
    fg: u32,
    bg: u32,
) -> Result<(u32, u32), InitError> {
    let mut fb = Framebuffer::init_with_mode(width, height)?;
    let out_width = fb.width;
    let out_height = fb.height;
    fb.clear(bg);
    let mut state = CONSOLE.lock();
    state.console = Some(Console::new(fb, fg, bg));
    Ok((out_width, out_height))
}

#[allow(dead_code)]
pub fn with_console<F: FnOnce(&mut Console)>(f: F) -> bool {
    let mut state = CONSOLE.lock();
    if let Some(console) = state.console.as_mut() {
        f(console);
        true
    } else {
        false
    }
}

pub fn try_with_console<F: FnOnce(&mut Console)>(f: F) -> bool {
    let mut guard = match CONSOLE.try_lock() {
        Some(guard) => guard,
        None => return false,
    };
    if let Some(console) = guard.console.as_mut() {
        f(console);
        true
    } else {
        false
    }
}

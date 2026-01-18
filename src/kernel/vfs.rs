use crate::drivers::framebuffer;
use crate::drivers::keyboard;

pub const FD_STDIN: usize = 0;
pub const FD_STDOUT: usize = 1;
pub const FD_STDERR: usize = 2;

pub const O_READ: u64 = 1 << 0;
pub const O_WRITE: u64 = 1 << 1;
pub const O_APPEND: u64 = 1 << 2;

#[derive(Copy, Clone, Debug)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    #[allow(dead_code)]
    pub append: bool,
}

impl OpenFlags {
    pub const fn new(read: bool, write: bool, append: bool) -> Self {
        Self { read, write, append }
    }

    pub const fn from_bits(bits: u64) -> Self {
        Self {
            read: bits & O_READ != 0,
            write: bits & O_WRITE != 0,
            append: bits & O_APPEND != 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum NodeType {
    Dir,
    DevFb0,
    DevKbd0,
}

#[derive(Copy, Clone, Debug)]
pub enum FileHandle {
    DevFb0,
    DevKbd0,
}

#[derive(Copy, Clone, Debug)]
pub struct FileDesc {
    pub handle: FileHandle,
    pub flags: OpenFlags,
}

pub fn init() {}

pub fn lookup(path: &[u8]) -> Option<NodeType> {
    // Simple path lookup for the fixed in-memory namespace.
    match path {
        b"/" => Some(NodeType::Dir),
        b"/dev" => Some(NodeType::Dir),
        b"/dev/fb0" => Some(NodeType::DevFb0),
        b"/dev/kbd0" => Some(NodeType::DevKbd0),
        _ => None,
    }
}

pub fn open_path(path: &str, flags: OpenFlags) -> Option<FileDesc> {
    // Convenience wrapper for string paths.
    open_bytes(path.as_bytes(), flags)
}

pub fn open_bytes(path: &[u8], flags: OpenFlags) -> Option<FileDesc> {
    // Resolve a path to a device node and create a FileDesc.
    match lookup(path) {
        Some(NodeType::DevFb0) => Some(FileDesc {
            handle: FileHandle::DevFb0,
            flags,
        }),
        Some(NodeType::DevKbd0) => Some(FileDesc {
            handle: FileHandle::DevKbd0,
            flags,
        }),
        _ => None,
    }
}

pub fn write(desc: &FileDesc, buf: &[u8]) -> usize {
    // Write to a device handle (framebuffer or keyboard).
    if !desc.flags.write {
        return 0;
    }
    match desc.handle {
        FileHandle::DevFb0 => {
            let wrote = framebuffer::try_with_console(|console| {
                for &b in buf {
                    console.write_byte(b);
                }
            });
            if wrote {
                buf.len()
            } else {
                0
            }
        }
        FileHandle::DevKbd0 => 0,
    }
}

pub fn read(desc: &FileDesc, buf: &mut [u8]) -> usize {
    // Read from a device handle (keyboard only for now).
    if !desc.flags.read {
        return 0;
    }
    match desc.handle {
        FileHandle::DevFb0 => 0,
        FileHandle::DevKbd0 => keyboard::read(buf),
    }
}

#[allow(dead_code)]
pub fn close(_desc: &FileDesc) {}

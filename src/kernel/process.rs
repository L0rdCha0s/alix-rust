#![allow(dead_code)]

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arch::aarch64::trap::{TrapFrame, TRAP_FRAME_SIZE};
use crate::kernel::smp;
use crate::kernel::vfs::{FileDesc, FD_STDERR, FD_STDOUT};
use core::fmt;
use crate::util::sync::SpinLock;

mod scheduler;
pub use scheduler::{schedule_from_irq, start_on_cpu};

pub type ProcessEntry = extern "C" fn() -> !;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ProcessId(pub u32);

pub const CPU_NONE: usize = usize::MAX;
pub const MAX_FDS: usize = 8;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProcessMode {
    Kernel,
    User,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Copy, Clone, Debug)]
pub struct Process {
    pub id: ProcessId,
    pub name: &'static str,
    pub entry: ProcessEntry,
    pub state: ProcessState,
    pub stack_top: usize,
    pub context_sp: usize,
    pub running_on: usize,
    pub in_run_queue: bool,
    pub mode: ProcessMode,
    pub parent: Option<ProcessId>,
    pub fds: [Option<FileDesc>; MAX_FDS],
}

pub const MAX_PROCS: usize = 64;
const STACK_SIZE: usize = 0x4000;
const INVALID_IDX: usize = usize::MAX;

#[allow(dead_code)]
#[repr(align(16))]
#[derive(Copy, Clone)]
struct ProcStack([u8; STACK_SIZE]);

static mut PROCESS_STACKS: [ProcStack; MAX_PROCS] = [ProcStack([0; STACK_SIZE]); MAX_PROCS];

#[derive(Copy, Clone)]
struct RunQueue {
    slots: [usize; MAX_PROCS],
    head: usize,
    tail: usize,
    len: usize,
}

impl RunQueue {
    const fn new() -> Self {
        Self {
            slots: [INVALID_IDX; MAX_PROCS],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    fn push(&mut self, idx: usize) {
        if self.len >= MAX_PROCS {
            return;
        }
        self.slots[self.tail] = idx;
        self.tail = (self.tail + 1) % MAX_PROCS;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        let idx = self.slots[self.head];
        self.head = (self.head + 1) % MAX_PROCS;
        self.len -= 1;
        Some(idx)
    }
}

#[derive(Copy, Clone)]
struct ProcessTable {
    slots: [Option<Process>; MAX_PROCS],
    next_pid: u32,
    run_queue: RunQueue,
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            slots: [None; MAX_PROCS],
            next_pid: 1,
            run_queue: RunQueue::new(),
        }
    }

    fn alloc_pid(&mut self) -> ProcessId {
        let pid = ProcessId(self.next_pid);
        self.next_pid = self.next_pid.wrapping_add(1).max(1);
        pid
    }
}

static PROCESS_TABLE: SpinLock<ProcessTable> = SpinLock::new(ProcessTable::new());
static CURRENT: [AtomicUsize; smp::MAX_CPUS] = [
    AtomicUsize::new(INVALID_IDX),
    AtomicUsize::new(INVALID_IDX),
    AtomicUsize::new(INVALID_IDX),
    AtomicUsize::new(INVALID_IDX),
];
static INIT_FDS: SpinLock<[Option<FileDesc>; MAX_FDS]> = SpinLock::new([None; MAX_FDS]);

pub fn init() {
    let mut table = PROCESS_TABLE.lock();
    *table = ProcessTable::new();
    for cpu in 0..smp::MAX_CPUS {
        CURRENT[cpu].store(INVALID_IDX, Ordering::Relaxed);
    }
    let mut init_fds = INIT_FDS.lock();
    *init_fds = [None; MAX_FDS];
}

pub fn create(name: &'static str, entry: ProcessEntry, stack_top: usize) -> Option<ProcessId> {
    let parent = current_pid();
    create_with_mode(name, entry, stack_top, ProcessMode::Kernel, parent)
}

pub fn create_user(name: &'static str, entry: ProcessEntry, stack_top: usize) -> Option<ProcessId> {
    let parent = current_pid();
    create_with_mode(name, entry, stack_top, ProcessMode::User, parent)
}

fn create_with_mode(
    name: &'static str,
    entry: ProcessEntry,
    stack_top: usize,
    mode: ProcessMode,
    parent: Option<ProcessId>,
) -> Option<ProcessId> {
    let mut table = PROCESS_TABLE.lock();
    let inherited = if let Some(pid) = parent {
        table
            .slots
            .iter()
            .flatten()
            .find(|p| p.id == pid)
            .map(|p| p.fds)
            .unwrap_or_else(|| *INIT_FDS.lock())
    } else {
        *INIT_FDS.lock()
    };
    for idx in 0..MAX_PROCS {
        if table.slots[idx].is_none() {
            let pid = table.alloc_pid();
            let stack_top = if stack_top == 0 {
                unsafe { PROCESS_STACKS[idx].0.as_ptr().add(STACK_SIZE) as usize }
            } else {
                stack_top
            };
            let context_sp = init_context(entry, stack_top);
            table.slots[idx] = Some(Process {
                id: pid,
                name,
                entry,
                state: ProcessState::Ready,
                stack_top,
                context_sp,
                running_on: CPU_NONE,
                in_run_queue: true,
                mode,
                parent,
                fds: inherited,
            });
            table.run_queue.push(idx);
            return Some(pid);
        }
    }
    None
}

pub fn set_init_fd(fd: usize, desc: Option<FileDesc>) {
    if fd >= MAX_FDS {
        return;
    }
    let mut table = INIT_FDS.lock();
    table[fd] = desc;
}

pub fn set_fd(pid: ProcessId, fd: usize, desc: Option<FileDesc>) -> bool {
    if fd >= MAX_FDS {
        return false;
    }
    let mut table = PROCESS_TABLE.lock();
    for slot in table.slots.iter_mut() {
        if let Some(proc) = slot {
            if proc.id == pid {
                proc.fds[fd] = desc;
                return true;
            }
        }
    }
    false
}

pub fn current_pid() -> Option<ProcessId> {
    let cpu = smp::cpu_id();
    let table = PROCESS_TABLE.lock();
    let idx = CURRENT[cpu].load(Ordering::Relaxed);
    if idx == INVALID_IDX {
        return None;
    }
    table.slots[idx].as_ref().map(|p| p.id)
}

pub fn with_current<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Process) -> R,
{
    let cpu = smp::cpu_id();
    let table = PROCESS_TABLE.lock();
    let idx = CURRENT[cpu].load(Ordering::Relaxed);
    if idx == INVALID_IDX {
        return None;
    }
    table.slots[idx].as_ref().map(f)
}

pub fn with_current_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Process) -> R,
{
    let cpu = smp::cpu_id();
    let mut table = PROCESS_TABLE.lock();
    let idx = CURRENT[cpu].load(Ordering::Relaxed);
    if idx == INVALID_IDX {
        return None;
    }
    table.slots[idx].as_mut().map(f)
}

pub fn alloc_fd_current(desc: FileDesc) -> Option<usize> {
    with_current_mut(|proc| {
        for fd in 0..MAX_FDS {
            if proc.fds[fd].is_none() {
                proc.fds[fd] = Some(desc);
                return Some(fd);
            }
        }
        None
    })?
}

pub fn close_fd_current(fd: usize) -> bool {
    if fd >= MAX_FDS {
        return false;
    }
    with_current_mut(|proc| {
        proc.fds[fd] = None;
        true
    })
    .unwrap_or(false)
}

pub fn get_fd_current(fd: usize) -> Option<FileDesc> {
    if fd >= MAX_FDS {
        return None;
    }
    with_current(|proc| proc.fds[fd])?
}

pub struct FdWriter {
    fd: usize,
}

impl fmt::Write for FdWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = write_current_fd(self.fd, s.as_bytes());
        Ok(())
    }
}

pub fn with_fd_writer<F: FnOnce(&mut FdWriter)>(fd: usize, f: F) {
    let mut writer = FdWriter { fd };
    f(&mut writer);
}

pub fn write_current_fd(fd: usize, buf: &[u8]) -> usize {
    let desc = match get_fd_current(fd) {
        Some(desc) => desc,
        None => return 0,
    };
    crate::kernel::vfs::write(&desc, buf)
}

pub fn write_stdout(buf: &[u8]) -> usize {
    write_current_fd(FD_STDOUT, buf)
}

pub fn write_stderr(buf: &[u8]) -> usize {
    write_current_fd(FD_STDERR, buf)
}

fn init_context(entry: ProcessEntry, stack_top: usize) -> usize {
    let frame_ptr = (stack_top - TRAP_FRAME_SIZE) & !0xF;
    let frame = frame_ptr as *mut TrapFrame;
    unsafe {
        frame.write(TrapFrame::new(entry as usize));
    }
    frame_ptr
}

pub fn get(pid: ProcessId) -> Option<Process> {
    let table = PROCESS_TABLE.lock();
    for slot in table.slots.iter() {
        if let Some(proc) = slot {
            if proc.id == pid {
                return Some(*proc);
            }
        }
    }
    None
}

pub fn set_state(pid: ProcessId, state: ProcessState) -> bool {
    let mut table = PROCESS_TABLE.lock();
    for slot in table.slots.iter_mut() {
        if let Some(proc) = slot {
            if proc.id == pid {
                proc.state = state;
                return true;
            }
        }
    }
    false
}

pub fn for_each(mut f: impl FnMut(&Process)) {
    let table = PROCESS_TABLE.lock();
    for slot in table.slots.iter() {
        if let Some(proc) = slot {
            f(proc);
        }
    }
}

#[allow(dead_code)]
extern "C" {
    fn restore_context(frame: *const TrapFrame) -> !;
}

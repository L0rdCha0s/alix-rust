#![allow(dead_code)]

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::smp;
use crate::sync::SpinLock;
use crate::trap::{TrapFrame, TRAP_FRAME_SIZE};

pub type ProcessEntry = extern "C" fn() -> !;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ProcessId(pub u32);

pub const CPU_ANY: usize = usize::MAX;

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
    pub cpu_affinity: usize,
    pub context_sp: usize,
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
struct ProcessTable {
    slots: [Option<Process>; MAX_PROCS],
    next_pid: u32,
}

impl ProcessTable {
    const fn new() -> Self {
        Self {
            slots: [None; MAX_PROCS],
            next_pid: 1,
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

pub fn init() {
    let mut table = PROCESS_TABLE.lock();
    *table = ProcessTable::new();
    for cpu in 0..smp::MAX_CPUS {
        CURRENT[cpu].store(INVALID_IDX, Ordering::Relaxed);
    }
}

pub fn create(name: &'static str, entry: ProcessEntry, stack_top: usize) -> Option<ProcessId> {
    create_on_cpu(name, entry, stack_top, CPU_ANY)
}

pub fn create_on_cpu(
    name: &'static str,
    entry: ProcessEntry,
    stack_top: usize,
    cpu_affinity: usize,
) -> Option<ProcessId> {
    let mut table = PROCESS_TABLE.lock();
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
                cpu_affinity,
                context_sp,
            });
            return Some(pid);
        }
    }
    None
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

pub fn schedule_from_irq(frame: *mut TrapFrame) -> *mut TrapFrame {
    let cpu = smp::cpu_id();
    let mut table = PROCESS_TABLE.lock();

    let current_idx = CURRENT[cpu].load(Ordering::Relaxed);
    if current_idx != INVALID_IDX {
        if let Some(proc) = &mut table.slots[current_idx] {
            proc.context_sp = frame as usize;
            if proc.state == ProcessState::Running {
                proc.state = ProcessState::Ready;
            }
        }
    }

    let next_idx = find_next_runnable(&table, cpu, current_idx).unwrap_or(current_idx);
    if next_idx == INVALID_IDX {
        return frame;
    }

    if let Some(proc) = &mut table.slots[next_idx] {
        proc.state = ProcessState::Running;
        CURRENT[cpu].store(next_idx, Ordering::Relaxed);
        return proc.context_sp as *mut TrapFrame;
    }

    frame
}

pub fn start_on_cpu(cpu: usize) -> ! {
    let (entry, stack_top) = {
        let mut table = PROCESS_TABLE.lock();
        let next_idx = find_next_runnable(&table, cpu, INVALID_IDX).expect("no runnable process");
        if let Some(proc) = &mut table.slots[next_idx] {
            proc.state = ProcessState::Running;
            CURRENT[cpu].store(next_idx, Ordering::Relaxed);
            (proc.entry, proc.stack_top)
        } else {
            panic!("invalid process index");
        }
    };
    unsafe { start_first(entry, stack_top) }
}

fn find_next_runnable(table: &ProcessTable, cpu: usize, start: usize) -> Option<usize> {
    for offset in 1..=MAX_PROCS {
        let idx = (start.wrapping_add(offset)) % MAX_PROCS;
        if let Some(proc) = &table.slots[idx] {
            let runnable = matches!(proc.state, ProcessState::Ready | ProcessState::Running);
            if runnable && (proc.cpu_affinity == cpu || proc.cpu_affinity == CPU_ANY) {
                return Some(idx);
            }
        }
    }
    None
}

extern "C" {
    fn restore_context(frame: *const TrapFrame) -> !;
    fn start_first(entry: ProcessEntry, stack_top: usize) -> !;
}

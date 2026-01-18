use core::sync::atomic::Ordering;

use crate::arch::aarch64::trap::TrapFrame;
use crate::kernel::smp;

use super::{ProcessState, ProcessTable, CPU_ANY, CURRENT, INVALID_IDX, MAX_PROCS, PROCESS_TABLE, ProcessEntry};

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
    fn start_first(entry: ProcessEntry, stack_top: usize) -> !;
}

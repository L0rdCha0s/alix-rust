use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arch::aarch64::trap::TrapFrame;
use crate::arch::aarch64::mmu;
use crate::drivers::uart;
use crate::kernel::smp;

use super::{
    ProcessEntry, ProcessState, ProcessTable, CPU_NONE, CURRENT, INVALID_IDX, PROCESS_TABLE,
};

const LOG_SCHED: bool = false;
const LOG_EVERY: usize = 50;
static LOG_TICKS: [AtomicUsize; smp::MAX_CPUS] = [
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
];

pub fn schedule_from_irq(frame: *mut TrapFrame) -> *mut TrapFrame {
    // Save the current context and pick the next runnable process.
    let cpu = smp::cpu_id();
    let mut log_data: Option<(usize, u32, &'static str, u32, &'static str, usize)> = None;
    let mut result = frame;
    {
        let mut table = PROCESS_TABLE.lock();

        let current_idx = CURRENT[cpu].load(Ordering::Relaxed);
        if current_idx != INVALID_IDX {
            if let Some(proc) = &mut table.slots[current_idx] {
                proc.context_sp = frame as usize;
                if proc.state == ProcessState::Running {
                    proc.state = ProcessState::Ready;
                    proc.running_on = CPU_NONE;
                    proc.in_run_queue = false;
                }
            }
        }

        // Select the next runnable process from the global queue.
        let next_idx = dequeue_next_runnable(&mut table);
        if next_idx.is_none() {
            if current_idx != INVALID_IDX {
                if let Some(proc) = &mut table.slots[current_idx] {
                    proc.state = ProcessState::Running;
                    proc.running_on = cpu;
                }
            }
            return frame;
        }
        let next_idx = next_idx.unwrap();

        if table.slots[next_idx].is_some() {
            if current_idx != INVALID_IDX {
                if let Some(proc) = &mut table.slots[current_idx] {
                    if proc.state == ProcessState::Ready && !proc.in_run_queue {
                        proc.in_run_queue = true;
                        table.run_queue.push(current_idx);
                    }
                }
            }

            let context_sp = {
                let proc = table.slots[next_idx].as_mut().unwrap();
                proc.state = ProcessState::Running;
                proc.running_on = cpu;
                proc.in_run_queue = false;
                mmu::set_ttbr0(proc.ttbr0);
                proc.context_sp
            };
            CURRENT[cpu].store(next_idx, Ordering::Relaxed);
            result = context_sp as *mut TrapFrame;

            if LOG_SCHED {
                let tick = LOG_TICKS[cpu].fetch_add(1, Ordering::Relaxed);
                if tick % LOG_EVERY == 0 {
                    let (from_id, from_name) = if current_idx != INVALID_IDX {
                        table
                            .slots[current_idx]
                            .as_ref()
                            .map(|p| (p.id.0, p.name))
                            .unwrap_or((0, "none"))
                    } else {
                        (0, "none")
                    };
                    let (to_id, to_name) = table
                        .slots[next_idx]
                        .as_ref()
                        .map(|p| (p.id.0, p.name))
                        .unwrap_or((0, "none"));
                    log_data = Some((cpu, from_id, from_name, to_id, to_name, table.run_queue.len));
                }
            }

            // fallthrough to logging below
        }

    };

    if LOG_SCHED {
        if let Some((cpu, from_id, from_name, to_id, to_name, qlen)) = log_data {
            uart::with_uart(|uart| {
                use core::fmt::Write;
                let _ = writeln!(
                    uart,
                    "sched cpu{} {}({}) -> {}({}) qlen={}",
                    cpu, from_name, from_id, to_name, to_id, qlen
                );
            });
        }
    }

    result
}

pub fn start_on_cpu(cpu: usize) -> ! {
    // Pick the first runnable process and jump directly into it.
    let (entry, stack_top) = {
        let mut table = PROCESS_TABLE.lock();
        let next_idx = dequeue_next_runnable(&mut table).expect("no runnable process");
        if table.slots[next_idx].is_some() {
            let (entry, stack_top) = {
                let proc = table.slots[next_idx].as_mut().unwrap();
                proc.state = ProcessState::Running;
                proc.running_on = cpu;
                proc.in_run_queue = false;
                mmu::set_ttbr0(proc.ttbr0);
                (proc.entry, proc.stack_top)
            };
            CURRENT[cpu].store(next_idx, Ordering::Relaxed);
            (entry, stack_top)
        } else {
            panic!("invalid process index");
        }
    };
    unsafe { start_first(entry, stack_top) }
}

fn dequeue_next_runnable(table: &mut ProcessTable) -> Option<usize> {
    // Round-robin scan of the run queue to find a runnable process.
    let initial_len = table.run_queue.len;
    for _ in 0..initial_len {
        let idx = table.run_queue.pop()?;
        let mut take = false;
        if let Some(proc) = &table.slots[idx] {
            take = proc.state == ProcessState::Ready && proc.running_on == CPU_NONE;
        }
        if take {
            if let Some(proc) = &mut table.slots[idx] {
                proc.in_run_queue = false;
            }
            return Some(idx);
        }
        table.run_queue.push(idx);
    }
    None
}

extern "C" {
    fn start_first(entry: ProcessEntry, stack_top: usize) -> !;
}

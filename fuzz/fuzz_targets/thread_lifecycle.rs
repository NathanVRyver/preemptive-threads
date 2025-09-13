#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use preemptive_threads::thread_new::ThreadBuilder;
use preemptive_threads::mem::StackSizeClass;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

#[derive(Debug, Arbitrary)]
struct ThreadConfig {
    stack_size_class: StackSizeClass,
    priority: u8,
    cpu_affinity: u64,
    enable_guards: bool,
    work_type: WorkType,
    work_amount: u32,
}

#[derive(Debug, Arbitrary)]
enum WorkType {
    CpuIntensive,
    MemoryAllocation,
    Yielding,
    Sleeping,
    Mixed,
}

#[derive(Debug, Arbitrary)]
enum ThreadOperation {
    Spawn(ThreadConfig),
    Join(usize), // Index into spawned threads
    SetPriority { thread_idx: usize, priority: u8 },
    SetAffinity { thread_idx: usize, affinity: u64 },
    GetState(usize),
    Yield,
    Sleep(u32), // milliseconds
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    
    // Generate a sequence of thread operations
    let operations: Result<Vec<ThreadOperation>, _> = (0..100)
        .map(|_| ThreadOperation::arbitrary(&mut u))
        .collect();
    
    if let Ok(ops) = operations {
        fuzz_thread_lifecycle(ops);
    }
});

fn fuzz_thread_lifecycle(operations: Vec<ThreadOperation>) {
    let mut handles = Vec::new();
    let counter = Arc::new(AtomicU64::new(0));
    
    for op in operations {
        match op {
            ThreadOperation::Spawn(config) => {
                let counter_clone = counter.clone();
                
                let mut builder = ThreadBuilder::new()
                    .stack_size_class(config.stack_size_class)
                    .priority(config.priority.clamp(1, 10))
                    .cpu_affinity(config.cpu_affinity);
                
                if config.enable_guards {
                    builder = builder.enable_stack_guards(true);
                }
                
                match builder.spawn(move || {
                    perform_work(config.work_type, config.work_amount, &counter_clone);
                    42
                }) {
                    Ok(handle) => {
                        handles.push(handle);
                        
                        // Limit number of concurrent threads
                        if handles.len() > 50 {
                            if let Some(handle) = handles.remove(0) {
                                let _ = handle.join();
                            }
                        }
                    }
                    Err(_) => {
                        // Spawn failed, continue with other operations
                    }
                }
            }
            
            ThreadOperation::Join(idx) => {
                if idx < handles.len() {
                    let handle = handles.remove(idx);
                    let _ = handle.join();
                }
            }
            
            ThreadOperation::SetPriority { thread_idx, priority } => {
                if thread_idx < handles.len() {
                    handles[thread_idx].thread().set_priority(priority.clamp(1, 10));
                }
            }
            
            ThreadOperation::SetAffinity { thread_idx, affinity } => {
                if thread_idx < handles.len() {
                    handles[thread_idx].thread().set_cpu_affinity(affinity);
                }
            }
            
            ThreadOperation::GetState(idx) => {
                if idx < handles.len() {
                    let _ = handles[idx].thread().state();
                }
            }
            
            ThreadOperation::Yield => {
                preemptive_threads::kernel::yield_now();
            }
            
            ThreadOperation::Sleep(ms) => {
                let duration = std::time::Duration::from_millis((ms % 100) as u64);
                std::thread::sleep(duration);
            }
        }
    }
    
    // Clean up remaining threads
    for handle in handles {
        let _ = handle.join();
    }
}

fn perform_work(work_type: WorkType, amount: u32, counter: &AtomicU64) {
    let amount = amount % 10000; // Limit work to prevent timeouts
    
    match work_type {
        WorkType::CpuIntensive => {
            for i in 0..amount {
                let mut sum = 0u64;
                for j in 0..100 {
                    sum = sum.wrapping_add(i as u64 * j as u64);
                }
                counter.fetch_add(sum % 2, Ordering::Relaxed);
            }
        }
        
        WorkType::MemoryAllocation => {
            for _ in 0..amount.min(1000) {
                let size = (amount % 1024) + 64;
                let vec: Vec<u8> = vec![0; size as usize];
                counter.fetch_add(vec.len() as u64 % 2, Ordering::Relaxed);
            }
        }
        
        WorkType::Yielding => {
            for _ in 0..amount.min(100) {
                counter.fetch_add(1, Ordering::Relaxed);
                preemptive_threads::kernel::yield_now();
            }
        }
        
        WorkType::Sleeping => {
            for _ in 0..amount.min(10) {
                counter.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }
        
        WorkType::Mixed => {
            let work_per_type = amount / 4;
            perform_work(WorkType::CpuIntensive, work_per_type, counter);
            perform_work(WorkType::MemoryAllocation, work_per_type, counter);
            perform_work(WorkType::Yielding, work_per_type, counter);
            perform_work(WorkType::Sleeping, work_per_type, counter);
        }
    }
}

impl Arbitrary for StackSizeClass {
    fn arbitrary(u: &mut Unstructured) -> arbitrary::Result<Self> {
        let choice = u.int_in_range(0..=2)?;
        Ok(match choice {
            0 => StackSizeClass::Small,
            1 => StackSizeClass::Medium,
            _ => StackSizeClass::Large,
        })
    }
}
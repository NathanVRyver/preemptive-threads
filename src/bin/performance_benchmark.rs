extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{scheduler::SCHEDULER, sync::yield_thread};
use std::time::{Duration, Instant};

static mut STACK1: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK2: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK3: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK4: [u8; 64 * 1024] = [0; 64 * 1024];

static mut YIELD_COUNT: u64 = 0;
static mut THREAD_SWITCHES: u64 = 0;

fn benchmark_thread() {
    for _ in 0..10000 {
        unsafe { YIELD_COUNT += 1; }
        yield_thread();
    }
}

fn cpu_intensive_thread() {
    let mut sum = 0u64;
    for i in 0..1_000_000 {
        sum = sum.wrapping_add(i);
        if i % 100_000 == 0 {
            unsafe { THREAD_SWITCHES += 1; }
            yield_thread();
        }
    }
}

fn main() {
    println!("=== Preemptive Multithreading Library Benchmarks ===\n");

    // Benchmark 1: Context switching overhead
    println!("Test 1: Context Switch Performance");
    println!("----------------------------------");
    
    unsafe {
        let scheduler = SCHEDULER.get();
        YIELD_COUNT = 0;
        
        let start = Instant::now();
        
        scheduler.spawn_thread(&mut STACK1, benchmark_thread, 1).unwrap();
        scheduler.spawn_thread(&mut STACK2, benchmark_thread, 1).unwrap();
        
        // Run threads to completion
        while let Some(next_id) = scheduler.schedule() {
            if let Some(thread) = scheduler.get_thread(next_id) {
                if thread.is_runnable() {
                    if let Some(current_id) = scheduler.get_current_thread() {
                        scheduler.set_current_thread(Some(next_id));
                        let _ = scheduler.switch_context(current_id, next_id);
                    } else {
                        // First thread
                        scheduler.set_current_thread(Some(next_id));
                        // Simulate initial context
                        let mut dummy_context = core::mem::MaybeUninit::uninit();
                        scheduler.switch_context(0, next_id).ok();
                    }
                }
            }
        }
        
        let elapsed = start.elapsed();
        let total_yields = YIELD_COUNT;
        
        println!("Total yields: {}", total_yields);
        println!("Total time: {:?}", elapsed);
        println!("Average time per yield: {:?}", elapsed / total_yields as u32);
        println!("Context switches per second: {:.0}", 
                 total_yields as f64 / elapsed.as_secs_f64());
    }

    println!("\nTest 2: CPU-Intensive Workload with Scheduling");
    println!("----------------------------------------------");
    
    unsafe {
        let scheduler = SCHEDULER.get();
        THREAD_SWITCHES = 0;
        
        // Clear scheduler state
        while scheduler.get_current_thread().is_some() {
            scheduler.exit_current_thread();
        }
        
        let start = Instant::now();
        
        scheduler.spawn_thread(&mut STACK3, cpu_intensive_thread, 1).unwrap();
        scheduler.spawn_thread(&mut STACK4, cpu_intensive_thread, 1).unwrap();
        
        // Run threads to completion
        while let Some(next_id) = scheduler.schedule() {
            if let Some(thread) = scheduler.get_thread(next_id) {
                if thread.is_runnable() {
                    if let Some(current_id) = scheduler.get_current_thread() {
                        scheduler.set_current_thread(Some(next_id));
                        let _ = scheduler.switch_context(current_id, next_id);
                    } else {
                        scheduler.set_current_thread(Some(next_id));
                        scheduler.switch_context(0, next_id).ok();
                    }
                }
            }
        }
        
        let elapsed = start.elapsed();
        
        println!("Total thread switches: {}", THREAD_SWITCHES);
        println!("Total time: {:?}", elapsed);
        println!("Throughput: {:.2} operations/second", 
                 2_000_000.0 / elapsed.as_secs_f64());
    }

    println!("\nTest 3: Thread Creation Overhead");
    println!("--------------------------------");
    
    let start = Instant::now();
    let creation_iterations = 1000;
    
    for _ in 0..creation_iterations {
        unsafe {
            let scheduler = SCHEDULER.get();
            let mut stack = vec![0u8; 4096];
            let stack_ptr: &'static mut [u8] = std::mem::transmute(stack.as_mut_slice());
            
            if let Ok(id) = scheduler.spawn_thread(stack_ptr, || {}, 1) {
                // Thread created
                std::mem::forget(stack); // Prevent deallocation
            }
        }
    }
    
    let elapsed = start.elapsed();
    println!("Created {} threads in {:?}", creation_iterations, elapsed);
    println!("Average creation time: {:?}", elapsed / creation_iterations);

    println!("\n=== Benchmark Complete ===");
}
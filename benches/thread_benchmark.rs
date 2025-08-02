extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{scheduler::SCHEDULER, sync::yield_thread};
use std::time::Instant;

static mut STACKS: [[u8; 32 * 1024]; 4] = [[0; 32 * 1024]; 4];
static mut COUNTER: u64 = 0;

fn benchmark_yield() {
    for _ in 0..1000 {
        unsafe {
            COUNTER += 1;
        }
        yield_thread();
    }
}

fn main() {
    println!("\n=== Preemptive Multithreading Rust - Performance Benchmarks ===\n");

    // Benchmark 1: Thread creation time
    println!("Benchmark 1: Thread Creation Performance");
    println!("----------------------------------------");

    let start = Instant::now();
    unsafe {
        let scheduler = SCHEDULER.get();
        for i in 0..4 {
            scheduler.spawn_thread(&mut STACKS[i], || {}, 1).unwrap();
        }
    }
    let creation_time = start.elapsed();
    println!("Created 4 threads in: {:?}", creation_time);
    println!("Average per thread: {:?}", creation_time / 4);

    // Benchmark 2: Context switching
    println!("\nBenchmark 2: Context Switch Performance");
    println!("---------------------------------------");

    unsafe {
        COUNTER = 0;
        let scheduler = SCHEDULER.get();

        // Clear previous threads
        while scheduler.get_current_thread().is_some() {
            scheduler.exit_current_thread();
            scheduler.schedule();
        }

        // Spawn benchmark threads
        scheduler
            .spawn_thread(&mut STACKS[0], benchmark_yield, 1)
            .unwrap();
        scheduler
            .spawn_thread(&mut STACKS[1], benchmark_yield, 1)
            .unwrap();

        let start = Instant::now();

        // Simple scheduler loop
        let mut iterations = 0;
        loop {
            if let Some(_) = scheduler.schedule() {
                iterations += 1;
                yield_thread();
            } else {
                break;
            }

            if iterations > 10000 {
                break; // Safety limit
            }
        }

        let elapsed = start.elapsed();
        let total_yields = unsafe { COUNTER };

        println!("Total yields: {}", total_yields);
        println!("Total time: {:?}", elapsed);
        if total_yields > 0 {
            println!("Average per yield: {:?}", elapsed / total_yields as u32);
            println!(
                "Yields per second: {:.0}",
                total_yields as f64 / elapsed.as_secs_f64()
            );
        }
    }

    // Benchmark 3: Memory footprint
    println!("\nBenchmark 3: Memory Usage");
    println!("-------------------------");
    println!(
        "Thread struct size: {} bytes",
        std::mem::size_of::<preemptive_mlthreading_rust::Thread>()
    );
    println!(
        "Scheduler overhead: ~{} KB",
        (std::mem::size_of::<preemptive_mlthreading_rust::Scheduler>() + 1023) / 1024
    );
    println!("Stack size per thread: {} KB", 32);
    println!(
        "Total for 4 threads: ~{} KB",
        4 * 32 + (std::mem::size_of::<preemptive_mlthreading_rust::Scheduler>() + 1023) / 1024
    );

    println!("\n=== Benchmarks Complete ===\n");
}

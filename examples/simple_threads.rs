extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{scheduler::SCHEDULER, sync::yield_thread};

static mut STACK1: [u8; 32 * 1024] = [0; 32 * 1024];
static mut STACK2: [u8; 32 * 1024] = [0; 32 * 1024];

fn worker_thread_1() {
    for i in 0..5 {
        println!("Thread 1: Iteration {}", i);
        yield_thread();
    }
    println!("Thread 1: Complete");
}

fn worker_thread_2() {
    for i in 0..5 {
        println!("Thread 2: Step {}", i);
        yield_thread();
    }
    println!("Thread 2: Done");
}

fn main() {
    unsafe {
        let scheduler = SCHEDULER.get();
        
        // Spawn two threads with equal priority
        scheduler.spawn_thread(&mut STACK1, worker_thread_1, 1).unwrap();
        scheduler.spawn_thread(&mut STACK2, worker_thread_2, 1).unwrap();
        
        // Simple scheduler loop
        while let Some(next_id) = scheduler.schedule() {
            if let Some(thread) = scheduler.get_thread(next_id) {
                if thread.is_runnable() {
                    scheduler.set_current_thread(Some(next_id));
                    // In a real implementation, you'd switch to this thread
                    // For this example, we'll just yield immediately
                    yield_thread();
                }
            }
        }
    }
    
    println!("All threads completed!");
}
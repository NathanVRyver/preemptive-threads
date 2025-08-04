//! Example demonstrating the platform timer API

use preemptive_threads::{
    init_preemption_timer, stop_preemption_timer, preemption_checkpoint,
    get_preemption_count, ThreadBuilder, ATOMIC_SCHEDULER,
};

fn main() {
    println!("=== Platform Timer Demo ===\n");
    
    // Example 1: Try to initialize preemption timer
    println!("1. Attempting to initialize preemption timer...");
    match init_preemption_timer(100) { // 100ms interval
        Ok(()) => {
            println!("   ✓ Preemption timer initialized successfully");
            
            // In a real application, you would do work here and
            // the timer would periodically interrupt execution
            for i in 0..10 {
                println!("   Working... iteration {}", i);
                
                // Insert preemption checkpoints in long-running code
                preemption_checkpoint();
                
                // Simulate some work
                for _ in 0..1000000 {
                    core::hint::black_box(i * 2);
                }
            }
            
            println!("   Preemption count: {}", get_preemption_count());
            
            // Clean up
            stop_preemption_timer();
            println!("   ✓ Preemption timer stopped");
        }
        Err(e) => {
            println!("   ⚠ Preemption timer not available: {}", e);
            println!("   Using cooperative scheduling instead...");
            demonstrate_cooperative_scheduling();
        }
    }
}

fn demonstrate_cooperative_scheduling() {
    println!("\n2. Demonstrating cooperative scheduling:");
    
    // Create some threads manually
    let stack1 = Box::leak(Box::new([0u8; 32768]));  
    let stack2 = Box::leak(Box::new([0u8; 32768]));
    
    // Spawn threads with different priorities
    match ATOMIC_SCHEDULER.spawn_thread(stack1, worker_task_1, 7) {
        Ok(id) => println!("   ✓ Spawned high priority thread {}", id),
        Err(e) => println!("   ✗ Failed to spawn thread: {:?}", e),
    }
    
    match ATOMIC_SCHEDULER.spawn_thread(stack2, worker_task_2, 3) {
        Ok(id) => println!("   ✓ Spawned low priority thread {}", id),  
        Err(e) => println!("   ✗ Failed to spawn thread: {:?}", e),
    }
    
    // Demonstrate cooperative scheduling
    println!("   Running cooperative scheduler:");
    for i in 0..5 {
        if let Some(thread_id) = ATOMIC_SCHEDULER.schedule() {
            println!("   Iteration {}: Would run thread {}", i, thread_id);
        } else {
            println!("   Iteration {}: No threads to schedule", i);
        }
        
        // Yield control cooperatively
        preemption_checkpoint();
    }
    
    println!("\n   In a real application, you would:");
    println!("   - Call preemption_checkpoint() regularly in long loops");
    println!("   - Use yield_now() to explicitly yield control");
    println!("   - Insert preemption_point!() macros in CPU-intensive code");
}

fn worker_task_1() {
    println!("      [HIGH PRIORITY] Task 1 would run here");
    // In real code, insert preemption_checkpoint() calls:
    // for i in 0..1000000 {
    //     do_work(i);
    //     if i % 10000 == 0 {
    //         preemption_checkpoint();
    //     }
    // }
}

fn worker_task_2() {
    println!("      [LOW PRIORITY] Task 2 would run here");
    // Regular preemption checkpoints allow higher priority threads to run
}
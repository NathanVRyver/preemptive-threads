//! Example demonstrating the safe threading API introduced in v0.1.2

use core::sync::atomic::{AtomicU32, Ordering};
use preemptive_threads::{
    protected_stack, yield_now, Mutex, StackStatus, ThreadBuilder, ATOMIC_SCHEDULER,
};

static COUNTER: Mutex<u32> = Mutex::new(0);
static ATOMIC_COUNTER: AtomicU32 = AtomicU32::new(0);

fn main() {
    println!("=== Safe Threading API Demo (v0.1.2) ===\n");

    // Example 1: Using the atomic scheduler with priority queues
    println!("1. Atomic Scheduler with Lock-Free Priority Queues:");
    demo_atomic_scheduler();

    // Example 2: Protected stacks with guard pages
    println!("\n2. Protected Stack with Guard Pages:");
    demo_protected_stack();

    // Example 3: Safe mutex implementation
    println!("\n3. Safe Mutex Usage:");
    demo_mutex();

    // Example 4: Thread builder (safe API)
    println!("\n4. Thread Builder API:");
    demo_thread_builder();
}

fn demo_atomic_scheduler() {
    // Create protected stacks
    let stack1 = Box::leak(Box::new([0u8; 65536]));
    let stack2 = Box::leak(Box::new([0u8; 65536]));
    let stack3 = Box::leak(Box::new([0u8; 65536]));

    // High priority thread
    ATOMIC_SCHEDULER
        .spawn_thread(stack1, high_priority_task, 7)
        .expect("Failed to spawn high priority thread");

    // Medium priority thread
    ATOMIC_SCHEDULER
        .spawn_thread(stack2, medium_priority_task, 4)
        .expect("Failed to spawn medium priority thread");

    // Low priority thread
    ATOMIC_SCHEDULER
        .spawn_thread(stack3, low_priority_task, 1)
        .expect("Failed to spawn low priority thread");

    // Simulate scheduling - in a real system this would be driven by interrupts
    println!("Scheduling order (higher priority threads run first):");
    for i in 0..10 {
        if let Some(thread_id) = ATOMIC_SCHEDULER.schedule() {
            println!("  Iteration {}: Scheduled thread {} ", i, thread_id);
        }
    }
}

fn high_priority_task() {
    println!("  [HIGH PRIORITY] Task executing");
    ATOMIC_COUNTER.fetch_add(100, Ordering::Relaxed);
}

fn medium_priority_task() {
    println!("  [MEDIUM PRIORITY] Task executing");
    ATOMIC_COUNTER.fetch_add(10, Ordering::Relaxed);
}

fn low_priority_task() {
    println!("  [LOW PRIORITY] Task executing");
    ATOMIC_COUNTER.fetch_add(1, Ordering::Relaxed);
}

fn demo_protected_stack() {
    // Create a small stack to demonstrate protection
    static mut SMALL_STACK: [u8; 8192] = [0; 8192];

    unsafe {
        let protected = preemptive_threads::ProtectedStack::new(
            &mut SMALL_STACK,
            preemptive_threads::StackGuard::default(),
        );

        // Check stack status
        match protected.check_overflow() {
            StackStatus::Ok {
                used_bytes,
                free_bytes,
            } => {
                println!(
                    "  Stack OK - Used: {} bytes, Free: {} bytes",
                    used_bytes, free_bytes
                );
            }
            StackStatus::NearOverflow { bytes_remaining } => {
                println!(
                    "  WARNING: Near overflow! Only {} bytes remaining",
                    bytes_remaining
                );
            }
            StackStatus::Overflow { overflow_bytes } => {
                println!("  ERROR: Stack overflow by {} bytes!", overflow_bytes);
            }
            StackStatus::Corrupted {
                corrupted_bytes, ..
            } => {
                println!(
                    "  ERROR: Stack corrupted! {} bytes corrupted",
                    corrupted_bytes
                );
            }
        }

        // Get detailed statistics
        let stats = protected.get_stats();
        println!("  Stack Statistics:");
        println!("    Total size: {} bytes", stats.total_size);
        println!("    Usable size: {} bytes", stats.usable_size);
        println!("    Guard size: {} bytes", stats.guard_size);
        println!("    Current usage: {} bytes", stats.current_usage);
        println!("    Peak usage: {} bytes", stats.peak_usage);
    }
}

fn demo_mutex() {
    // Lock the mutex and increment
    {
        let mut guard = COUNTER.lock();
        *guard += 1;
        println!("  Mutex locked, counter incremented to: {}", *guard);

        // Try to lock again (should fail)
        if COUNTER.try_lock().is_none() {
            println!("  try_lock() correctly failed while mutex is held");
        }
    } // Lock automatically released here

    // Now try_lock should succeed
    if let Some(guard) = COUNTER.try_lock() {
        println!(
            "  try_lock() succeeded after release, counter value: {}",
            *guard
        );
    }
}

fn demo_thread_builder() {
    // Create a thread with builder pattern
    let builder = ThreadBuilder::new()
        .stack_size(128 * 1024) // 128KB stack
        .priority(6) // High priority
        .name("worker_thread");

    println!("  Created thread builder with:");
    println!("    Stack size: 128KB");
    println!("    Priority: 6/7");
    println!("    Name: worker_thread");

    // Note: spawn() returns NotImplemented in no_std environment
    match builder.spawn(|| {
        println!("This would run in the thread");
    }) {
        Err(preemptive_threads::ThreadError::NotImplemented) => {
            println!("  (Thread spawning not implemented in no_std demo)");
        }
        _ => {}
    }
}

// Compile with: rustc --edition 2021 examples/safe_threading.rs -L target/release/deps --extern preemptive_threads

//! Basic multithreading example demonstrating thread creation and synchronization.

#![no_std]

extern crate alloc;
use alloc::{vec, vec::Vec, boxed::Box};

use preemptive_mlthreading_rust::{
    ThreadBuilder, JoinHandle, yield_now, Mutex, 
    init_security, SecurityConfig, get_security_stats
};

/// Shared counter protected by mutex
static COUNTER: Mutex<u32> = Mutex::new(0);

/// Simple worker function that increments the shared counter
fn worker_thread(thread_id: u32, iterations: u32) -> u32 {
    let mut local_sum = 0;
    
    for i in 0..iterations {
        // Access shared resource safely
        {
            let mut counter = COUNTER.lock();
            *counter += 1;
            local_sum = *counter;
        }
        
        // Yield control to other threads periodically
        if i % 10 == 0 {
            yield_now();
        }
    }
    
    println!("Worker {} completed {} iterations, final counter: {}", 
             thread_id, iterations, local_sum);
    local_sum
}

/// CPU-intensive computation to demonstrate CPU affinity
fn compute_worker(thread_id: u32) -> u64 {
    let mut result = 0u64;
    
    for i in 0..1000 {
        // Simulate work
        for j in 0..100 {
            result = result.wrapping_add((i * j) as u64);
        }
        
        // Periodic yield
        if i % 50 == 0 {
            yield_now();
        }
    }
    
    println!("Compute worker {} finished with result: {}", thread_id, result);
    result
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    println!("=== Basic Threading Example ===");
    
    // Initialize security subsystem
    let security_config = SecurityConfig {
        enable_stack_canaries: true,
        enable_guard_pages: false, // Disabled for this example
        enable_cfi: false,
        enable_thread_isolation: false,
        enable_aslr: false,
        enable_audit_logging: true,
        use_secure_rng: true,
        panic_on_violation: true,
    };
    
    init_security(security_config)?;
    println!("Security initialized");
    
    // Create a vector to store thread handles
    let mut handles: Vec<JoinHandle<u32>> = vec![];
    
    // Spawn worker threads with different configurations
    for i in 0..4 {
        let handle = ThreadBuilder::new()
            .name(&format!("worker_{}", i))
            .stack_size(64 * 1024) // 64KB stack
            .priority(5 + (i % 3) as u8) // Varying priorities 5-7
            .spawn(move || worker_thread(i, 50))
            .expect("Failed to spawn worker thread");
        
        handles.push(handle);
    }
    
    // Spawn compute-intensive threads with CPU affinity
    let mut compute_handles: Vec<JoinHandle<u64>> = vec![];
    
    for i in 0..2 {
        let affinity_mask = 1u64 << i; // Pin to specific CPU
        let handle = ThreadBuilder::new()
            .name(&format!("compute_{}", i))
            .stack_size(128 * 1024) // Larger stack for compute work
            .priority(8) // High priority
            .cpu_affinity(affinity_mask)
            .spawn(move || compute_worker(i))
            .expect("Failed to spawn compute thread");
        
        compute_handles.push(handle);
    }
    
    println!("Spawned {} worker threads and {} compute threads", 
             handles.len(), compute_handles.len());
    
    // Wait for all worker threads to complete
    let mut worker_results = vec![];
    for handle in handles {
        let result = handle.join()?;
        worker_results.push(result);
    }
    
    // Wait for compute threads
    let mut compute_results = vec![];
    for handle in compute_handles {
        let result = handle.join()?;
        compute_results.push(result);
    }
    
    // Display results
    println!("\n=== Results ===");
    println!("Worker thread results: {:?}", worker_results);
    println!("Compute thread results: {:?}", compute_results);
    
    // Final counter value
    {
        let final_counter = COUNTER.lock();
        println!("Final shared counter value: {}", *final_counter);
    }
    
    // Security statistics
    let stats = get_security_stats();
    println!("\n=== Security Stats ===");
    println!("Total violations: {}", stats.total_violations);
    println!("Stack violations: {}", stats.stack_violations);
    println!("Features enabled: {:?}", stats.features_enabled);
    
    println!("\n=== Threading Example Completed Successfully ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_worker_thread() {
        let result = worker_thread(1, 10);
        assert!(result > 0);
    }
    
    #[test] 
    fn test_compute_worker() {
        let result = compute_worker(1);
        assert!(result > 0);
    }
}
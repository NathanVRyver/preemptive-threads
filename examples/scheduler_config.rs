//! Advanced scheduler configuration and custom scheduling policies.

#![no_std]

extern crate alloc;
use alloc::{vec, vec::Vec};

use preemptive_mlthreading_rust::{
    ThreadBuilder, JoinHandle, yield_now, Duration,
    NewScheduler, RoundRobinScheduler, CpuId,
    ThreadMetrics, SystemMetrics, ThreadProfiler
};

#[cfg(feature = "work-stealing")]
use preemptive_mlthreading_rust::WorkStealingScheduler;

/// High-priority real-time task
fn realtime_task(task_id: u32) -> u32 {
    println!("RT Task {} started", task_id);
    
    for i in 0..20 {
        // Simulate real-time work with tight timing constraints
        busy_wait_microseconds(100); // 100μs of work
        
        if i % 5 == 0 {
            yield_now(); // Allow preemption at controlled points
        }
    }
    
    println!("RT Task {} completed", task_id);
    task_id
}

/// Background computation task
fn background_task(task_id: u32) -> u64 {
    println!("Background Task {} started", task_id);
    
    let mut result = 0u64;
    for i in 0..1000 {
        // CPU-intensive work
        for j in 0..50 {
            result = result.wrapping_add((i * j) as u64);
        }
        
        // More frequent yielding for background tasks
        if i % 10 == 0 {
            yield_now();
        }
    }
    
    println!("Background Task {} completed with result: {}", task_id, result);
    result
}

/// Interactive task with variable load
fn interactive_task(task_id: u32) -> u32 {
    println!("Interactive Task {} started", task_id);
    
    let mut responses = 0;
    
    for i in 0..50 {
        // Simulate user interaction processing
        if i % 7 == 0 {
            // Burst of activity
            busy_wait_microseconds(200);
            responses += 1;
        } else {
            // Light processing
            busy_wait_microseconds(50);
        }
        
        yield_now(); // Interactive tasks yield frequently
    }
    
    println!("Interactive Task {} processed {} responses", task_id, responses);
    responses
}

/// Simulate microsecond-level busy waiting
fn busy_wait_microseconds(us: u32) {
    for _ in 0..(us * 10) {
        // Simple busy loop - in real code you'd use actual timing
        core::hint::black_box(());
    }
}

fn demonstrate_round_robin_scheduler() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Round Robin Scheduler Demo ===");
    
    // Configure round-robin scheduler
    let mut scheduler = RoundRobinScheduler::new();
    scheduler.set_time_slice(Duration::from_millis(10));
    
    let mut handles = vec![];
    
    // Create threads with equal priority (round-robin behavior)
    for i in 0..3 {
        let handle = ThreadBuilder::new()
            .name(&format!("rr_task_{}", i))
            .priority(10) // Same priority for all
            .scheduler(Box::new(scheduler.clone()))
            .spawn(move || background_task(i))
            .expect("Failed to spawn round-robin task");
        
        handles.push(handle);
    }
    
    // Wait for completion and collect results
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("Round-robin scheduling completed");
    Ok(())
}

#[cfg(feature = "work-stealing")]
fn demonstrate_work_stealing_scheduler() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Work Stealing Scheduler Demo ===");
    
    // Configure work-stealing scheduler with NUMA awareness
    let scheduler = WorkStealingScheduler::new()
        .with_numa_awareness(true)
        .with_cache_optimization(true);
    
    let mut handles = vec![];
    
    // Create compute-intensive tasks that benefit from work stealing
    for i in 0..6 {
        let cpu_affinity = if i < 3 { 0x0F } else { 0xF0 }; // Split across NUMA nodes
        
        let handle = ThreadBuilder::new()
            .name(&format!("ws_task_{}", i))
            .priority(12)
            .cpu_affinity(cpu_affinity)
            .scheduler(Box::new(scheduler.clone()))
            .spawn(move || {
                let mut result = 0u64;
                // Irregular work load that benefits from work stealing
                for j in 0..(100 * (i + 1)) {
                    result = result.wrapping_add(fibonacci(j % 20));
                    if j % 50 == 0 {
                        yield_now();
                    }
                }
                result
            })
            .expect("Failed to spawn work-stealing task");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("Work-stealing scheduling completed");
    Ok(())
}

/// Simple fibonacci function for variable workload
fn fibonacci(n: u32) -> u64 {
    if n < 2 {
        n as u64
    } else {
        let mut a = 0u64;
        let mut b = 1u64;
        for _ in 2..=n {
            let temp = a + b;
            a = b;
            b = temp;
        }
        b
    }
}

fn demonstrate_priority_scheduling() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Priority Scheduling Demo ===");
    
    let mut handles = vec![];
    
    // High priority real-time tasks
    for i in 0..2 {
        let handle = ThreadBuilder::new()
            .name(&format!("rt_task_{}", i))
            .priority(20) // Highest priority
            .spawn(move || realtime_task(i))
            .expect("Failed to spawn real-time task");
        
        handles.push(handle);
    }
    
    // Medium priority interactive tasks  
    for i in 0..3 {
        let handle = ThreadBuilder::new()
            .name(&format!("ui_task_{}", i))
            .priority(15) // Medium priority
            .spawn(move || interactive_task(i))
            .expect("Failed to spawn interactive task");
        
        handles.push(handle);
    }
    
    // Low priority background tasks
    for i in 0..2 {
        let handle = ThreadBuilder::new()
            .name(&format!("bg_task_{}", i))
            .priority(5) // Low priority
            .spawn(move || background_task(i))
            .expect("Failed to spawn background task");
        
        handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("Priority scheduling completed");
    Ok(())
}

fn demonstrate_cpu_affinity() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== CPU Affinity Demo ===");
    
    let mut handles = vec![];
    
    // Create threads pinned to specific CPUs
    for cpu_id in 0..4u32 {
        let affinity_mask = 1u64 << cpu_id;
        
        let handle = ThreadBuilder::new()
            .name(&format!("cpu_{}_task", cpu_id))
            .priority(10)
            .cpu_affinity(affinity_mask)
            .spawn(move || {
                println!("Task running on CPU {} (mask: 0x{:x})", cpu_id, affinity_mask);
                
                // CPU-specific work
                let mut result = 0u64;
                for i in 0..500 {
                    result = result.wrapping_add((cpu_id as u64 * i as u64));
                    if i % 100 == 0 {
                        yield_now();
                    }
                }
                
                println!("CPU {} task completed with result: {}", cpu_id, result);
                result
            })
            .expect("Failed to spawn CPU-affine task");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("CPU affinity demonstration completed");
    Ok(())
}

fn demonstrate_performance_monitoring() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Performance Monitoring Demo ===");
    
    // Create profiler for performance analysis
    let profiler = ThreadProfiler::new();
    profiler.enable_context_switch_tracking();
    profiler.enable_cpu_usage_tracking();
    
    let mut handles = vec![];
    
    // Create monitored tasks
    for i in 0..3 {
        let handle = ThreadBuilder::new()
            .name(&format!("monitored_task_{}", i))
            .priority(12)
            .enable_profiling(true)
            .spawn(move || {
                // Mixed workload for interesting metrics
                for j in 0..200 {
                    if j % 3 == 0 {
                        // CPU-intensive phase
                        busy_wait_microseconds(1000);
                    } else {
                        // I/O or wait simulation
                        yield_now();
                        busy_wait_microseconds(100);
                    }
                }
                
                i * 1000
            })
            .expect("Failed to spawn monitored task");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let thread_id = handle.thread().id();
        let _result = handle.join()?;
        
        // Get performance metrics for this thread
        if let Ok(metrics) = ThreadMetrics::for_thread(thread_id) {
            println!("Thread {} metrics:", thread_id);
            println!("  CPU time: {}ms", metrics.cpu_time_ms);
            println!("  Context switches: {}", metrics.context_switches);
            println!("  Preemptions: {}", metrics.preemptions);
            println!("  Average latency: {}μs", metrics.avg_latency_us);
        }
    }
    
    // System-wide metrics
    let system_metrics = SystemMetrics::current();
    println!("\nSystem Metrics:");
    println!("  Total threads: {}", system_metrics.active_threads);
    println!("  Total context switches: {}", system_metrics.total_context_switches);
    println!("  CPU utilization: {:.1}%", system_metrics.cpu_utilization * 100.0);
    println!("  Memory usage: {}KB", system_metrics.memory_usage_kb);
    
    println!("Performance monitoring completed");
    Ok(())
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    println!("=== Advanced Scheduler Configuration Example ===");
    
    // Demonstrate different scheduling approaches
    demonstrate_round_robin_scheduler()?;
    
    #[cfg(feature = "work-stealing")]
    demonstrate_work_stealing_scheduler()?;
    
    demonstrate_priority_scheduling()?;
    demonstrate_cpu_affinity()?;
    demonstrate_performance_monitoring()?;
    
    println!("\n=== All Scheduler Demonstrations Completed Successfully ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_realtime_task() {
        let result = realtime_task(1);
        assert_eq!(result, 1);
    }
    
    #[test]
    fn test_background_task() {
        let result = background_task(1);
        assert!(result > 0);
    }
    
    #[test]
    fn test_interactive_task() {
        let result = interactive_task(1);
        assert!(result > 0);
    }
    
    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(10), 55);
    }
}
//! Performance testing and benchmarking example demonstrating optimization features.

#![no_std]

extern crate alloc;
use alloc::{vec, vec::Vec, format};

use preemptive_mlthreading_rust::{
    ThreadBuilder, JoinHandle, yield_now, Duration,
    ThreadMetrics, SystemMetrics, ThreadProfiler, HealthMonitor,
    // Performance modules
    perf::{
        context_switch_opt::{init_context_switch_optimization, get_context_switch_stats},
        fast_paths::{FastPaths, FastPathMetrics},
        cache_aware::{init_cache_optimization, get_cache_stats},
        numa::{init_numa_optimization, get_numa_stats},
        cpu_dispatch::{init_cpu_optimization, get_cpu_features},
        memory_pools::{init_memory_pools, get_pool_stats},
    }
};

/// CPU-intensive benchmark task
fn cpu_benchmark_task(task_id: u32, iterations: u32) -> (u32, u64) {
    let start_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    
    let mut result = 0u64;
    for i in 0..iterations {
        // Mix of different CPU operations
        result = result.wrapping_add(fibonacci_fast(i % 25) as u64);
        result = result.wrapping_mul(1664525u64).wrapping_add(1013904223u64); // LCG
        result ^= result << 13;
        result ^= result >> 7;
        result ^= result << 17;
        
        // Periodic yield for fairness
        if i % 1000 == 0 {
            yield_now();
        }
    }
    
    let end_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    let elapsed_ms = end_time.duration_since(start_time).as_millis() as u32;
    
    println!("CPU benchmark {} completed: {}ms for {} iterations", 
             task_id, elapsed_ms, iterations);
    
    (elapsed_ms, result)
}

/// Memory-intensive benchmark task
fn memory_benchmark_task(task_id: u32, size_mb: u32) -> (u32, u64) {
    let start_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    
    let size_bytes = (size_mb * 1024 * 1024) as usize;
    let mut data = vec![0u8; size_bytes];
    
    // Fill with pattern
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i ^ (i >> 8) ^ task_id as usize) as u8;
    }
    
    // Read back and compute checksum
    let mut checksum = 0u64;
    for (i, &byte) in data.iter().enumerate() {
        checksum = checksum.wrapping_add(byte as u64);
        checksum = checksum.wrapping_mul(31);
        
        if i % 100000 == 0 {
            yield_now();
        }
    }
    
    let end_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    let elapsed_ms = end_time.duration_since(start_time).as_millis() as u32;
    
    println!("Memory benchmark {} completed: {}ms for {}MB", 
             task_id, elapsed_ms, size_mb);
    
    (elapsed_ms, checksum)
}

/// Lock contention benchmark
fn lock_contention_task(task_id: u32, lock: &preemptive_mlthreading_rust::Mutex<u64>) -> u32 {
    let start_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    
    let mut acquisitions = 0u32;
    
    for i in 0..1000 {
        // Try to acquire lock and do work
        {
            let mut counter = lock.lock();
            *counter += task_id as u64 * i as u64;
            acquisitions += 1;
            
            // Hold lock for variable time
            busy_wait_cycles(100 + (i % 50));
        }
        
        // Small delay between acquisitions
        busy_wait_cycles(50);
        
        if i % 100 == 0 {
            yield_now();
        }
    }
    
    let end_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    let elapsed_ms = end_time.duration_since(start_time).as_millis() as u32;
    
    println!("Lock contention task {} completed: {}ms, {} acquisitions", 
             task_id, elapsed_ms, acquisitions);
    
    acquisitions
}

/// Context switch intensive benchmark
fn context_switch_benchmark(task_id: u32) -> u32 {
    let start_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    
    let mut switches = 0u32;
    
    for i in 0..2000 {
        // Minimal work then yield
        busy_wait_cycles(10);
        yield_now();
        switches += 1;
        
        if i % 200 == 0 {
            // Occasionally do slightly more work
            busy_wait_cycles(100);
        }
    }
    
    let end_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    let elapsed_ms = end_time.duration_since(start_time).as_millis() as u32;
    
    println!("Context switch benchmark {} completed: {}ms, {} switches", 
             task_id, elapsed_ms, switches);
    
    switches
}

/// Fast path operations benchmark
fn fast_path_benchmark(task_id: u32) -> u32 {
    let start_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    
    let mut operations = 0u32;
    
    for i in 0..10000 {
        // Use fast path operations
        let thread_id = FastPaths::fast_current_thread_id();
        operations += 1;
        
        // Fast atomic operations
        let counter = preemptive_mlthreading_rust::portable_atomic::AtomicU64::new(i as u64);
        let _result = FastPaths::fast_atomic_increment(&counter);
        operations += 1;
        
        if i % 1000 == 0 {
            yield_now();
        }
    }
    
    let end_time = preemptive_mlthreading_rust::time::get_monotonic_time();
    let elapsed_ms = end_time.duration_since(start_time).as_millis() as u32;
    
    println!("Fast path benchmark {} completed: {}ms, {} operations", 
             task_id, elapsed_ms, operations);
    
    operations
}

/// Optimized fibonacci using iteration
fn fibonacci_fast(n: u32) -> u32 {
    if n < 2 { return n; }
    
    let mut a = 0u32;
    let mut b = 1u32;
    
    for _ in 2..=n {
        let temp = a.wrapping_add(b);
        a = b;
        b = temp;
    }
    
    b
}

/// Busy wait for specified cycles
fn busy_wait_cycles(cycles: u32) {
    for _ in 0..cycles {
        core::hint::black_box(());
    }
}

fn run_cpu_benchmarks() -> Result<Vec<(u32, u64)>, preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== CPU Benchmarks ===");
    
    let mut handles = vec![];
    
    // Different CPU load levels
    let test_configs = [
        (10000, "Light"),
        (50000, "Medium"), 
        (100000, "Heavy"),
        (200000, "Extreme"),
    ];
    
    for (i, &(iterations, label)) in test_configs.iter().enumerate() {
        let handle = ThreadBuilder::new()
            .name(&format!("cpu_bench_{}", i))
            .priority(10 + (i % 4) as u8)
            .cpu_affinity(1u64 << (i % 4))
            .spawn(move || {
                println!("Starting {} CPU benchmark with {} iterations", label, iterations);
                cpu_benchmark_task(i as u32, iterations)
            })
            .expect("Failed to spawn CPU benchmark");
        
        handles.push(handle);
    }
    
    let mut results = vec![];
    for handle in handles {
        let result = handle.join()?;
        results.push(result);
    }
    
    // Calculate statistics
    let total_time: u32 = results.iter().map(|(time, _)| *time).sum();
    let avg_time = total_time / results.len() as u32;
    
    println!("CPU Benchmark Results:");
    for (i, &(time, result)) in results.iter().enumerate() {
        println!("  Test {}: {}ms (result: 0x{:x})", i, time, result);
    }
    println!("  Average time: {}ms", avg_time);
    
    Ok(results)
}

fn run_memory_benchmarks() -> Result<Vec<(u32, u64)>, preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Memory Benchmarks ===");
    
    let mut handles = vec![];
    
    // Different memory sizes
    let memory_sizes = [1, 4, 16, 32]; // MB
    
    for (i, &size_mb) in memory_sizes.iter().enumerate() {
        let handle = ThreadBuilder::new()
            .name(&format!("mem_bench_{}", i))
            .priority(10)
            .stack_size(256 * 1024) // Larger stack for memory work
            .spawn(move || {
                println!("Starting memory benchmark with {}MB", size_mb);
                memory_benchmark_task(i as u32, size_mb)
            })
            .expect("Failed to spawn memory benchmark");
        
        handles.push(handle);
    }
    
    let mut results = vec![];
    for handle in handles {
        let result = handle.join()?;
        results.push(result);
    }
    
    println!("Memory Benchmark Results:");
    for (i, &(time, checksum)) in results.iter().enumerate() {
        let mb_per_sec = (memory_sizes[i] * 1000) / time.max(1);
        println!("  {}MB: {}ms ({} MB/s, checksum: 0x{:x})", 
                 memory_sizes[i], time, mb_per_sec, checksum);
    }
    
    Ok(results)
}

fn run_lock_contention_benchmark() -> Result<Vec<u32>, preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Lock Contention Benchmark ===");
    
    use preemptive_mlthreading_rust::Mutex;
    static SHARED_COUNTER: Mutex<u64> = Mutex::new(0);
    
    let mut handles = vec![];
    
    // Multiple threads contending for the same lock
    for i in 0..8 {
        let handle = ThreadBuilder::new()
            .name(&format!("lock_bench_{}", i))
            .priority(10)
            .cpu_affinity(1u64 << (i % 4))
            .spawn(move || lock_contention_task(i, &SHARED_COUNTER))
            .expect("Failed to spawn lock benchmark");
        
        handles.push(handle);
    }
    
    let mut results = vec![];
    for handle in handles {
        let acquisitions = handle.join()?;
        results.push(acquisitions);
    }
    
    let final_counter = {
        let counter = SHARED_COUNTER.lock();
        *counter
    };
    
    println!("Lock Contention Results:");
    println!("  Final counter value: {}", final_counter);
    for (i, &acquisitions) in results.iter().enumerate() {
        println!("  Thread {}: {} acquisitions", i, acquisitions);
    }
    
    Ok(results)
}

fn run_context_switch_benchmark() -> Result<Vec<u32>, preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Context Switch Benchmark ===");
    
    let mut handles = vec![];
    
    // Many threads for maximum context switching
    for i in 0..16 {
        let handle = ThreadBuilder::new()
            .name(&format!("ctx_bench_{}", i))
            .priority(8 + (i % 8) as u8)
            .spawn(move || context_switch_benchmark(i))
            .expect("Failed to spawn context switch benchmark");
        
        handles.push(handle);
    }
    
    let mut results = vec![];
    for handle in handles {
        let switches = handle.join()?;
        results.push(switches);
    }
    
    let total_switches: u32 = results.iter().sum();
    
    println!("Context Switch Results:");
    println!("  Total context switches: {}", total_switches);
    println!("  Average per thread: {}", total_switches / results.len() as u32);
    
    // Get context switch optimization stats
    if let Some(stats) = get_context_switch_stats() {
        println!("  Optimization stats:");
        println!("    Average switch time: {}ns", stats.average_switch_time_ns);
        println!("    Fastest switch: {}ns", stats.fastest_switch_ns);
        println!("    Slowest switch: {}ns", stats.slowest_switch_ns);
        println!("    Meets target: {}", stats.meets_target);
    }
    
    Ok(results)
}

fn run_fast_path_benchmark() -> Result<Vec<u32>, preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Fast Path Operations Benchmark ===");
    
    let mut handles = vec![];
    
    for i in 0..4 {
        let handle = ThreadBuilder::new()
            .name(&format!("fast_bench_{}", i))
            .priority(12)
            .spawn(move || fast_path_benchmark(i))
            .expect("Failed to spawn fast path benchmark");
        
        handles.push(handle);
    }
    
    let mut results = vec![];
    for handle in handles {
        let operations = handle.join()?;
        results.push(operations);
    }
    
    let total_ops: u32 = results.iter().sum();
    
    // Get fast path metrics
    let metrics = FastPathMetrics::current();
    
    println!("Fast Path Results:");
    println!("  Total operations: {}", total_ops);
    println!("  Fast path hits: {}", metrics.fast_path_hits);
    println!("  Slow path hits: {}", metrics.slow_path_hits);
    println!("  Lock-free operations: {}", metrics.lockfree_operations);
    println!("  Effectiveness ratio: {:.2}%", metrics.effectiveness_ratio() * 100.0);
    println!("  Lock-free ratio: {:.2}%", metrics.lockfree_ratio() * 100.0);
    
    Ok(results)
}

fn show_performance_optimizations() {
    println!("\n=== Performance Optimizations Status ===");
    
    // Context switch optimizations
    if let Some(stats) = get_context_switch_stats() {
        println!("Context Switch Optimization:");
        println!("  Enabled: {}", stats.optimization_enabled);
        println!("  Average time: {}ns", stats.average_switch_time_ns);
        println!("  Target met: {}", stats.meets_target);
    }
    
    // Cache optimizations
    if let Some(stats) = get_cache_stats() {
        println!("Cache Optimization:");
        println!("  L1 hit rate: {:.2}%", stats.l1_hit_rate * 100.0);
        println!("  L2 hit rate: {:.2}%", stats.l2_hit_rate * 100.0);
        println!("  Cache-friendly operations: {}", stats.cache_friendly_ops);
    }
    
    // NUMA optimizations
    if let Some(stats) = get_numa_stats() {
        println!("NUMA Optimization:");
        println!("  Local allocations: {}", stats.local_allocations);
        println!("  Remote allocations: {}", stats.remote_allocations);
        println!("  Locality ratio: {:.2}%", 
                stats.local_allocations as f64 / 
                (stats.local_allocations + stats.remote_allocations).max(1) as f64 * 100.0);
    }
    
    // CPU features
    let cpu_features = get_cpu_features();
    println!("CPU Features:");
    println!("  SIMD support: {}", cpu_features.simd_support);
    println!("  Hardware entropy: {}", cpu_features.hardware_rng);
    println!("  Architecture optimizations: {}", cpu_features.arch_optimizations);
    
    // Memory pool stats
    if let Some(stats) = get_pool_stats() {
        println!("Memory Pool Optimization:");
        println!("  Pool allocations: {}", stats.pool_allocations);
        println!("  Fallback allocations: {}", stats.fallback_allocations);
        println!("  Pool efficiency: {:.2}%", 
                stats.pool_allocations as f64 / 
                (stats.pool_allocations + stats.fallback_allocations).max(1) as f64 * 100.0);
    }
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    println!("=== Performance Testing and Benchmarking ===");
    
    // Initialize performance optimizations
    init_context_switch_optimization();
    init_cache_optimization();
    init_numa_optimization();
    init_cpu_optimization();
    init_memory_pools();
    
    println!("Performance optimizations initialized\n");
    
    // Show initial optimization status
    show_performance_optimizations();
    
    // Run comprehensive benchmarks
    let _cpu_results = run_cpu_benchmarks()?;
    let _memory_results = run_memory_benchmarks()?;
    let _lock_results = run_lock_contention_benchmark()?;
    let _context_results = run_context_switch_benchmark()?;
    let _fast_path_results = run_fast_path_benchmark()?;
    
    // System-wide performance metrics
    println!("\n=== System Performance Metrics ===");
    let system_metrics = SystemMetrics::current();
    println!("Active threads: {}", system_metrics.active_threads);
    println!("Total context switches: {}", system_metrics.total_context_switches);
    println!("CPU utilization: {:.1}%", system_metrics.cpu_utilization * 100.0);
    println!("Memory usage: {}KB", system_metrics.memory_usage_kb);
    println!("Scheduler efficiency: {:.2}%", system_metrics.scheduler_efficiency * 100.0);
    
    // Health monitoring
    let health = HealthMonitor::current_status();
    println!("\nSystem Health:");
    println!("  Overall status: {:?}", health.status);
    println!("  Thread health: {}/{} healthy", 
             health.healthy_threads, health.total_threads);
    println!("  Resource usage: {:.1}%", health.resource_usage * 100.0);
    
    // Final optimization status
    show_performance_optimizations();
    
    println!("\n=== Performance Testing Completed Successfully ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cpu_benchmark() {
        let (time, result) = cpu_benchmark_task(1, 1000);
        assert!(time > 0);
        assert!(result > 0);
    }
    
    #[test]
    fn test_memory_benchmark() {
        let (time, checksum) = memory_benchmark_task(1, 1);
        assert!(time > 0);
        assert!(checksum > 0);
    }
    
    #[test]
    fn test_fibonacci_fast() {
        assert_eq!(fibonacci_fast(0), 0);
        assert_eq!(fibonacci_fast(1), 1);
        assert_eq!(fibonacci_fast(10), 55);
        assert_eq!(fibonacci_fast(20), 6765);
    }
}
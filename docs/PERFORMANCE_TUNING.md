# Performance Tuning Guide

This guide provides comprehensive performance optimization strategies for the preemptive multithreading library.

## Table of Contents

- [Performance Fundamentals](#performance-fundamentals)
- [Context Switch Optimization](#context-switch-optimization)
- [Scheduler Tuning](#scheduler-tuning)
- [Memory Management](#memory-management)
- [NUMA Optimization](#numa-optimization)
- [Cache Optimization](#cache-optimization)
- [Lock-Free Programming](#lock-free-programming)
- [Architecture-Specific Optimizations](#architecture-specific-optimizations)
- [Profiling and Monitoring](#profiling-and-monitoring)
- [Common Performance Issues](#common-performance-issues)

## Performance Fundamentals

### Key Performance Metrics

Understanding these metrics is crucial for optimization:

| Metric | Description | Target | Impact |
|--------|-------------|---------|---------|
| Context Switch Time | Time to switch between threads | <500ns | Thread responsiveness |
| Scheduling Latency | Time from runnable to running | <10μs | Real-time performance |
| Memory Bandwidth | Effective memory throughput | >80% peak | Overall system speed |
| Cache Hit Ratio | L1/L2/L3 cache effectiveness | >95% L1, >90% L2 | Memory access speed |
| Lock Contention | Time waiting for locks | <5% total | Scalability |
| CPU Utilization | Effective CPU usage | >90% target cores | Resource efficiency |

### Performance Hierarchy

Optimization impact from highest to lowest:

1. **Algorithm Choice** (100x-1000x impact)
2. **Data Structure Design** (10x-100x impact)  
3. **Memory Layout** (2x-10x impact)
4. **Microoptimizations** (10%-50% impact)
5. **Compiler Optimizations** (5%-20% impact)

## Context Switch Optimization

### Hardware-Assisted Switching

Enable architecture-specific optimizations:

```rust
use preemptive_mlthreading_rust::{
    perf::context_switch_opt::{
        init_context_switch_optimization,
        OptimizationConfig,
        get_context_switch_stats
    }
};

// Initialize with aggressive optimization
let config = OptimizationConfig {
    enable_timing: true,
    use_optimized_assembly: true,
    enable_stack_prefetch: true,
    minimize_register_saves: true,
    target_switch_time_ns: 300,  // 300ns target
    warmup_switches: 1000,
};

init_context_switch_optimization();

// Monitor performance
let stats = get_context_switch_stats().unwrap();
println!("Average context switch: {}ns", stats.average_switch_time_ns);
println!("Target met: {}", stats.meets_target);
```

### Selective Register Saving

Minimize context switch overhead by saving only necessary state:

```rust
// Configure FPU context saving based on workload
let thread = ThreadBuilder::new()
    .name("compute_thread")
    .enable_fpu(workload_uses_floating_point()) // Only if needed
    .enable_vector(workload_uses_simd())        // Only if needed
    .spawn(compute_function)?;
```

### Stack Prefetching

Improve context switch cache behavior:

```rust
// Enable stack prefetching for better cache locality
ThreadBuilder::new()
    .stack_size(2 * 1024 * 1024)  // Use larger stacks for prefetch efficiency
    .enable_stack_prefetch(true)   // Prefetch next thread's stack
    .spawn(worker_function)?;
```

## Scheduler Tuning

### Time Slice Configuration

Optimize time slices for your workload:

```rust
use preemptive_mlthreading_rust::{Duration, RoundRobinScheduler};

// Short time slices for interactive workloads
let interactive_scheduler = RoundRobinScheduler::new()
    .with_time_slice(Duration::from_millis(1));  // 1ms for responsiveness

// Longer time slices for compute workloads  
let compute_scheduler = RoundRobinScheduler::new()
    .with_time_slice(Duration::from_millis(50)); // 50ms to reduce overhead
```

### Priority Configuration

Use priority levels effectively:

```rust
// System service threads (highest priority)
let system_thread = ThreadBuilder::new()
    .priority(20)  // Highest priority
    .cpu_affinity(1u64 << 0)  // Pin to CPU 0
    .spawn(system_service)?;

// Interactive threads (high priority)
let ui_thread = ThreadBuilder::new() 
    .priority(15)  // High priority
    .spawn(ui_handler)?;

// Background threads (low priority)
let background_thread = ThreadBuilder::new()
    .priority(5)   // Low priority  
    .spawn(background_task)?;
```

### Work-Stealing Configuration

For CPU-intensive workloads with work imbalance:

```rust
#[cfg(feature = "work-stealing")]
use preemptive_mlthreading_rust::WorkStealingScheduler;

#[cfg(feature = "work-stealing")]
let scheduler = WorkStealingScheduler::new()
    .with_numa_awareness(true)      // Enable NUMA optimization
    .with_cache_optimization(true)  // Optimize for cache locality
    .with_steal_batch_size(4)       // Steal 4 tasks at once
    .with_local_queue_size(256);    // Larger local queues

// Create compute pool with work stealing
for i in 0..num_cpus() {
    ThreadBuilder::new()
        .cpu_affinity(1u64 << i)
        .scheduler(scheduler.clone())
        .spawn(move || compute_worker(i))?;
}
```

## Memory Management

### Stack Sizing

Choose optimal stack sizes:

```rust
// Small stacks for simple tasks (saves memory)
let simple_task = ThreadBuilder::new()
    .stack_size(64 * 1024)  // 64KB
    .spawn(simple_function)?;

// Large stacks for recursive/complex tasks
let complex_task = ThreadBuilder::new()
    .stack_size(2 * 1024 * 1024)  // 2MB
    .spawn(recursive_function)?;

// Use huge pages for performance-critical threads
let perf_critical = ThreadBuilder::new()
    .stack_size(2 * 1024 * 1024)  // 2MB (huge page aligned)
    .use_huge_pages(true)         // Enable huge pages
    .spawn(critical_function)?;
```

### Memory Pool Optimization

Use memory pools to reduce allocation overhead:

```rust
use preemptive_mlthreading_rust::perf::memory_pools::{
    init_memory_pools, MemoryPoolConfig
};

// Initialize optimized memory pools
let pool_config = MemoryPoolConfig {
    small_object_size: 64,        // 64-byte objects
    small_pool_capacity: 1000,    // 1000 objects per pool
    medium_object_size: 1024,     // 1KB objects  
    medium_pool_capacity: 100,    // 100 objects per pool
    enable_per_cpu_pools: true,   // Per-CPU pools for better locality
};

init_memory_pools(pool_config);

// Use pools in performance-critical code
let small_buffer = allocate_from_small_pool()?;
let medium_buffer = allocate_from_medium_pool()?;
```

### Lock-Free Data Structures

Minimize lock contention with lock-free structures:

```rust
use preemptive_mlthreading_rust::perf::fast_paths::{LockFreeQueue, FastPaths};

// Use lock-free queue for producer-consumer patterns
static WORK_QUEUE: LockFreeQueue<WorkItem> = LockFreeQueue::new();

// Producer thread
for work_item in work_items {
    WORK_QUEUE.fast_enqueue(work_item)?;
}

// Consumer thread  
while let Some(work_item) = WORK_QUEUE.fast_dequeue() {
    process_work_item(work_item);
}
```

## NUMA Optimization

### Topology-Aware Allocation

Allocate memory on the same NUMA node as the thread:

```rust
use preemptive_mlthreading_rust::perf::numa::{
    get_numa_topology, allocate_on_node, get_current_node
};

// Get NUMA topology
let topology = get_numa_topology();

// Create threads pinned to specific NUMA nodes
for node in 0..topology.node_count() {
    let cpu_mask = topology.get_cpu_mask_for_node(node);
    
    ThreadBuilder::new()
        .cpu_affinity(cpu_mask)
        .numa_node(node)  // Prefer allocation on this node
        .spawn(move || {
            // Allocate thread-local memory on the same NUMA node
            let buffer = allocate_on_node(node, BUFFER_SIZE)?;
            
            // Process data with good NUMA locality
            process_data_locally(buffer);
        })?;
}
```

### Migration Avoidance

Prevent threads from migrating between NUMA nodes:

```rust
// Pin threads to specific NUMA nodes to avoid migration
let node_0_threads: Vec<JoinHandle<_>> = (0..4)
    .map(|i| {
        ThreadBuilder::new()
            .name(&format!("node0_worker_{}", i))
            .cpu_affinity(topology.get_cpu_mask_for_node(0))
            .migration_policy(MigrationPolicy::Prohibited)
            .spawn(numa_worker)
    })
    .collect::<Result<Vec<_>, _>>()?;
```

## Cache Optimization

### Data Structure Alignment

Align hot data structures to cache lines:

```rust
// Cache-line aligned data for better performance
#[repr(align(64))]  // 64-byte cache line
struct CacheOptimizedData {
    // Hot data together
    counter: AtomicU64,
    flags: AtomicU32,
    
    // Padding to separate from cold data
    _padding: [u8; 64 - 12],
    
    // Cold data
    debug_info: String,
    creation_time: Instant,
}

// Use cache-optimized structures in performance-critical code
static PERF_DATA: CacheOptimizedData = CacheOptimizedData {
    counter: AtomicU64::new(0),
    flags: AtomicU32::new(0), 
    _padding: [0; 52],
    debug_info: String::new(),
    creation_time: Instant::now(),
};
```

### False Sharing Avoidance

Prevent false sharing between threads:

```rust
// Separate frequently-modified data to different cache lines
#[repr(align(64))]
struct PerCpuData {
    cpu_id: u32,
    local_counter: AtomicU64,
    // Pad to cache line boundary
    _padding: [u8; 64 - 4 - 8],
}

// Array of per-CPU data to avoid false sharing
static PER_CPU_DATA: [PerCpuData; 64] = [PerCpuData {
    cpu_id: 0,
    local_counter: AtomicU64::new(0),
    _padding: [0; 52],
}; 64];
```

### Prefetching Strategies

Use prefetching to improve cache hit rates:

```rust
use preemptive_mlthreading_rust::perf::cache_aware::{prefetch_read, prefetch_write};

// Prefetch data before processing
fn process_array(data: &[DataItem]) {
    for i in 0..data.len() {
        // Prefetch next items while processing current
        if i + 4 < data.len() {
            prefetch_read(&data[i + 4]);
        }
        
        process_item(&data[i]);
    }
}

// Prefetch in context switch path
unsafe fn optimized_schedule_next() {
    if let Some(next_thread) = find_next_thread() {
        // Prefetch next thread's stack and context
        prefetch_read(next_thread.context_ptr());
        prefetch_read(next_thread.stack_top());
        
        switch_to_thread(next_thread);
    }
}
```

## Lock-Free Programming

### Fast Path Optimization

Use lock-free fast paths for common operations:

```rust
use preemptive_mlthreading_rust::perf::fast_paths::FastPaths;

// Fast path operations avoid locks entirely
fn optimized_increment_counter() {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    
    // Use fast atomic increment (single instruction)
    let old_value = FastPaths::fast_atomic_increment(&COUNTER);
    
    // Only use slow path if needed
    if old_value > THRESHOLD {
        handle_overflow_slow_path();
    }
}

// Fast mutex operations
fn try_fast_lock(mutex: &Mutex<Data>) -> Option<MutexGuard<Data>> {
    // Try fast lock first (single CAS)
    if let Ok(guard) = mutex.try_lock() {
        FastPaths::record_fast_path();
        Some(guard)
    } else {
        FastPaths::record_slow_path();
        None
    }
}
```

### Lock-Free Data Structures

Replace locks with lock-free alternatives:

```rust
// Lock-free stack for high-performance scenarios
use preemptive_mlthreading_rust::sync::LockFreeStack;

static WORK_STACK: LockFreeStack<WorkItem> = LockFreeStack::new();

// Multiple producers can push simultaneously
fn producer_thread() {
    for item in work_items {
        WORK_STACK.push(item);  // No locks needed
    }
}

// Multiple consumers can pop simultaneously  
fn consumer_thread() {
    while let Some(item) = WORK_STACK.pop() {
        process_item(item);
    }
}
```

### Read-Copy-Update (RCU) Patterns

Use RCU for read-heavy data structures:

```rust
use preemptive_mlthreading_rust::sync::RcuProtected;

// RCU-protected configuration that changes rarely
static CONFIG: RcuProtected<SystemConfig> = RcuProtected::new(SystemConfig::default());

// Readers access data without locks
fn read_config() -> SystemConfig {
    let guard = CONFIG.read();
    (*guard).clone()  // Fast read access
}

// Writers update atomically 
fn update_config(new_config: SystemConfig) {
    CONFIG.update(|_old| new_config);  // Atomic update
    CONFIG.synchronize();              // Wait for readers
}
```

## Architecture-Specific Optimizations

### x86_64 Optimizations

```rust
// Use x86_64-specific features
#[cfg(target_arch = "x86_64")]
fn x86_64_optimizations() {
    // Use RDTSC for high-precision timing
    let start_cycles = unsafe { core::arch::x86_64::_rdtsc() };
    
    // Critical work
    do_work();
    
    let end_cycles = unsafe { core::arch::x86_64::_rdtsc() };
    let elapsed_cycles = end_cycles - start_cycles;
    
    // Use PAUSE instruction in spin loops
    for _ in 0..100 {
        if try_acquire_resource() {
            break;
        }
        unsafe { core::arch::x86_64::_mm_pause(); }
    }
    
    // Use PREFETCH instructions for cache optimization
    unsafe {
        core::arch::x86_64::_mm_prefetch(data_ptr, core::arch::x86_64::_MM_HINT_T0);
    }
}
```

### ARM64 Optimizations  

```rust
// Use ARM64-specific features
#[cfg(target_arch = "aarch64")]
fn arm64_optimizations() {
    // Use NEON for vector operations
    unsafe {
        let a = core::arch::aarch64::vld1q_u32(src_ptr);
        let b = core::arch::aarch64::vaddq_u32(a, a);
        core::arch::aarch64::vst1q_u32(dst_ptr, b);
    }
    
    // Use WFE/SEV for efficient waiting
    fn efficient_wait_for_condition<F>(condition: F) 
    where F: Fn() -> bool 
    {
        while !condition() {
            unsafe { core::arch::asm!("wfe") };  // Wait for event
        }
    }
    
    // Use pointer authentication for security without performance cost
    unsafe {
        let authenticated_ptr: *const u8;
        core::arch::asm!(
            "pacia {ptr}, sp",
            ptr = inout(reg) function_ptr => authenticated_ptr
        );
    }
}
```

### RISC-V Optimizations

```rust
// Use RISC-V-specific features
#[cfg(target_arch = "riscv64")]
fn riscv_optimizations() {
    // Use vector extension for data parallel work
    if cpu_has_vector_extension() {
        process_with_vectors(data);
    } else {
        process_scalar(data);
    }
    
    // Use custom instructions if available
    if has_custom_threading_extension() {
        unsafe {
            core::arch::asm!(
                ".insn r 0x0B, 0x0, 0x0, x0, {}, x0",
                in(reg) custom_operation_arg
            );
        }
    }
    
    // Use bit manipulation extensions
    let leading_zeros = if cpu_has_zbb() {
        let result: u64;
        unsafe {
            core::arch::asm!("clz {}, {}", out(reg) result, in(reg) value);
        }
        result
    } else {
        value.leading_zeros() as u64
    };
}
```

## Profiling and Monitoring

### Performance Counter Integration

Use hardware performance counters for detailed analysis:

```rust
use preemptive_mlthreading_rust::{ThreadProfiler, SystemMetrics};

// Enable detailed performance monitoring
let profiler = ThreadProfiler::new();
profiler.enable_context_switch_tracking();
profiler.enable_cache_miss_tracking();
profiler.enable_branch_prediction_tracking();

// Create monitored thread
let handle = ThreadBuilder::new()
    .enable_profiling(true)
    .spawn(|| {
        // Performance-critical work
        critical_computation();
    })?;

// Analyze results
let thread_id = handle.thread().id();
handle.join()?;

let metrics = ThreadMetrics::for_thread(thread_id)?;
println!("Context switches: {}", metrics.context_switches);
println!("Cache miss rate: {:.2}%", metrics.cache_miss_rate * 100.0);
println!("Branch mispredict rate: {:.2}%", metrics.branch_mispredict_rate * 100.0);
println!("Average CPU utilization: {:.1}%", metrics.cpu_utilization * 100.0);
```

### Real-Time Monitoring

Set up continuous performance monitoring:

```rust
use preemptive_mlthreading_rust::{SystemMetrics, HealthMonitor};

// Monitor system performance
fn start_performance_monitoring() -> Result<(), MonitorError> {
    let monitor = HealthMonitor::new();
    monitor.set_update_interval(Duration::from_millis(100));
    
    // Set performance thresholds
    monitor.set_context_switch_threshold(Duration::from_nanos(1000));
    monitor.set_memory_usage_threshold(0.85);  // 85% memory usage
    monitor.set_cpu_usage_threshold(0.95);     // 95% CPU usage
    
    // Start monitoring thread
    ThreadBuilder::new()
        .name("perf_monitor")
        .priority(18)  // High priority for monitoring
        .spawn(move || {
            loop {
                let metrics = SystemMetrics::current();
                
                // Check for performance issues
                if metrics.average_context_switch_time > Duration::from_nanos(1000) {
                    warn!("High context switch latency: {:?}", 
                          metrics.average_context_switch_time);
                }
                
                if metrics.memory_usage > 0.85 {
                    warn!("High memory usage: {:.1}%", metrics.memory_usage * 100.0);
                }
                
                sleep(Duration::from_millis(100));
            }
        })?;
    
    Ok(())
}
```

## Common Performance Issues

### Issue: High Context Switch Overhead

**Symptoms:**
- High CPU usage with low throughput
- Context switch times >1μs
- Poor interactive responsiveness

**Solutions:**
```rust
// 1. Reduce context switch frequency
let scheduler = RoundRobinScheduler::new()
    .with_time_slice(Duration::from_millis(10));  // Longer time slices

// 2. Use cooperative scheduling where possible
fn cooperative_worker() {
    for i in 0..1000 {
        do_work_batch();
        
        // Yield only at safe points
        if i % 100 == 0 {
            yield_now();
        }
    }
}

// 3. Enable context switch optimization
init_context_switch_optimization();
```

### Issue: Lock Contention

**Symptoms:**
- Threads spending time waiting for locks
- Poor scalability with more cores
- High variance in execution times

**Solutions:**
```rust
// 1. Use lock-free data structures
use preemptive_mlthreading_rust::sync::LockFreeQueue;
static QUEUE: LockFreeQueue<WorkItem> = LockFreeQueue::new();

// 2. Reduce critical section size
{
    let guard = mutex.lock();
    let data = (*guard).clone();  // Copy data out
} // Release lock early

// Process data outside of lock
process_data(data);

// 3. Use reader-writer locks for read-heavy workloads
use preemptive_mlthreading_rust::sync::RwLock;
static DATA: RwLock<SharedData> = RwLock::new(SharedData::new());

// Many readers can access simultaneously
let data = DATA.read();

// Only one writer at a time
let mut data = DATA.write();
```

### Issue: Cache Misses

**Symptoms:**
- High memory access latency
- Poor data processing throughput
- High L2/L3 cache miss rates

**Solutions:**
```rust
// 1. Improve data locality
#[repr(C)]
struct OptimizedData {
    // Hot data together
    frequently_accessed: [u32; 4],
    
    // Cold data separate  
    rarely_accessed: Vec<u8>,
}

// 2. Use prefetching
fn process_with_prefetch(items: &[DataItem]) {
    for i in 0..items.len() {
        // Prefetch future items
        if i + 8 < items.len() {
            prefetch_read(&items[i + 8]);
        }
        
        process_item(&items[i]);
    }
}

// 3. Align data structures
#[repr(align(64))]  // Cache line alignment
struct CacheAligned {
    data: [u8; 64],
}
```

### Issue: NUMA Effects

**Symptoms:**
- Performance varies by thread placement
- Memory access latency differences
- Poor scalability on multi-socket systems

**Solutions:**
```rust
// 1. NUMA-aware thread placement
let topology = get_numa_topology();
for node in 0..topology.node_count() {
    let cpu_mask = topology.get_cpu_mask_for_node(node);
    
    ThreadBuilder::new()
        .cpu_affinity(cpu_mask)
        .numa_node(node)
        .spawn(numa_aware_worker)?;
}

// 2. Local memory allocation
fn numa_worker() -> Result<(), Error> {
    let current_node = get_current_numa_node();
    let local_buffer = allocate_on_node(current_node, BUFFER_SIZE)?;
    
    // Use local buffer for better performance
    process_locally(local_buffer);
    Ok(())
}
```

## Performance Testing Framework

### Benchmark Setup

```rust
use preemptive_mlthreading_rust::{ThreadBuilder, Duration, SystemMetrics};

fn benchmark_context_switches(num_threads: usize, iterations: usize) -> BenchmarkResult {
    let start_time = Instant::now();
    let start_metrics = SystemMetrics::current();
    
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            ThreadBuilder::new()
                .name(&format!("bench_thread_{}", i))
                .spawn(move || {
                    for _ in 0..iterations {
                        yield_now();
                    }
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    
    // Wait for completion
    for handle in handles {
        handle.join()?;
    }
    
    let elapsed = start_time.elapsed();
    let end_metrics = SystemMetrics::current();
    
    let context_switches = end_metrics.total_context_switches - 
                          start_metrics.total_context_switches;
    
    BenchmarkResult {
        total_time: elapsed,
        context_switches,
        switches_per_second: (context_switches as f64 / elapsed.as_secs_f64()) as u64,
        average_switch_time: elapsed / context_switches as u32,
    }
}
```

### Automated Performance Regression Testing

```rust
fn performance_regression_test() -> Result<(), TestError> {
    let baseline = load_baseline_performance()?;
    let current = measure_current_performance()?;
    
    // Check for regressions (>5% slower)
    if current.context_switch_time > baseline.context_switch_time * 1.05 {
        return Err(TestError::PerformanceRegression {
            metric: "context_switch_time",
            baseline: baseline.context_switch_time,
            current: current.context_switch_time,
        });
    }
    
    if current.throughput < baseline.throughput * 0.95 {
        return Err(TestError::PerformanceRegression {
            metric: "throughput", 
            baseline: baseline.throughput,
            current: current.throughput,
        });
    }
    
    // Update baseline if performance improved significantly
    if current.throughput > baseline.throughput * 1.1 {
        save_new_baseline(current)?;
        println!("Performance improved! New baseline saved.");
    }
    
    Ok(())
}
```

## Conclusion

Performance tuning is an iterative process that requires:

1. **Measurement First**: Always profile before optimizing
2. **Focus on Hotspots**: Optimize the 20% of code that takes 80% of time
3. **Architecture Awareness**: Use platform-specific optimizations
4. **Trade-off Analysis**: Balance performance vs complexity/maintainability
5. **Continuous Monitoring**: Watch for performance regressions
6. **Holistic Approach**: Consider system-wide effects, not just local optimizations

The preemptive multithreading library provides extensive tools and configuration options to achieve optimal performance for your specific workload. Use the profiling and monitoring capabilities to guide your optimization efforts and validate improvements.

For additional help with performance issues, see the troubleshooting sections in the architecture-specific guides:
- [x86_64 Performance Guide](architecture/x86_64.md#performance-optimizations)  
- [ARM64 Performance Guide](architecture/arm64.md#performance-optimizations)
- [RISC-V Performance Guide](architecture/riscv64.md#performance-optimizations)
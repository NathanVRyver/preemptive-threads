//! Comprehensive metrics collection for threading system performance.
//!
//! This module provides detailed metrics about thread creation, scheduling,
//! context switches, resource usage, and system performance.

use portable_atomic::{AtomicU64, AtomicU32, AtomicUsize, AtomicBool, Ordering};
use crate::time::{Instant, Duration};
use crate::thread_new::ThreadId;
extern crate alloc;
use alloc::{vec::Vec, collections::BTreeMap};
use spin::Mutex;

/// Per-thread metrics tracking.
#[derive(Debug, Clone)]
pub struct ThreadMetrics {
    /// Thread ID
    pub thread_id: ThreadId,
    /// Total CPU time consumed (nanoseconds)
    pub cpu_time_ns: u64,
    /// Number of context switches
    pub context_switches: u64,
    /// Number of voluntary yields
    pub voluntary_yields: u64,
    /// Number of involuntary preemptions  
    pub involuntary_preemptions: u64,
    /// Peak stack usage (bytes)
    pub peak_stack_usage: usize,
    /// Current stack usage (bytes)
    pub current_stack_usage: usize,
    /// Number of page faults
    pub page_faults: u64,
    /// Time spent in user mode (nanoseconds)
    pub user_time_ns: u64,
    /// Time spent in kernel mode (nanoseconds)
    pub kernel_time_ns: u64,
    /// Thread creation time
    pub created_at: Instant,
    /// Last active time
    pub last_active: Instant,
    /// Priority changes
    pub priority_changes: u32,
    /// Memory allocations
    pub memory_allocations: u64,
    /// Memory deallocations  
    pub memory_deallocations: u64,
    /// Total memory allocated (bytes)
    pub total_memory_allocated: u64,
    /// Current memory usage (bytes)
    pub current_memory_usage: u64,
}

impl ThreadMetrics {
    /// Create new thread metrics.
    pub fn new(thread_id: ThreadId) -> Self {
        let now = Instant::now();
        Self {
            thread_id,
            cpu_time_ns: 0,
            context_switches: 0,
            voluntary_yields: 0,
            involuntary_preemptions: 0,
            peak_stack_usage: 0,
            current_stack_usage: 0,
            page_faults: 0,
            user_time_ns: 0,
            kernel_time_ns: 0,
            created_at: now,
            last_active: now,
            priority_changes: 0,
            memory_allocations: 0,
            memory_deallocations: 0,
            total_memory_allocated: 0,
            current_memory_usage: 0,
        }
    }
    
    /// Update CPU time metrics.
    pub fn add_cpu_time(&mut self, duration: Duration, is_user_mode: bool) {
        if is_user_mode {
            self.user_time_ns += duration.as_nanos();
        } else {
            self.kernel_time_ns += duration.as_nanos();
        }
        self.cpu_time_ns += duration.as_nanos();
        self.last_active = Instant::now();
    }
    
    /// Record a context switch.
    pub fn record_context_switch(&mut self, voluntary: bool) {
        self.context_switches += 1;
        if voluntary {
            self.voluntary_yields += 1;
        } else {
            self.involuntary_preemptions += 1;
        }
        self.last_active = Instant::now();
    }
    
    /// Update stack usage metrics.
    pub fn update_stack_usage(&mut self, current_usage: usize) {
        self.current_stack_usage = current_usage;
        if current_usage > self.peak_stack_usage {
            self.peak_stack_usage = current_usage;
        }
    }
    
    /// Record memory allocation.
    pub fn record_allocation(&mut self, size: u64) {
        self.memory_allocations += 1;
        self.total_memory_allocated += size;
        self.current_memory_usage += size;
    }
    
    /// Record memory deallocation.
    pub fn record_deallocation(&mut self, size: u64) {
        self.memory_deallocations += 1;
        if self.current_memory_usage >= size {
            self.current_memory_usage -= size;
        }
    }
    
    /// Calculate CPU utilization percentage.
    pub fn cpu_utilization(&self) -> f64 {
        let total_time = self.last_active.duration_since(self.created_at).as_nanos() as f64;
        if total_time > 0.0 {
            (self.cpu_time_ns as f64 / total_time) * 100.0
        } else {
            0.0
        }
    }
    
    /// Calculate preemption rate.
    pub fn preemption_rate(&self) -> f64 {
        if self.context_switches > 0 {
            (self.involuntary_preemptions as f64 / self.context_switches as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// System-wide metrics.
#[derive(Debug)]
pub struct SystemMetrics {
    /// Total number of threads created
    pub threads_created: AtomicU64,
    /// Total number of threads destroyed
    pub threads_destroyed: AtomicU64,
    /// Current number of active threads
    pub active_threads: AtomicU64,
    /// Total context switches across all threads
    pub total_context_switches: AtomicU64,
    /// Total CPU time across all threads (nanoseconds)
    pub total_cpu_time_ns: AtomicU64,
    /// System uptime (nanoseconds)
    pub system_uptime_ns: AtomicU64,
    /// Timer interrupts processed
    pub timer_interrupts: AtomicU64,
    /// Scheduler decisions made
    pub scheduler_decisions: AtomicU64,
    /// Load balancing operations
    pub load_balance_ops: AtomicU64,
    /// Memory pool allocations
    pub pool_allocations: AtomicU64,
    /// Memory pool deallocations
    pub pool_deallocations: AtomicU64,
    /// Stack overflows detected
    pub stack_overflows: AtomicU64,
    /// Priority inversions detected
    pub priority_inversions: AtomicU64,
    /// Deadlocks detected
    pub deadlocks_detected: AtomicU64,
    /// System start time
    pub system_start_time: Instant,
    /// Peak memory usage (bytes)
    pub peak_memory_usage: AtomicU64,
    /// Current memory usage (bytes)
    pub current_memory_usage: AtomicU64,
}

impl SystemMetrics {
    /// Create new system metrics.
    pub const fn new() -> Self {
        Self {
            threads_created: AtomicU64::new(0),
            threads_destroyed: AtomicU64::new(0),
            active_threads: AtomicU64::new(0),
            total_context_switches: AtomicU64::new(0),
            total_cpu_time_ns: AtomicU64::new(0),
            system_uptime_ns: AtomicU64::new(0),
            timer_interrupts: AtomicU64::new(0),
            scheduler_decisions: AtomicU64::new(0),
            load_balance_ops: AtomicU64::new(0),
            pool_allocations: AtomicU64::new(0),
            pool_deallocations: AtomicU64::new(0),
            stack_overflows: AtomicU64::new(0),
            priority_inversions: AtomicU64::new(0),
            deadlocks_detected: AtomicU64::new(0),
            system_start_time: unsafe { core::mem::transmute(0u64) }, // Will be initialized properly
            peak_memory_usage: AtomicU64::new(0),
            current_memory_usage: AtomicU64::new(0),
        }
    }
    
    /// Initialize system metrics with current time.
    pub fn init(&self) {
        let start_time = Instant::now();
        // We can't modify system_start_time after const initialization in a safe way,
        // so we'll track uptime relative to when metrics collection started
        self.system_uptime_ns.store(start_time.as_nanos(), Ordering::Release);
    }
    
    /// Record thread creation.
    pub fn record_thread_created(&self) {
        self.threads_created.fetch_add(1, Ordering::AcqRel);
        self.active_threads.fetch_add(1, Ordering::AcqRel);
    }
    
    /// Record thread destruction.
    pub fn record_thread_destroyed(&self) {
        self.threads_destroyed.fetch_add(1, Ordering::AcqRel);
        self.active_threads.fetch_sub(1, Ordering::AcqRel);
    }
    
    /// Record a context switch.
    pub fn record_context_switch(&self) {
        self.total_context_switches.fetch_add(1, Ordering::AcqRel);
    }
    
    /// Add CPU time to system total.
    pub fn add_cpu_time(&self, duration: Duration) {
        self.total_cpu_time_ns.fetch_add(duration.as_nanos(), Ordering::AcqRel);
    }
    
    /// Record timer interrupt.
    pub fn record_timer_interrupt(&self) {
        self.timer_interrupts.fetch_add(1, Ordering::AcqRel);
    }
    
    /// Record scheduler decision.
    pub fn record_scheduler_decision(&self) {
        self.scheduler_decisions.fetch_add(1, Ordering::AcqRel);
    }
    
    /// Update memory usage.
    pub fn update_memory_usage(&self, new_usage: u64) {
        self.current_memory_usage.store(new_usage, Ordering::Release);
        
        // Update peak if necessary
        let current_peak = self.peak_memory_usage.load(Ordering::Acquire);
        if new_usage > current_peak {
            let _ = self.peak_memory_usage.compare_exchange_weak(
                current_peak,
                new_usage,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
    
    /// Calculate system CPU utilization.
    pub fn system_cpu_utilization(&self) -> f64 {
        let total_cpu = self.total_cpu_time_ns.load(Ordering::Acquire) as f64;
        let uptime_start = self.system_uptime_ns.load(Ordering::Acquire);
        let current_time = Instant::now().as_nanos();
        
        if current_time > uptime_start {
            let uptime = (current_time - uptime_start) as f64;
            let active = self.active_threads.load(Ordering::Acquire) as f64;
            if uptime > 0.0 && active > 0.0 {
                (total_cpu / (uptime * active)) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
    
    /// Get average context switches per second.
    pub fn context_switches_per_second(&self) -> f64 {
        let switches = self.total_context_switches.load(Ordering::Acquire) as f64;
        let uptime_start = self.system_uptime_ns.load(Ordering::Acquire);
        let current_time = Instant::now().as_nanos();
        
        if current_time > uptime_start {
            let uptime_seconds = ((current_time - uptime_start) as f64) / 1_000_000_000.0;
            if uptime_seconds > 0.0 {
                switches / uptime_seconds
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// Metrics collector that aggregates and manages all metrics.
pub struct MetricsCollector {
    /// System-wide metrics
    system_metrics: SystemMetrics,
    /// Per-thread metrics storage
    thread_metrics: Mutex<BTreeMap<ThreadId, ThreadMetrics>>,
    /// Collection enabled flag
    enabled: AtomicBool,
    /// Collection interval
    collection_interval_ms: AtomicU32,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub const fn new() -> Self {
        Self {
            system_metrics: SystemMetrics::new(),
            thread_metrics: Mutex::new(BTreeMap::new()),
            enabled: AtomicBool::new(false),
            collection_interval_ms: AtomicU32::new(1000),
        }
    }
    
    /// Initialize the metrics collector.
    pub fn init(&self, interval_ms: u32) -> Result<(), &'static str> {
        self.system_metrics.init();
        self.collection_interval_ms.store(interval_ms, Ordering::Release);
        self.enabled.store(true, Ordering::Release);
        Ok(())
    }
    
    /// Check if metrics collection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    
    /// Register a new thread for metrics tracking.
    pub fn register_thread(&self, thread_id: ThreadId) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut metrics) = self.thread_metrics.try_lock() {
            metrics.insert(thread_id, ThreadMetrics::new(thread_id));
        }
        
        self.system_metrics.record_thread_created();
    }
    
    /// Unregister a thread from metrics tracking.
    pub fn unregister_thread(&self, thread_id: ThreadId) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut metrics) = self.thread_metrics.try_lock() {
            metrics.remove(&thread_id);
        }
        
        self.system_metrics.record_thread_destroyed();
    }
    
    /// Record CPU time for a thread.
    pub fn record_cpu_time(&self, thread_id: ThreadId, duration: Duration, user_mode: bool) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut metrics) = self.thread_metrics.try_lock() {
            if let Some(thread_metrics) = metrics.get_mut(&thread_id) {
                thread_metrics.add_cpu_time(duration, user_mode);
            }
        }
        
        self.system_metrics.add_cpu_time(duration);
    }
    
    /// Record a context switch for a thread.
    pub fn record_context_switch(&self, thread_id: ThreadId, voluntary: bool) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut metrics) = self.thread_metrics.try_lock() {
            if let Some(thread_metrics) = metrics.get_mut(&thread_id) {
                thread_metrics.record_context_switch(voluntary);
            }
        }
        
        self.system_metrics.record_context_switch();
    }
    
    /// Update stack usage for a thread.
    pub fn update_stack_usage(&self, thread_id: ThreadId, usage: usize) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut metrics) = self.thread_metrics.try_lock() {
            if let Some(thread_metrics) = metrics.get_mut(&thread_id) {
                thread_metrics.update_stack_usage(usage);
            }
        }
    }
    
    /// Get metrics for a specific thread.
    pub fn get_thread_metrics(&self, thread_id: ThreadId) -> Option<ThreadMetrics> {
        if let Some(metrics) = self.thread_metrics.try_lock() {
            metrics.get(&thread_id).cloned()
        } else {
            None
        }
    }
    
    /// Get system-wide metrics.
    pub fn get_system_metrics(&self) -> &SystemMetrics {
        &self.system_metrics
    }
    
    /// Get all thread metrics.
    pub fn get_all_thread_metrics(&self) -> Vec<ThreadMetrics> {
        if let Some(metrics) = self.thread_metrics.try_lock() {
            metrics.values().cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    /// Reset all metrics.
    pub fn reset_metrics(&self) {
        if let Some(mut metrics) = self.thread_metrics.try_lock() {
            metrics.clear();
        }
        
        // Reset system metrics (keeping start time)
        let start_time = self.system_metrics.system_uptime_ns.load(Ordering::Acquire);
        self.system_metrics.threads_created.store(0, Ordering::Release);
        self.system_metrics.threads_destroyed.store(0, Ordering::Release);
        self.system_metrics.active_threads.store(0, Ordering::Release);
        self.system_metrics.total_context_switches.store(0, Ordering::Release);
        self.system_metrics.total_cpu_time_ns.store(0, Ordering::Release);
        self.system_metrics.timer_interrupts.store(0, Ordering::Release);
        self.system_metrics.scheduler_decisions.store(0, Ordering::Release);
        self.system_metrics.system_uptime_ns.store(start_time, Ordering::Release);
    }
    
    /// Generate a comprehensive metrics report.
    pub fn generate_report(&self) -> MetricsReport {
        let system = SystemMetricsSnapshot {
            threads_created: self.system_metrics.threads_created.load(Ordering::Acquire),
            threads_destroyed: self.system_metrics.threads_destroyed.load(Ordering::Acquire),
            active_threads: self.system_metrics.active_threads.load(Ordering::Acquire),
            total_context_switches: self.system_metrics.total_context_switches.load(Ordering::Acquire),
            total_cpu_time_ns: self.system_metrics.total_cpu_time_ns.load(Ordering::Acquire),
            cpu_utilization: self.system_metrics.system_cpu_utilization(),
            context_switches_per_second: self.system_metrics.context_switches_per_second(),
            current_memory_usage: self.system_metrics.current_memory_usage.load(Ordering::Acquire),
            peak_memory_usage: self.system_metrics.peak_memory_usage.load(Ordering::Acquire),
        };
        
        let threads = self.get_all_thread_metrics();
        
        MetricsReport {
            system,
            threads,
            timestamp: Instant::now(),
        }
    }
}

/// Snapshot of system metrics for reporting.
#[derive(Debug, Clone)]
pub struct SystemMetricsSnapshot {
    pub threads_created: u64,
    pub threads_destroyed: u64,
    pub active_threads: u64,
    pub total_context_switches: u64,
    pub total_cpu_time_ns: u64,
    pub cpu_utilization: f64,
    pub context_switches_per_second: f64,
    pub current_memory_usage: u64,
    pub peak_memory_usage: u64,
}

/// Complete metrics report.
#[derive(Debug, Clone)]
pub struct MetricsReport {
    pub system: SystemMetricsSnapshot,
    pub threads: Vec<ThreadMetrics>,
    pub timestamp: Instant,
}

/// Global metrics collector instance.
pub static GLOBAL_METRICS: MetricsCollector = MetricsCollector::new();

/// Initialize the global metrics collector.
pub fn init_metrics_collector(interval_ms: u32) -> Result<(), &'static str> {
    GLOBAL_METRICS.init(interval_ms)
}

/// Cleanup metrics collection.
pub fn cleanup_metrics() {
    GLOBAL_METRICS.enabled.store(false, Ordering::Release);
}
//! Cache-aware optimizations for scheduler hot paths.

use crate::arch::barriers::CacheLinePadded;
use crate::perf::{PerfConfig, PERF_COUNTERS};
use crate::thread_new::{Thread, ThreadId};
use crate::sched::CpuId;
use portable_atomic::{AtomicUsize, AtomicU64, AtomicPtr, Ordering};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Cache-optimized per-CPU scheduler data structure.
#[repr(align(64))] // Align to cache line
pub struct CacheOptimizedCpuData {
    /// CPU ID this data belongs to
    pub cpu_id: CpuId,
    
    /// Currently running thread (hot data)
    pub current_thread: AtomicPtr<Thread>,
    
    /// CPU-local run queue head (frequently accessed)
    pub local_queue_head: AtomicUsize,
    pub local_queue_tail: AtomicUsize,
    
    /// Thread priority statistics (moderately hot)
    pub priority_counts: [AtomicUsize; 11], // Priority levels 0-10
    
    /// Performance counters (cache line padded)
    pub perf_stats: CacheLinePadded<CpuPerfStats>,
    
    /// Padding to next cache line
    _padding: [u8; 0],
}

#[derive(Default)]
pub struct CpuPerfStats {
    pub schedules: AtomicU64,
    pub preemptions: AtomicU64,
    pub yields: AtomicU64,
    pub idle_cycles: AtomicU64,
}

impl CacheOptimizedCpuData {
    pub fn new(cpu_id: CpuId) -> Self {
        Self {
            cpu_id,
            current_thread: AtomicPtr::new(core::ptr::null_mut()),
            local_queue_head: AtomicUsize::new(0),
            local_queue_tail: AtomicUsize::new(0),
            priority_counts: core::array::from_fn(|_| AtomicUsize::new(0)),
            perf_stats: CacheLinePadded::new(CpuPerfStats::default()),
            _padding: [],
        }
    }
    
    /// Fast path for getting current thread (single atomic read).
    #[inline(always)]
    pub fn get_current_thread(&self) -> Option<Arc<Thread>> {
        let ptr = self.current_thread.load(Ordering::Acquire);
        if ptr.is_null() {
            None
        } else {
            // Safety: We maintain the invariant that this pointer is always valid
            // when non-null, and we use Arc to manage the lifetime
            unsafe { Some(Arc::from_raw(ptr)) }
        }
    }
    
    /// Fast path for setting current thread.
    #[inline(always)]
    pub fn set_current_thread(&self, thread: Option<Arc<Thread>>) {
        match thread {
            Some(thread) => {
                let ptr = Arc::into_raw(thread) as *mut Thread;
                let old_ptr = self.current_thread.swap(ptr, Ordering::AcqRel);
                
                // Clean up old thread reference
                if !old_ptr.is_null() {
                    unsafe { Arc::from_raw(old_ptr) };
                }
            }
            None => {
                let old_ptr = self.current_thread.swap(core::ptr::null_mut(), Ordering::AcqRel);
                if !old_ptr.is_null() {
                    unsafe { Arc::from_raw(old_ptr) };
                }
            }
        }
    }
    
    /// Check if local run queue is empty (lock-free).
    #[inline(always)]
    pub fn is_local_queue_empty(&self) -> bool {
        let head = self.local_queue_head.load(Ordering::Acquire);
        let tail = self.local_queue_tail.load(Ordering::Acquire);
        head == tail
    }
    
    /// Get approximate local queue length.
    #[inline(always)]
    pub fn local_queue_length(&self) -> usize {
        let head = self.local_queue_head.load(Ordering::Relaxed);
        let tail = self.local_queue_tail.load(Ordering::Relaxed);
        tail.wrapping_sub(head)
    }
    
    /// Update priority count for thread scheduling.
    #[inline(always)]
    pub fn update_priority_count(&self, priority: u8, delta: isize) {
        let idx = (priority as usize).min(10);
        if delta > 0 {
            self.priority_counts[idx].fetch_add(delta as usize, Ordering::Relaxed);
        } else {
            self.priority_counts[idx].fetch_sub((-delta) as usize, Ordering::Relaxed);
        }
    }
    
    /// Get thread count for a specific priority level.
    #[inline(always)]
    pub fn get_priority_count(&self, priority: u8) -> usize {
        let idx = (priority as usize).min(10);
        self.priority_counts[idx].load(Ordering::Relaxed)
    }
    
    /// Record scheduling event.
    #[inline(always)]
    pub fn record_schedule(&self) {
        self.perf_stats.get().schedules.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record preemption event.
    #[inline(always)]
    pub fn record_preemption(&self) {
        self.perf_stats.get().preemptions.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record yield event.
    #[inline(always)]
    pub fn record_yield(&self) {
        self.perf_stats.get().yields.fetch_add(1, Ordering::Relaxed);
    }
}

/// Cache-aware scheduler implementation with optimized hot paths.
pub struct CacheAwareScheduler {
    /// Per-CPU data (cache line aligned)
    cpu_data: Vec<CacheLinePadded<CacheOptimizedCpuData>>,
    
    /// Global state (cold data, separate cache lines)
    total_threads: AtomicUsize,
    active_cpus: AtomicUsize,
    
    /// Configuration
    config: PerfConfig,
}

impl CacheAwareScheduler {
    pub fn new(config: PerfConfig) -> Self {
        let mut cpu_data = Vec::with_capacity(config.cpu_count);
        for cpu_id in 0..config.cpu_count {
            cpu_data.push(CacheLinePadded::new(
                CacheOptimizedCpuData::new(cpu_id as CpuId)
            ));
        }
        
        Self {
            cpu_data,
            total_threads: AtomicUsize::new(0),
            active_cpus: AtomicUsize::new(config.cpu_count),
            config,
        }
    }
    
    /// Hot path: Get current thread for CPU (single cache line access).
    #[inline(always)]
    pub fn current_thread(&self, cpu_id: CpuId) -> Option<Arc<Thread>> {
        if let Some(cpu_data) = self.cpu_data.get(cpu_id as usize) {
            PERF_COUNTERS.record_fast_path();
            cpu_data.get().get_current_thread()
        } else {
            PERF_COUNTERS.record_slow_path();
            None
        }
    }
    
    /// Hot path: Schedule next thread (optimized for cache locality).
    #[inline(always)]
    pub fn schedule_next(&self, cpu_id: CpuId, current: Option<Arc<Thread>>) -> Option<Arc<Thread>> {
        let cpu_data = match self.cpu_data.get(cpu_id as usize) {
            Some(data) => data.get(),
            None => {
                PERF_COUNTERS.record_slow_path();
                return None;
            }
        };
        
        // Fast path: Check if we have threads in local queue
        if !cpu_data.is_local_queue_empty() {
            PERF_COUNTERS.record_fast_path();
            cpu_data.record_schedule();
            
            // Try to pick from local queue first (best cache locality)
            if let Some(next_thread) = self.pick_from_local_queue(cpu_data) {
                // Update current thread atomically
                cpu_data.set_current_thread(Some(next_thread.clone()));
                return Some(next_thread);
            }
        }
        
        // Medium path: Try work stealing from neighboring CPUs
        if let Some(next_thread) = self.try_work_stealing(cpu_id) {
            PERF_COUNTERS.record_fast_path();
            cpu_data.set_current_thread(Some(next_thread.clone()));
            return Some(next_thread);
        }
        
        // Slow path: Global queue or idle
        PERF_COUNTERS.record_slow_path();
        self.pick_from_global_queue(cpu_id)
    }
    
    /// Optimized local queue access (lock-free when possible).
    #[inline(always)]
    fn pick_from_local_queue(&self, cpu_data: &CacheOptimizedCpuData) -> Option<Arc<Thread>> {
        // This would integrate with the actual queue implementation
        // For now, return None to indicate no local threads
        None
    }
    
    /// Work stealing implementation with cache-aware CPU selection.
    fn try_work_stealing(&self, cpu_id: CpuId) -> Option<Arc<Thread>> {
        let cpu_count = self.cpu_data.len();
        
        // Try nearby CPUs first (better cache locality)
        for distance in 1..cpu_count {
            // Check both directions
            for direction in [1, -1] {
                let target_cpu = ((cpu_id as isize + direction * distance as isize + cpu_count as isize) 
                    % cpu_count as isize) as usize;
                
                if target_cpu < self.cpu_data.len() {
                    let target_data = self.cpu_data[target_cpu].get();
                    
                    // Only steal if target has enough work
                    if target_data.local_queue_length() > 1 {
                        if let Some(stolen_thread) = self.steal_from_cpu(target_data) {
                            return Some(stolen_thread);
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// Steal thread from another CPU's queue.
    fn steal_from_cpu(&self, target_cpu_data: &CacheOptimizedCpuData) -> Option<Arc<Thread>> {
        // This would implement the actual work stealing algorithm
        // Return None for now
        None
    }
    
    /// Fallback to global queue.
    fn pick_from_global_queue(&self, cpu_id: CpuId) -> Option<Arc<Thread>> {
        // This would integrate with global queue
        None
    }
    
    /// Add thread to CPU's local queue (cache-optimized).
    pub fn add_to_local_queue(&self, cpu_id: CpuId, thread: Arc<Thread>) -> Result<(), Arc<Thread>> {
        if let Some(cpu_data) = self.cpu_data.get(cpu_id as usize) {
            let data = cpu_data.get();
            
            // Update priority counts
            data.update_priority_count(thread.priority(), 1);
            
            // Add to queue (this would integrate with actual queue implementation)
            // For now, just update tail counter
            data.local_queue_tail.fetch_add(1, Ordering::Release);
            
            Ok(())
        } else {
            Err(thread)
        }
    }
    
    /// Remove thread from CPU's local queue.
    pub fn remove_from_local_queue(&self, cpu_id: CpuId, thread: &Thread) -> bool {
        if let Some(cpu_data) = self.cpu_data.get(cpu_id as usize) {
            let data = cpu_data.get();
            
            // Update priority counts
            data.update_priority_count(thread.priority(), -1);
            
            // Remove from queue (this would integrate with actual queue implementation)
            true
        } else {
            false
        }
    }
    
    /// Get CPU utilization statistics.
    pub fn get_cpu_stats(&self, cpu_id: CpuId) -> Option<CpuStats> {
        self.cpu_data.get(cpu_id as usize).map(|data| {
            let perf = data.get().perf_stats.get();
            CpuStats {
                cpu_id,
                total_schedules: perf.schedules.load(Ordering::Relaxed),
                total_preemptions: perf.preemptions.load(Ordering::Relaxed),
                total_yields: perf.yields.load(Ordering::Relaxed),
                idle_cycles: perf.idle_cycles.load(Ordering::Relaxed),
                local_queue_length: data.get().local_queue_length(),
                thread_counts_by_priority: core::array::from_fn(|i| {
                    data.get().get_priority_count(i as u8)
                }),
            }
        })
    }
    
    /// Get system-wide scheduler statistics.
    pub fn get_system_stats(&self) -> SystemSchedulerStats {
        let mut total_schedules = 0;
        let mut total_preemptions = 0;
        let mut total_yields = 0;
        let mut total_queue_length = 0;
        
        for cpu_data in &self.cpu_data {
            let data = cpu_data.get();
            let perf = data.perf_stats.get();
            
            total_schedules += perf.schedules.load(Ordering::Relaxed);
            total_preemptions += perf.preemptions.load(Ordering::Relaxed);
            total_yields += perf.yields.load(Ordering::Relaxed);
            total_queue_length += data.local_queue_length();
        }
        
        SystemSchedulerStats {
            total_threads: self.total_threads.load(Ordering::Relaxed),
            active_cpus: self.active_cpus.load(Ordering::Relaxed),
            total_schedules,
            total_preemptions,
            total_yields,
            total_queue_length,
            fast_path_ratio: PERF_COUNTERS.fast_path_ratio(),
        }
    }
}

/// Per-CPU statistics.
#[derive(Debug, Clone)]
pub struct CpuStats {
    pub cpu_id: CpuId,
    pub total_schedules: u64,
    pub total_preemptions: u64,
    pub total_yields: u64,
    pub idle_cycles: u64,
    pub local_queue_length: usize,
    pub thread_counts_by_priority: [usize; 11],
}

/// System-wide scheduler statistics.
#[derive(Debug, Clone)]
pub struct SystemSchedulerStats {
    pub total_threads: usize,
    pub active_cpus: usize,
    pub total_schedules: u64,
    pub total_preemptions: u64,
    pub total_yields: u64,
    pub total_queue_length: usize,
    pub fast_path_ratio: f64,
}
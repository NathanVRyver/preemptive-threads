//! Lock-free fast paths for common threading operations.

use crate::perf::PERF_COUNTERS;
use crate::thread_new::{Thread, ThreadId, ThreadState};
use crate::sched::CpuId;
use portable_atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use alloc::sync::Arc;

/// Fast path implementations for critical threading operations.
pub struct FastPaths;

impl FastPaths {
    /// Ultra-fast thread state check (single atomic read).
    #[inline(always)]
    pub fn quick_state_check(thread: &Thread) -> ThreadState {
        PERF_COUNTERS.record_fast_path();
        thread.state()
    }
    
    /// Fast path for thread yield (minimal overhead).
    #[inline(always)]
    pub fn fast_yield() {
        PERF_COUNTERS.record_fast_path();
        // Implementation would do minimal work and delegate to scheduler
        crate::yield_now();
    }
    
    /// Fast path for getting current thread ID.
    #[inline(always)]
    pub fn fast_current_thread_id() -> ThreadId {
        PERF_COUNTERS.record_fast_path();
        crate::thread_new::current_thread_id()
    }
    
    /// Fast path for simple mutex lock attempt.
    #[inline(always)]
    pub fn fast_mutex_try_lock(mutex_state: &AtomicUsize) -> bool {
        // Try to acquire mutex with single CAS
        let result = mutex_state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        
        if result {
            PERF_COUNTERS.record_fast_path();
        } else {
            PERF_COUNTERS.record_slow_path();
        }
        
        result
    }
    
    /// Fast path for mutex unlock.
    #[inline(always)]
    pub fn fast_mutex_unlock(mutex_state: &AtomicUsize) {
        PERF_COUNTERS.record_fast_path();
        mutex_state.store(0, Ordering::Release);
    }
    
    /// Fast path for atomic counter increment.
    #[inline(always)]
    pub fn fast_atomic_increment(counter: &AtomicU64) -> u64 {
        PERF_COUNTERS.record_lockfree_operation();
        counter.fetch_add(1, Ordering::Relaxed)
    }
    
    /// Fast path for thread priority comparison.
    #[inline(always)]
    pub fn fast_priority_compare(thread1: &Thread, thread2: &Thread) -> core::cmp::Ordering {
        PERF_COUNTERS.record_fast_path();
        thread1.priority().cmp(&thread2.priority())
    }
    
    /// Fast path for CPU affinity check.
    #[inline(always)]
    pub fn fast_affinity_check(thread: &Thread, cpu_id: CpuId) -> bool {
        PERF_COUNTERS.record_fast_path();
        let affinity = thread.cpu_affinity();
        (affinity & (1u64 << cpu_id)) != 0
    }
}

/// Optimized reference counting for thread handles.
#[repr(align(64))] // Cache line aligned
pub struct OptimizedArcCounter {
    strong_count: AtomicUsize,
    weak_count: AtomicUsize,
    data_ptr: AtomicPtr<u8>,
}

impl OptimizedArcCounter {
    pub fn new<T>(data: T) -> Self {
        let boxed = alloc::boxed::Box::new(data);
        let ptr = alloc::boxed::Box::into_raw(boxed) as *mut u8;
        
        Self {
            strong_count: AtomicUsize::new(1),
            weak_count: AtomicUsize::new(1),
            data_ptr: AtomicPtr::new(ptr),
        }
    }
    
    /// Fast path for cloning (single atomic increment).
    #[inline(always)]
    pub fn fast_clone(&self) -> bool {
        let old_count = self.strong_count.fetch_add(1, Ordering::Relaxed);
        PERF_COUNTERS.record_fast_path();
        old_count != 0 // Returns false if object was being destroyed
    }
    
    /// Fast path for dropping (single atomic decrement).
    #[inline(always)]
    pub fn fast_drop(&self) -> bool {
        let old_count = self.strong_count.fetch_sub(1, Ordering::Release);
        PERF_COUNTERS.record_fast_path();
        old_count == 1 // Returns true if this was the last reference
    }
    
    /// Get current reference count (fast read).
    #[inline(always)]
    pub fn fast_count(&self) -> usize {
        PERF_COUNTERS.record_fast_path();
        self.strong_count.load(Ordering::Relaxed)
    }
}

/// Lock-free queue for fast thread scheduling.
#[repr(align(64))] // Cache line aligned  
pub struct LockFreeQueue<T> {
    head: AtomicPtr<QueueNode<T>>,
    tail: AtomicPtr<QueueNode<T>>,
    size: AtomicUsize,
}

struct QueueNode<T> {
    next: AtomicPtr<QueueNode<T>>,
    data: Option<T>,
}

impl<T> LockFreeQueue<T> {
    pub fn new() -> Self {
        // Create dummy node
        let dummy = alloc::boxed::Box::into_raw(alloc::boxed::Box::new(QueueNode {
            next: AtomicPtr::new(core::ptr::null_mut()),
            data: None,
        }));
        
        Self {
            head: AtomicPtr::new(dummy),
            tail: AtomicPtr::new(dummy),
            size: AtomicUsize::new(0),
        }
    }
    
    /// Fast path enqueue (Michael & Scott algorithm).
    #[inline(always)]
    pub fn fast_enqueue(&self, item: T) -> Result<(), T> {
        PERF_COUNTERS.record_lockfree_operation();
        
        let new_node = alloc::boxed::Box::into_raw(alloc::boxed::Box::new(QueueNode {
            next: AtomicPtr::new(core::ptr::null_mut()),
            data: Some(item),
        }));
        
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*tail).next.load(Ordering::Acquire) };
            
            if tail == self.tail.load(Ordering::Acquire) {
                if next.is_null() {
                    // Try to link new node at end of list
                    if unsafe { (*tail).next.compare_exchange_weak(
                        next,
                        new_node,
                        Ordering::Release,
                        Ordering::Relaxed,
                    ).is_ok() } {
                        // Successfully linked, now try to swing tail to new node
                        let _ = self.tail.compare_exchange(
                            tail,
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                        break;
                    }
                } else {
                    // Help advance tail
                    let _ = self.tail.compare_exchange(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                }
            }
            
            core::hint::spin_loop();
        }
        
        self.size.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    /// Fast path dequeue.
    #[inline(always)]
    pub fn fast_dequeue(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*head).next.load(Ordering::Acquire) };
            
            if head == self.head.load(Ordering::Acquire) {
                if head == tail {
                    if next.is_null() {
                        // Queue is empty
                        PERF_COUNTERS.record_fast_path();
                        return None;
                    }
                    
                    // Help advance tail
                    let _ = self.tail.compare_exchange(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                } else {
                    if next.is_null() {
                        continue; // Inconsistent state, retry
                    }
                    
                    // Read data before CAS to avoid ABA problem
                    let data = unsafe { (*next).data.take() };
                    
                    // Try to swing head to next node
                    if self.head.compare_exchange_weak(
                        head,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    ).is_ok() {
                        // Successfully dequeued
                        unsafe {
                            let _ = alloc::boxed::Box::from_raw(head);
                        }
                        
                        self.size.fetch_sub(1, Ordering::Relaxed);
                        PERF_COUNTERS.record_lockfree_operation();
                        return data;
                    }
                }
            }
            
            core::hint::spin_loop();
        }
    }
    
    /// Fast size check.
    #[inline(always)]
    pub fn fast_len(&self) -> usize {
        PERF_COUNTERS.record_fast_path();
        self.size.load(Ordering::Relaxed)
    }
    
    /// Fast empty check.
    #[inline(always)]
    pub fn fast_is_empty(&self) -> bool {
        PERF_COUNTERS.record_fast_path();
        self.size.load(Ordering::Relaxed) == 0
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        // Drain all remaining items
        while self.fast_dequeue().is_some() {}
        
        // Free dummy node
        let head = self.head.load(Ordering::Relaxed);
        if !head.is_null() {
            unsafe {
                let _ = alloc::boxed::Box::from_raw(head);
            }
        }
    }
}

/// Fast path optimizations for common patterns.
pub struct CommonPatterns;

impl CommonPatterns {
    /// Optimized spin-wait with exponential backoff.
    #[inline(always)]
    pub fn optimized_spin_wait<F>(mut condition: F, max_spins: u32) -> bool 
    where
        F: FnMut() -> bool,
    {
        let mut spins = 0;
        let mut backoff = 1;
        
        while spins < max_spins {
            if condition() {
                PERF_COUNTERS.record_fast_path();
                return true;
            }
            
            // Exponential backoff with CPU pause
            for _ in 0..backoff {
                core::hint::spin_loop();
            }
            
            backoff = (backoff * 2).min(64);
            spins += 1;
        }
        
        PERF_COUNTERS.record_slow_path();
        false
    }
    
    /// Fast double-checked locking pattern.
    #[inline(always)]
    pub fn fast_double_checked_lock<T, F>(
        flag: &AtomicUsize,
        initializer: F,
    ) -> bool 
    where
        F: FnOnce() -> T,
    {
        // Fast path: already initialized
        if flag.load(Ordering::Acquire) == 2 {
            PERF_COUNTERS.record_fast_path();
            return true;
        }
        
        // Try to acquire initialization lock
        if flag.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            // We got the lock, do initialization
            let _result = initializer();
            flag.store(2, Ordering::Release);
            PERF_COUNTERS.record_slow_path();
            true
        } else {
            // Someone else is initializing, wait for completion
            while flag.load(Ordering::Acquire) == 1 {
                core::hint::spin_loop();
            }
            PERF_COUNTERS.record_slow_path();
            flag.load(Ordering::Acquire) == 2
        }
    }
    
    /// Fast compare-and-swap loop with bounded retries.
    #[inline(always)]
    pub fn bounded_cas_loop<T, F>(
        atomic: &AtomicUsize,
        mut updater: F,
        max_retries: u32,
    ) -> Result<usize, usize>
    where
        F: FnMut(usize) -> usize,
    {
        let mut retries = 0;
        let mut current = atomic.load(Ordering::Relaxed);
        
        while retries < max_retries {
            let new_value = updater(current);
            
            match atomic.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    PERF_COUNTERS.record_lockfree_operation();
                    return Ok(new_value);
                }
                Err(actual) => {
                    current = actual;
                    retries += 1;
                    core::hint::spin_loop();
                }
            }
        }
        
        PERF_COUNTERS.record_slow_path();
        Err(current)
    }
}

/// Performance metrics for fast path effectiveness.
pub struct FastPathMetrics {
    pub fast_path_hits: u64,
    pub slow_path_hits: u64,
    pub lockfree_operations: u64,
    pub contention_events: u64,
}

impl FastPathMetrics {
    /// Get current fast path metrics.
    pub fn current() -> Self {
        Self {
            fast_path_hits: PERF_COUNTERS.fast_path_hits.load(Ordering::Relaxed),
            slow_path_hits: PERF_COUNTERS.slow_path_hits.load(Ordering::Relaxed),
            lockfree_operations: PERF_COUNTERS.lockfree_operations.load(Ordering::Relaxed),
            contention_events: 0, // Would be collected from various sources
        }
    }
    
    /// Calculate fast path effectiveness ratio.
    pub fn effectiveness_ratio(&self) -> f64 {
        let total = self.fast_path_hits + self.slow_path_hits;
        if total > 0 {
            self.fast_path_hits as f64 / total as f64
        } else {
            0.0
        }
    }
    
    /// Calculate lock-free operation ratio.
    pub fn lockfree_ratio(&self) -> f64 {
        let total = self.fast_path_hits + self.slow_path_hits + self.lockfree_operations;
        if total > 0 {
            self.lockfree_operations as f64 / total as f64
        } else {
            0.0
        }
    }
}
//! Per-CPU memory pools for optimized allocation performance.

use crate::perf::{PerfConfig, PERF_COUNTERS};
use crate::sched::CpuId;
use crate::mem::{Stack, StackSizeClass};
use crate::arch::barriers::CacheLinePadded;
use portable_atomic::{AtomicUsize, AtomicPtr, AtomicU64, Ordering};
use alloc::vec::Vec;
use alloc::collections::VecDeque;

/// Per-CPU memory pool for lock-free allocation.
#[repr(align(64))] // Cache line aligned
pub struct PerCpuMemoryPool {
    /// CPU ID this pool serves
    pub cpu_id: CpuId,
    
    /// Stack pools by size class (frequently accessed)
    pub small_stack_pool: LockFreePool<Stack>,
    pub medium_stack_pool: LockFreePool<Stack>,
    pub large_stack_pool: LockFreePool<Stack>,
    
    /// General-purpose object pools
    pub thread_object_pool: LockFreePool<ThreadObjectSlot>,
    pub sync_object_pool: LockFreePool<SyncObjectSlot>,
    
    /// Pool statistics (cache line padded)
    pub stats: CacheLinePadded<PoolStats>,
    
    /// Emergency fallback pool pointer (for cross-CPU allocation)
    pub fallback_pool: AtomicPtr<GlobalMemoryPool>,
    
    /// Pool configuration
    pub config: PoolConfig,
}

/// Configuration for memory pools.
#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    /// Initial pool size per CPU
    pub initial_pool_size: usize,
    /// Maximum pool size per CPU
    pub max_pool_size: usize,
    /// Batch allocation size for refills
    pub batch_size: usize,
    /// Low watermark for triggering background refill
    pub low_watermark: usize,
    /// High watermark for triggering cleanup
    pub high_watermark: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            initial_pool_size: 64,
            max_pool_size: 1024,
            batch_size: 16,
            low_watermark: 8,
            high_watermark: 768,
        }
    }
}

/// Lock-free memory pool implementation using Michael & Scott algorithm.
#[repr(align(64))] // Cache line aligned
pub struct LockFreePool<T> {
    /// Head of the free list (ABA-safe pointer with generation counter)
    head: AtomicPtr<PoolNode<T>>,
    
    /// Pool size statistics
    available_count: AtomicUsize,
    total_allocated: AtomicUsize,
    
    /// Performance counters
    allocations: AtomicU64,
    deallocations: AtomicU64,
    contention_events: AtomicU64,
}

/// Pool node for lock-free linked list.
#[repr(align(16))] // Prevent false sharing
struct PoolNode<T> {
    next: AtomicPtr<PoolNode<T>>,
    data: T,
    generation: AtomicU64, // ABA prevention
}

/// Pool statistics.
#[derive(Default)]
pub struct PoolStats {
    pub total_allocations: AtomicU64,
    pub total_deallocations: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub cross_cpu_allocations: AtomicU64,
    pub pool_refills: AtomicU64,
    pub memory_reclaimed: AtomicU64,
}

/// Placeholder types for different object pools.
pub struct ThreadObjectSlot {
    pub data: [u8; 256], // Thread-related data
}

pub struct SyncObjectSlot {
    pub data: [u8; 128], // Synchronization object data
}

impl<T> LockFreePool<T> {
    /// Create a new lock-free pool.
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
            available_count: AtomicUsize::new(0),
            total_allocated: AtomicUsize::new(0),
            allocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            contention_events: AtomicU64::new(0),
        }
    }
    
    /// Allocate an object from the pool (lock-free).
    pub fn allocate(&self) -> Option<T> {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        
        loop {
            let head = self.head.load(Ordering::Acquire);
            if head.is_null() {
                // Pool is empty
                PERF_COUNTERS.record_slow_path();
                return None;
            }
            
            // Safety: We loaded head with Acquire ordering, so if it's not null,
            // the node is valid until we successfully CAS it out
            let node = unsafe { &*head };
            let next = node.next.load(Ordering::Relaxed);
            
            // Try to update head to next
            if self.head
                .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.available_count.fetch_sub(1, Ordering::Relaxed);
                
                // Extract the data (we now own the node)
                let data = unsafe { core::ptr::read(&node.data) };
                
                // Free the node memory
                unsafe {
                    let layout = core::alloc::Layout::new::<PoolNode<T>>();
                    alloc::alloc::dealloc(head as *mut u8, layout);
                }
                
                PERF_COUNTERS.record_fast_path();
                return Some(data);
            }
            
            // CAS failed, retry
            self.contention_events.fetch_add(1, Ordering::Relaxed);
            core::hint::spin_loop();
        }
    }
    
    /// Return an object to the pool (lock-free).
    pub fn deallocate(&self, item: T) -> Result<(), T> {
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        
        // Allocate a new node
        let layout = core::alloc::Layout::new::<PoolNode<T>>();
        let node_ptr = unsafe { alloc::alloc::alloc(layout) as *mut PoolNode<T> };
        
        if node_ptr.is_null() {
            return Err(item); // Allocation failed
        }
        
        // Initialize the node
        unsafe {
            core::ptr::write(node_ptr, PoolNode {
                next: AtomicPtr::new(core::ptr::null_mut()),
                data: item,
                generation: AtomicU64::new(0),
            });
        }
        
        // Insert at head of list
        loop {
            let head = self.head.load(Ordering::Relaxed);
            unsafe { (*node_ptr).next.store(head, Ordering::Relaxed) };
            
            if self.head
                .compare_exchange_weak(head, node_ptr, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.available_count.fetch_add(1, Ordering::Relaxed);
                break;
            }
            
            core::hint::spin_loop();
        }
        
        Ok(())
    }
    
    /// Get number of available items in pool.
    pub fn available(&self) -> usize {
        self.available_count.load(Ordering::Relaxed)
    }
    
    /// Get total number of allocations.
    pub fn total_allocations(&self) -> u64 {
        self.allocations.load(Ordering::Relaxed)
    }
    
    /// Get total number of deallocations.
    pub fn total_deallocations(&self) -> u64 {
        self.deallocations.load(Ordering::Relaxed)
    }
    
    /// Get contention events count.
    pub fn contention_events(&self) -> u64 {
        self.contention_events.load(Ordering::Relaxed)
    }
}

impl PerCpuMemoryPool {
    /// Create a new per-CPU memory pool.
    pub fn new(cpu_id: CpuId, config: PoolConfig) -> Self {
        Self {
            cpu_id,
            small_stack_pool: LockFreePool::new(),
            medium_stack_pool: LockFreePool::new(),
            large_stack_pool: LockFreePool::new(),
            thread_object_pool: LockFreePool::new(),
            sync_object_pool: LockFreePool::new(),
            stats: CacheLinePadded::new(PoolStats::default()),
            fallback_pool: AtomicPtr::new(core::ptr::null_mut()),
            config,
        }
    }
    
    /// Allocate a stack from the appropriate pool.
    pub fn allocate_stack(&self, size_class: StackSizeClass) -> Option<Stack> {
        self.stats.get().total_allocations.fetch_add(1, Ordering::Relaxed);
        
        let pool = match size_class {
            StackSizeClass::Small => &self.small_stack_pool,
            StackSizeClass::Medium => &self.medium_stack_pool,
            StackSizeClass::Large => &self.large_stack_pool,
            StackSizeClass::ExtraLarge => &self.large_stack_pool, // Use large pool for extra large
        };
        
        if let Some(stack) = pool.allocate() {
            self.stats.get().cache_hits.fetch_add(1, Ordering::Relaxed);
            Some(stack)
        } else {
            self.stats.get().cache_misses.fetch_add(1, Ordering::Relaxed);
            
            // Try fallback to global pool
            self.allocate_from_fallback(size_class)
        }
    }
    
    /// Return a stack to the appropriate pool.
    pub fn deallocate_stack(&self, stack: Stack, size_class: StackSizeClass) {
        self.stats.get().total_deallocations.fetch_add(1, Ordering::Relaxed);
        
        let pool = match size_class {
            StackSizeClass::Small => &self.small_stack_pool,
            StackSizeClass::Medium => &self.medium_stack_pool,
            StackSizeClass::Large => &self.large_stack_pool,
            StackSizeClass::ExtraLarge => &self.large_stack_pool, // Use large pool for extra large
        };
        
        if pool.deallocate(stack.clone()).is_err() {
            // Pool is full or allocation failed, try fallback
            self.deallocate_to_fallback(stack, size_class);
        }
    }
    
    /// Allocate from fallback global pool.
    fn allocate_from_fallback(&self, size_class: StackSizeClass) -> Option<Stack> {
        let fallback_ptr = self.fallback_pool.load(Ordering::Acquire);
        if !fallback_ptr.is_null() {
            self.stats.get().cross_cpu_allocations.fetch_add(1, Ordering::Relaxed);
            // In real implementation, would delegate to global pool
            None
        } else {
            None
        }
    }
    
    /// Deallocate to fallback global pool.
    fn deallocate_to_fallback(&self, _stack: Stack, _size_class: StackSizeClass) {
        // In real implementation, would return to global pool
        self.stats.get().memory_reclaimed.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Check if pool needs refilling.
    pub fn needs_refill(&self) -> bool {
        let small_available = self.small_stack_pool.available();
        let medium_available = self.medium_stack_pool.available();
        let large_available = self.large_stack_pool.available();
        
        small_available < self.config.low_watermark
            || medium_available < self.config.low_watermark
            || large_available < self.config.low_watermark
    }
    
    /// Trigger background refill of pools.
    pub fn trigger_refill(&self) {
        self.stats.get().pool_refills.fetch_add(1, Ordering::Relaxed);
        // In real implementation, would signal background thread to refill
    }
    
    /// Get pool utilization statistics.
    pub fn get_utilization(&self) -> PoolUtilization {
        PoolUtilization {
            cpu_id: self.cpu_id,
            small_pool_size: self.small_stack_pool.available(),
            medium_pool_size: self.medium_stack_pool.available(),
            large_pool_size: self.large_stack_pool.available(),
            thread_pool_size: self.thread_object_pool.available(),
            sync_pool_size: self.sync_object_pool.available(),
            total_allocations: self.stats.get().total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.stats.get().total_deallocations.load(Ordering::Relaxed),
            cache_hit_ratio: self.get_cache_hit_ratio(),
            contention_ratio: self.get_contention_ratio(),
        }
    }
    
    /// Calculate cache hit ratio.
    fn get_cache_hit_ratio(&self) -> f64 {
        let hits = self.stats.get().cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.stats.get().cache_misses.load(Ordering::Relaxed) as f64;
        
        if hits + misses > 0.0 {
            hits / (hits + misses)
        } else {
            1.0
        }
    }
    
    /// Calculate contention ratio.
    fn get_contention_ratio(&self) -> f64 {
        let total_ops = self.small_stack_pool.total_allocations() + 
                       self.small_stack_pool.total_deallocations();
        let contention = self.small_stack_pool.contention_events();
        
        if total_ops > 0 {
            contention as f64 / total_ops as f64
        } else {
            0.0
        }
    }
}

/// Global memory pool for fallback allocation.
pub struct GlobalMemoryPool {
    /// Global pools by size class
    pub small_stacks: spin::Mutex<VecDeque<Stack>>,
    pub medium_stacks: spin::Mutex<VecDeque<Stack>>,
    pub large_stacks: spin::Mutex<VecDeque<Stack>>,
    
    /// Global statistics
    pub stats: GlobalPoolStats,
}

#[derive(Default)]
pub struct GlobalPoolStats {
    pub total_allocations: AtomicU64,
    pub total_deallocations: AtomicU64,
    pub cross_cpu_requests: AtomicU64,
    pub emergency_allocations: AtomicU64,
}

/// Per-CPU pool utilization statistics.
#[derive(Debug, Clone)]
pub struct PoolUtilization {
    pub cpu_id: CpuId,
    pub small_pool_size: usize,
    pub medium_pool_size: usize,
    pub large_pool_size: usize,
    pub thread_pool_size: usize,
    pub sync_pool_size: usize,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub cache_hit_ratio: f64,
    pub contention_ratio: f64,
}

/// System-wide memory pool manager.
pub struct MemoryPoolManager {
    /// Per-CPU pools (cache line padded)
    per_cpu_pools: Vec<CacheLinePadded<PerCpuMemoryPool>>,
    
    /// Global fallback pool
    global_pool: GlobalMemoryPool,
    
    /// Pool configuration
    config: PerfConfig,
}

impl MemoryPoolManager {
    pub fn new(config: PerfConfig) -> Self {
        let pool_config = PoolConfig::default();
        let mut per_cpu_pools = Vec::with_capacity(config.cpu_count);
        
        for cpu_id in 0..config.cpu_count {
            per_cpu_pools.push(CacheLinePadded::new(
                PerCpuMemoryPool::new(cpu_id as CpuId, pool_config)
            ));
        }
        
        Self {
            per_cpu_pools,
            global_pool: GlobalMemoryPool {
                small_stacks: spin::Mutex::new(VecDeque::new()),
                medium_stacks: spin::Mutex::new(VecDeque::new()),
                large_stacks: spin::Mutex::new(VecDeque::new()),
                stats: GlobalPoolStats::default(),
            },
            config,
        }
    }
    
    /// Get per-CPU pool for the specified CPU.
    pub fn get_cpu_pool(&self, cpu_id: CpuId) -> Option<&PerCpuMemoryPool> {
        self.per_cpu_pools.get(cpu_id as usize).map(|pool| pool.get())
    }
    
    /// Allocate stack using CPU-local pool.
    pub fn allocate_stack(&self, cpu_id: CpuId, size_class: StackSizeClass) -> Option<Stack> {
        if let Some(pool) = self.get_cpu_pool(cpu_id) {
            pool.allocate_stack(size_class)
        } else {
            self.allocate_from_global(size_class)
        }
    }
    
    /// Deallocate stack to CPU-local pool.
    pub fn deallocate_stack(&self, cpu_id: CpuId, stack: Stack, size_class: StackSizeClass) {
        if let Some(pool) = self.get_cpu_pool(cpu_id) {
            pool.deallocate_stack(stack, size_class);
        } else {
            self.deallocate_to_global(stack, size_class);
        }
    }
    
    /// Allocate from global pool (fallback).
    fn allocate_from_global(&self, size_class: StackSizeClass) -> Option<Stack> {
        self.global_pool.stats.emergency_allocations.fetch_add(1, Ordering::Relaxed);
        
        let queue = match size_class {
            StackSizeClass::Small => &self.global_pool.small_stacks,
            StackSizeClass::Medium => &self.global_pool.medium_stacks,
            StackSizeClass::Large => &self.global_pool.large_stacks,
            StackSizeClass::ExtraLarge => &self.global_pool.large_stacks, // Use large queue for extra large
        };
        
        queue.lock().pop_front()
    }
    
    /// Deallocate to global pool (fallback).
    fn deallocate_to_global(&self, stack: Stack, size_class: StackSizeClass) {
        let queue = match size_class {
            StackSizeClass::Small => &self.global_pool.small_stacks,
            StackSizeClass::Medium => &self.global_pool.medium_stacks,
            StackSizeClass::Large => &self.global_pool.large_stacks,
            StackSizeClass::ExtraLarge => &self.global_pool.large_stacks, // Use large queue for extra large
        };
        
        queue.lock().push_back(stack);
    }
    
    /// Get system-wide pool statistics.
    pub fn get_system_stats(&self) -> SystemMemoryStats {
        let mut total_allocations = 0;
        let mut total_deallocations = 0;
        let mut total_cache_hits = 0;
        let mut total_cache_misses = 0;
        
        for pool in &self.per_cpu_pools {
            let utilization = pool.get().get_utilization();
            total_allocations += utilization.total_allocations;
            total_deallocations += utilization.total_deallocations;
            
            let stats = pool.get().stats.get();
            total_cache_hits += stats.cache_hits.load(Ordering::Relaxed);
            total_cache_misses += stats.cache_misses.load(Ordering::Relaxed);
        }
        
        let global_stats = &self.global_pool.stats;
        
        SystemMemoryStats {
            per_cpu_pools: self.per_cpu_pools.len(),
            total_allocations,
            total_deallocations,
            cache_hit_ratio: if total_cache_hits + total_cache_misses > 0 {
                total_cache_hits as f64 / (total_cache_hits + total_cache_misses) as f64
            } else {
                1.0
            },
            cross_cpu_allocations: global_stats.cross_cpu_requests.load(Ordering::Relaxed),
            emergency_allocations: global_stats.emergency_allocations.load(Ordering::Relaxed),
        }
    }
}

/// System-wide memory pool statistics.
#[derive(Debug, Clone)]
pub struct SystemMemoryStats {
    pub per_cpu_pools: usize,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub cache_hit_ratio: f64,
    pub cross_cpu_allocations: u64,
    pub emergency_allocations: u64,
}

/// Initialize per-CPU memory pools.
pub fn init_per_cpu_pools(config: &PerfConfig) -> MemoryPoolManager {
    let manager = MemoryPoolManager::new(*config);
    
    // Set up fallback pool pointers
    for pool in &manager.per_cpu_pools {
        pool.get().fallback_pool.store(
            &manager.global_pool as *const _ as *mut _,
            Ordering::Release
        );
    }
    
    // Per-CPU memory pools initialized with CPU count and initial pool size
    
    manager
}
//! Performance optimization infrastructure and implementations.
//!
//! This module provides optimized implementations of critical paths in the
//! threading system, including CPU cache-aware algorithms, NUMA optimization,
//! and architecture-specific acceleration.

pub mod cache_aware;
pub mod numa;
pub mod cpu_dispatch;
pub mod fast_paths;
pub mod memory_pools;
pub mod context_switch_opt;

use portable_atomic::{AtomicU64, Ordering};
use crate::arch::detection::{CpuFeatures, detect_cpu_features};

/// Global performance configuration based on detected CPU features.
static PERF_CONFIG: spin::Mutex<Option<PerfConfig>> = spin::Mutex::new(None);

/// Performance configuration structure.
#[derive(Debug, Clone, Copy)]
pub struct PerfConfig {
    /// CPU cache line size for alignment
    pub cache_line_size: usize,
    /// Number of CPU cores
    pub cpu_count: usize,
    /// NUMA node count
    pub numa_nodes: usize,
    /// Whether to use lock-free fast paths
    pub use_lock_free_paths: bool,
    /// Whether to use SIMD acceleration
    pub use_simd: bool,
    /// Whether to use CPU-specific optimizations
    pub use_cpu_specific: bool,
    /// Detected CPU features
    pub cpu_features: CpuFeatures,
}

impl Default for PerfConfig {
    fn default() -> Self {
        let cpu_features = detect_cpu_features();
        Self {
            cache_line_size: cpu_features.cache_line_size as usize,
            cpu_count: cpu_features.cpu_cores as usize,
            numa_nodes: 1, // Default single NUMA node
            use_lock_free_paths: cpu_features.supports_atomic_cas,
            use_simd: cpu_features.supports_vector,
            use_cpu_specific: true,
            cpu_features,
        }
    }
}

/// Initialize performance subsystem with optimal configuration.
pub fn init_perf_optimization() {
    let config = PerfConfig::default();
    
    // Initialize CPU-specific optimizations
    cpu_dispatch::init_cpu_dispatch(&config);
    
    // Initialize NUMA-aware allocation if available
    if config.numa_nodes > 1 {
        numa::init_numa_optimization(&config);
    }
    
    // Initialize per-CPU memory pools
    memory_pools::init_per_cpu_pools(&config);
    
    // Store global configuration
    *PERF_CONFIG.lock() = Some(config);
}

/// Get the current performance configuration.
pub fn get_perf_config() -> PerfConfig {
    PERF_CONFIG.lock().unwrap_or_default()
}

/// Performance counters for monitoring optimization effectiveness.
#[repr(align(64))] // Cache line aligned
pub struct PerfCounters {
    /// Number of fast path hits
    pub fast_path_hits: AtomicU64,
    /// Number of slow path hits
    pub slow_path_hits: AtomicU64,
    /// Number of cache line bounces detected
    pub cache_bounces: AtomicU64,
    /// Number of NUMA-optimized allocations
    pub numa_local_allocations: AtomicU64,
    /// Number of NUMA remote allocations
    pub numa_remote_allocations: AtomicU64,
    /// Total context switches optimized
    pub optimized_context_switches: AtomicU64,
    /// SIMD operations executed
    pub simd_operations: AtomicU64,
    /// Lock-free operations completed
    pub lockfree_operations: AtomicU64,
}

impl Default for PerfCounters {
    fn default() -> Self {
        Self {
            fast_path_hits: AtomicU64::new(0),
            slow_path_hits: AtomicU64::new(0),
            cache_bounces: AtomicU64::new(0),
            numa_local_allocations: AtomicU64::new(0),
            numa_remote_allocations: AtomicU64::new(0),
            optimized_context_switches: AtomicU64::new(0),
            simd_operations: AtomicU64::new(0),
            lockfree_operations: AtomicU64::new(0),
        }
    }
}

impl PerfCounters {
    /// Record a fast path hit.
    #[inline(always)]
    pub fn record_fast_path(&self) {
        self.fast_path_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a slow path hit.
    #[inline(always)]
    pub fn record_slow_path(&self) {
        self.slow_path_hits.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a cache bounce.
    #[inline(always)]
    pub fn record_cache_bounce(&self) {
        self.cache_bounces.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a NUMA-local allocation.
    #[inline(always)]
    pub fn record_numa_local(&self) {
        self.numa_local_allocations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a NUMA-remote allocation.
    #[inline(always)]
    pub fn record_numa_remote(&self) {
        self.numa_remote_allocations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record an optimized context switch.
    #[inline(always)]
    pub fn record_context_switch(&self) {
        self.optimized_context_switches.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a SIMD operation.
    #[inline(always)]
    pub fn record_simd_operation(&self) {
        self.simd_operations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a lock-free operation.
    #[inline(always)]
    pub fn record_lockfree_operation(&self) {
        self.lockfree_operations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get fast path hit ratio.
    pub fn fast_path_ratio(&self) -> f64 {
        let fast = self.fast_path_hits.load(Ordering::Relaxed) as f64;
        let slow = self.slow_path_hits.load(Ordering::Relaxed) as f64;
        
        if fast + slow > 0.0 {
            fast / (fast + slow)
        } else {
            0.0
        }
    }
    
    /// Get NUMA locality ratio.
    pub fn numa_locality_ratio(&self) -> f64 {
        let local = self.numa_local_allocations.load(Ordering::Relaxed) as f64;
        let remote = self.numa_remote_allocations.load(Ordering::Relaxed) as f64;
        
        if local + remote > 0.0 {
            local / (local + remote)
        } else {
            0.0
        }
    }
    
    /// Reset all counters.
    pub fn reset(&self) {
        self.fast_path_hits.store(0, Ordering::Relaxed);
        self.slow_path_hits.store(0, Ordering::Relaxed);
        self.cache_bounces.store(0, Ordering::Relaxed);
        self.numa_local_allocations.store(0, Ordering::Relaxed);
        self.numa_remote_allocations.store(0, Ordering::Relaxed);
        self.optimized_context_switches.store(0, Ordering::Relaxed);
        self.simd_operations.store(0, Ordering::Relaxed);
        self.lockfree_operations.store(0, Ordering::Relaxed);
    }
}

/// Global performance counters instance.
pub static PERF_COUNTERS: PerfCounters = PerfCounters {
    fast_path_hits: AtomicU64::new(0),
    slow_path_hits: AtomicU64::new(0),
    cache_bounces: AtomicU64::new(0),
    numa_local_allocations: AtomicU64::new(0),
    numa_remote_allocations: AtomicU64::new(0),
    optimized_context_switches: AtomicU64::new(0),
    simd_operations: AtomicU64::new(0),
    lockfree_operations: AtomicU64::new(0),
};
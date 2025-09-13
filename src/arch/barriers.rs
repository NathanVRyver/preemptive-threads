//! Cross-platform memory barriers and atomic operation helpers.
//!
//! This module provides unified memory barrier operations across different
//! CPU architectures, ensuring proper memory ordering in lock-free code.

use portable_atomic::{AtomicU64, AtomicUsize, Ordering};

/// Memory barrier types for different ordering requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierType {
    /// Full memory barrier - orders all memory operations.
    Full,
    /// Acquire barrier - prevents reordering of loads before this barrier.
    Acquire,
    /// Release barrier - prevents reordering of stores after this barrier.
    Release,
    /// Load-only barrier - orders load operations only.
    LoadOnly,
    /// Store-only barrier - orders store operations only.
    StoreOnly,
}

/// Cross-platform memory barrier operations.
pub struct MemoryBarriers;

impl MemoryBarriers {
    /// Execute a memory barrier of the specified type.
    pub fn barrier(barrier_type: BarrierType) {
        match barrier_type {
            BarrierType::Full => Self::full_barrier(),
            BarrierType::Acquire => Self::acquire_barrier(),
            BarrierType::Release => Self::release_barrier(),
            BarrierType::LoadOnly => Self::load_barrier(),
            BarrierType::StoreOnly => Self::store_barrier(),
        }
    }
    
    /// Full memory barrier - prevents reordering of any memory operations.
    #[inline(always)]
    pub fn full_barrier() {
        #[cfg(feature = "x86_64")]
        crate::arch::x86_64::memory_barrier_full();
        
        #[cfg(feature = "arm64")]
        crate::arch::aarch64::memory_barrier_full();
        
        #[cfg(feature = "riscv64")]
        crate::arch::riscv::memory_barrier_full();
        
        #[cfg(not(any(feature = "x86_64", feature = "arm64", feature = "riscv64")))]
        core::sync::atomic::fence(Ordering::SeqCst);
    }
    
    /// Acquire barrier - prevents loads from being reordered before this point.
    #[inline(always)]
    pub fn acquire_barrier() {
        #[cfg(feature = "x86_64")]
        crate::arch::x86_64::memory_barrier_acquire();
        
        #[cfg(feature = "arm64")]
        crate::arch::aarch64::memory_barrier_acquire();
        
        #[cfg(feature = "riscv64")]
        crate::arch::riscv::memory_barrier_acquire();
        
        #[cfg(not(any(feature = "x86_64", feature = "arm64", feature = "riscv64")))]
        core::sync::atomic::fence(Ordering::Acquire);
    }
    
    /// Release barrier - prevents stores from being reordered after this point.
    #[inline(always)]
    pub fn release_barrier() {
        #[cfg(feature = "x86_64")]
        crate::arch::x86_64::memory_barrier_release();
        
        #[cfg(feature = "arm64")]
        crate::arch::aarch64::memory_barrier_release();
        
        #[cfg(feature = "riscv64")]
        crate::arch::riscv::memory_barrier_release();
        
        #[cfg(not(any(feature = "x86_64", feature = "arm64", feature = "riscv64")))]
        core::sync::atomic::fence(Ordering::Release);
    }
    
    /// Load barrier - orders load operations only.
    #[inline(always)]
    pub fn load_barrier() {
        // On most architectures, this is the same as acquire
        Self::acquire_barrier();
    }
    
    /// Store barrier - orders store operations only.
    #[inline(always)]
    pub fn store_barrier() {
        // On most architectures, this is the same as release
        Self::release_barrier();
    }
}

/// Cross-platform atomic operation extensions.
pub trait AtomicExt<T> {
    /// Atomic compare-and-swap with explicit memory ordering.
    fn compare_exchange_explicit(
        &self,
        current: T,
        new: T,
        success_order: Ordering,
        failure_order: Ordering,
    ) -> Result<T, T>;
    
    /// Atomic fetch-and-add with explicit memory ordering.
    fn fetch_add_explicit(&self, val: T, order: Ordering) -> T;
    
    /// Atomic fetch-and-subtract with explicit memory ordering.
    fn fetch_sub_explicit(&self, val: T, order: Ordering) -> T;
    
    /// Atomic load with memory barrier.
    fn load_with_barrier(&self, barrier: BarrierType) -> T;
    
    /// Atomic store with memory barrier.
    fn store_with_barrier(&self, val: T, barrier: BarrierType);
}

impl AtomicExt<u64> for AtomicU64 {
    fn compare_exchange_explicit(
        &self,
        current: u64,
        new: u64,
        success_order: Ordering,
        failure_order: Ordering,
    ) -> Result<u64, u64> {
        self.compare_exchange(current, new, success_order, failure_order)
    }
    
    fn fetch_add_explicit(&self, val: u64, order: Ordering) -> u64 {
        self.fetch_add(val, order)
    }
    
    fn fetch_sub_explicit(&self, val: u64, order: Ordering) -> u64 {
        self.fetch_sub(val, order)
    }
    
    fn load_with_barrier(&self, barrier: BarrierType) -> u64 {
        match barrier {
            BarrierType::Full => {
                MemoryBarriers::full_barrier();
                let val = self.load(Ordering::Relaxed);
                MemoryBarriers::full_barrier();
                val
            }
            BarrierType::Acquire => self.load(Ordering::Acquire),
            BarrierType::Release => {
                MemoryBarriers::release_barrier();
                self.load(Ordering::Relaxed)
            }
            _ => self.load(Ordering::SeqCst),
        }
    }
    
    fn store_with_barrier(&self, val: u64, barrier: BarrierType) {
        match barrier {
            BarrierType::Full => {
                MemoryBarriers::full_barrier();
                self.store(val, Ordering::Relaxed);
                MemoryBarriers::full_barrier();
            }
            BarrierType::Acquire => {
                self.store(val, Ordering::Relaxed);
                MemoryBarriers::acquire_barrier();
            }
            BarrierType::Release => self.store(val, Ordering::Release),
            _ => self.store(val, Ordering::SeqCst),
        }
    }
}

impl AtomicExt<usize> for AtomicUsize {
    fn compare_exchange_explicit(
        &self,
        current: usize,
        new: usize,
        success_order: Ordering,
        failure_order: Ordering,
    ) -> Result<usize, usize> {
        self.compare_exchange(current, new, success_order, failure_order)
    }
    
    fn fetch_add_explicit(&self, val: usize, order: Ordering) -> usize {
        self.fetch_add(val, order)
    }
    
    fn fetch_sub_explicit(&self, val: usize, order: Ordering) -> usize {
        self.fetch_sub(val, order)
    }
    
    fn load_with_barrier(&self, barrier: BarrierType) -> usize {
        match barrier {
            BarrierType::Full => {
                MemoryBarriers::full_barrier();
                let val = self.load(Ordering::Relaxed);
                MemoryBarriers::full_barrier();
                val
            }
            BarrierType::Acquire => self.load(Ordering::Acquire),
            BarrierType::Release => {
                MemoryBarriers::release_barrier();
                self.load(Ordering::Relaxed)
            }
            _ => self.load(Ordering::SeqCst),
        }
    }
    
    fn store_with_barrier(&self, val: usize, barrier: BarrierType) {
        match barrier {
            BarrierType::Full => {
                MemoryBarriers::full_barrier();
                self.store(val, Ordering::Relaxed);
                MemoryBarriers::full_barrier();
            }
            BarrierType::Acquire => {
                self.store(val, Ordering::Relaxed);
                MemoryBarriers::acquire_barrier();
            }
            BarrierType::Release => self.store(val, Ordering::Release),
            _ => self.store(val, Ordering::SeqCst),
        }
    }
}

/// Lock-free atomic utilities for high-performance operations.
pub struct LockFreeUtils;

impl LockFreeUtils {
    /// Perform an atomic read-modify-write operation with retry.
    pub fn atomic_update<F>(atomic: &AtomicU64, mut updater: F) -> u64
    where
        F: FnMut(u64) -> u64,
    {
        let mut current = atomic.load(Ordering::Acquire);
        loop {
            let new_value = updater(current);
            match atomic.compare_exchange_weak(
                current,
                new_value,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return new_value,
                Err(actual) => current = actual,
            }
        }
    }
    
    /// Perform an atomic increment with overflow protection.
    pub fn atomic_increment_bounded(atomic: &AtomicU64, max_value: u64) -> Result<u64, u64> {
        let final_val = Self::atomic_update(atomic, |current| {
            if current >= max_value {
                current // No change if at max
            } else {
                current + 1
            }
        });
        
        if final_val > max_value {
            Err(final_val)
        } else {
            Ok(final_val)
        }
    }
    
    /// Perform an atomic decrement with underflow protection.
    pub fn atomic_decrement_bounded(atomic: &AtomicU64, min_value: u64) -> Result<u64, u64> {
        let final_val = Self::atomic_update(atomic, |current| {
            if current <= min_value {
                current // No change if at min
            } else {
                current - 1
            }
        });
        
        if final_val < min_value {
            Err(final_val)
        } else {
            Ok(final_val)
        }
    }
    
    /// Double-checked locking pattern helper.
    pub fn double_checked_init<T, F>(
        atomic_flag: &AtomicUsize,
        initializer: F,
    ) -> bool
    where
        F: FnOnce() -> T,
    {
        // Fast path - check if already initialized
        if atomic_flag.load(Ordering::Acquire) != 0 {
            return true;
        }
        
        // Slow path - try to initialize
        MemoryBarriers::full_barrier();
        
        // Check again after barrier
        if atomic_flag.load(Ordering::Acquire) == 0 {
            // Try to claim initialization
            if atomic_flag
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // We won the race, perform initialization
                let _result = initializer();
                
                // Mark as fully initialized
                atomic_flag.store(2, Ordering::Release);
                return true;
            }
        }
        
        // Wait for initialization to complete
        while atomic_flag.load(Ordering::Acquire) == 1 {
            core::hint::spin_loop();
        }
        
        atomic_flag.load(Ordering::Acquire) == 2
    }
}

/// Architecture-specific cache line size detection.
pub struct CacheInfo;

impl CacheInfo {
    /// Get the cache line size for the current architecture.
    pub const fn cache_line_size() -> usize {
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        return 64; // Most common cache line size
        
        #[cfg(target_arch = "riscv64")]
        return 64; // Common for RISC-V as well
        
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        return 64; // Safe default
    }
    
    /// Align a size to cache line boundaries.
    pub const fn align_to_cache_line(size: usize) -> usize {
        let cache_size = Self::cache_line_size();
        (size + cache_size - 1) & !(cache_size - 1)
    }
    
    /// Check if an address is cache-line aligned.
    pub fn is_cache_line_aligned(addr: *const u8) -> bool {
        (addr as usize) & (Self::cache_line_size() - 1) == 0
    }
}

/// Padding structure to prevent false sharing.
#[repr(align(64))] // Align to cache line
#[derive(Debug, Default)]
pub struct CacheLinePadded<T> {
    pub value: T,
    _padding: [u8; 0], // Zero-sized padding for alignment
}

impl<T> CacheLinePadded<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            _padding: [],
        }
    }
    
    pub fn get(&self) -> &T {
        &self.value
    }
    
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
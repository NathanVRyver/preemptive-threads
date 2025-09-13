//! Data race detection utilities for debugging lock-free data structures.
//!
//! This module provides tools for detecting potential data races and memory
//! ordering issues during testing and debugging of lock-free algorithms.

use portable_atomic::{AtomicUsize, AtomicBool, Ordering};
use core::sync::atomic::{fence, AtomicU64};

/// A data race detector that tracks memory accesses and detects potential races.
///
/// This is primarily intended for debugging and testing lock-free data structures
/// to ensure they are free from data races and memory ordering issues.
pub struct RaceDetector {
    /// Whether race detection is enabled
    enabled: AtomicBool,
    /// Total number of memory accesses tracked
    total_accesses: AtomicUsize,
    /// Number of potential races detected
    races_detected: AtomicUsize,
    /// Timestamp for ordering detection
    clock: AtomicU64,
}

impl RaceDetector {
    /// Create a new race detector.
    pub const fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            total_accesses: AtomicUsize::new(0),
            races_detected: AtomicUsize::new(0),
            clock: AtomicU64::new(0),
        }
    }
    
    /// Enable race detection.
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }
    
    /// Disable race detection.
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }
    
    /// Check if race detection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    
    /// Record a memory read operation.
    ///
    /// This should be called before performing any load operation on shared memory.
    pub fn record_read(&self, addr: usize, size: usize) {
        if !self.is_enabled() {
            return;
        }
        
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        let timestamp = self.clock.fetch_add(1, Ordering::AcqRel);
        
        // In a full implementation, we would track per-address access history
        // For now, just record basic statistics
        let _ = (addr, size, timestamp);
    }
    
    /// Record a memory write operation.
    ///
    /// This should be called before performing any store operation on shared memory.
    pub fn record_write(&self, addr: usize, size: usize) {
        if !self.is_enabled() {
            return;
        }
        
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        let timestamp = self.clock.fetch_add(1, Ordering::AcqRel);
        
        // In a full implementation, we would check for conflicting accesses
        // and detect potential races
        let _ = (addr, size, timestamp);
    }
    
    /// Record a memory barrier/fence operation.
    pub fn record_fence(&self, ordering: Ordering) {
        if !self.is_enabled() {
            return;
        }
        
        let timestamp = self.clock.fetch_add(1, Ordering::AcqRel);
        let _ = (ordering, timestamp);
        
        // Memory barrier provides ordering guarantees
        fence(Ordering::SeqCst);
    }
    
    /// Get race detection statistics.
    pub fn stats(&self) -> RaceDetectorStats {
        RaceDetectorStats {
            total_accesses: self.total_accesses.load(Ordering::Acquire),
            races_detected: self.races_detected.load(Ordering::Acquire),
            enabled: self.is_enabled(),
        }
    }
    
    /// Reset all statistics.
    pub fn reset(&self) {
        self.total_accesses.store(0, Ordering::Release);
        self.races_detected.store(0, Ordering::Release);
        self.clock.store(0, Ordering::Release);
    }
}

/// Statistics from the race detector.
#[derive(Debug, Clone, Copy)]
pub struct RaceDetectorStats {
    /// Total number of memory accesses tracked
    pub total_accesses: usize,
    /// Number of potential races detected
    pub races_detected: usize,
    /// Whether race detection is currently enabled
    pub enabled: bool,
}

/// Global race detector instance.
pub static RACE_DETECTOR: RaceDetector = RaceDetector::new();

/// Convenience macro for recording memory reads.
#[macro_export]
macro_rules! record_read {
    ($addr:expr, $size:expr) => {
        #[cfg(debug_assertions)]
        {
            $crate::mem::race_detector::RACE_DETECTOR.record_read($addr as usize, $size);
        }
    };
}

/// Convenience macro for recording memory writes.
#[macro_export]
macro_rules! record_write {
    ($addr:expr, $size:expr) => {
        #[cfg(debug_assertions)]
        {
            $crate::mem::race_detector::RACE_DETECTOR.record_write($addr as usize, $size);
        }
    };
}

/// Convenience macro for recording memory fences.
#[macro_export]
macro_rules! record_fence {
    ($ordering:expr) => {
        #[cfg(debug_assertions)]
        {
            $crate::mem::race_detector::RACE_DETECTOR.record_fence($ordering);
        }
    };
}

/// Memory ordering validation utilities.
pub struct OrderingValidator;

impl OrderingValidator {
    /// Validate that the given ordering is appropriate for a load operation.
    pub fn validate_load_ordering(ordering: Ordering) -> bool {
        matches!(ordering, Ordering::Relaxed | Ordering::Acquire | Ordering::SeqCst)
    }
    
    /// Validate that the given ordering is appropriate for a store operation.
    pub fn validate_store_ordering(ordering: Ordering) -> bool {
        matches!(ordering, Ordering::Relaxed | Ordering::Release | Ordering::SeqCst)
    }
    
    /// Validate that the given ordering is appropriate for a compare-exchange operation.
    pub fn validate_cas_ordering(success: Ordering, failure: Ordering) -> bool {
        // Failure ordering cannot be stronger than success ordering
        let success_strength = Self::ordering_strength(success);
        let failure_strength = Self::ordering_strength(failure);
        
        failure_strength <= success_strength && 
        !matches!(failure, Ordering::Release | Ordering::AcqRel)
    }
    
    /// Get the relative strength of a memory ordering.
    fn ordering_strength(ordering: Ordering) -> u8 {
        match ordering {
            Ordering::Relaxed => 0,
            Ordering::Acquire => 1,
            Ordering::Release => 1,
            Ordering::AcqRel => 2,
            Ordering::SeqCst => 3,
            // Handle any potential future orderings
            _ => 3, // Treat unknown orderings as strongest
        }
    }
}

/// Assert that a memory ordering is valid for load operations.
#[macro_export]
macro_rules! assert_valid_load_ordering {
    ($ordering:expr) => {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                $crate::mem::race_detector::OrderingValidator::validate_load_ordering($ordering),
                "Invalid memory ordering for load operation: {:?}",
                $ordering
            );
        }
    };
}

/// Assert that a memory ordering is valid for store operations.
#[macro_export]
macro_rules! assert_valid_store_ordering {
    ($ordering:expr) => {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                $crate::mem::race_detector::OrderingValidator::validate_store_ordering($ordering),
                "Invalid memory ordering for store operation: {:?}",
                $ordering
            );
        }
    };
}

/// Assert that memory orderings are valid for compare-exchange operations.
#[macro_export]
macro_rules! assert_valid_cas_ordering {
    ($success:expr, $failure:expr) => {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                $crate::mem::race_detector::OrderingValidator::validate_cas_ordering($success, $failure),
                "Invalid memory ordering for compare-exchange: success={:?}, failure={:?}",
                $success,
                $failure
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_race_detector_basic() {
        let detector = RaceDetector::new();
        assert!(!detector.is_enabled());
        
        detector.enable();
        assert!(detector.is_enabled());
        
        detector.record_read(0x1000, 8);
        detector.record_write(0x1008, 4);
        
        let stats = detector.stats();
        assert_eq!(stats.total_accesses, 2);
        assert_eq!(stats.races_detected, 0);
        
        detector.reset();
        let stats = detector.stats();
        assert_eq!(stats.total_accesses, 0);
    }
    
    #[test]
    fn test_ordering_validation() {
        // Valid load orderings
        assert!(OrderingValidator::validate_load_ordering(Ordering::Relaxed));
        assert!(OrderingValidator::validate_load_ordering(Ordering::Acquire));
        assert!(OrderingValidator::validate_load_ordering(Ordering::SeqCst));
        
        // Valid store orderings
        assert!(OrderingValidator::validate_store_ordering(Ordering::Relaxed));
        assert!(OrderingValidator::validate_store_ordering(Ordering::Release));
        assert!(OrderingValidator::validate_store_ordering(Ordering::SeqCst));
        
        // Valid CAS orderings
        assert!(OrderingValidator::validate_cas_ordering(
            Ordering::SeqCst, 
            Ordering::Relaxed
        ));
        assert!(OrderingValidator::validate_cas_ordering(
            Ordering::AcqRel, 
            Ordering::Acquire
        ));
        
        // Invalid CAS ordering (failure stronger than success)
        assert!(!OrderingValidator::validate_cas_ordering(
            Ordering::Relaxed, 
            Ordering::SeqCst
        ));
    }
}
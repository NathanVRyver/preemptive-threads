//! Memory management abstractions for thread safety.
//!
//! This module provides safe abstractions for managing thread stacks,
//! reference counting, and memory reclamation in a no_std environment.

pub mod stack_pool;
pub mod arc_lite;

// Epoch-based reclamation for lock-free data structures
#[cfg(feature = "work-stealing")]
pub mod epoch;

// Hazard pointers for fine-grained memory reclamation
#[cfg(feature = "work-stealing")]
pub mod hazard;

// Data race detection utilities for debugging
#[cfg(debug_assertions)]
pub mod race_detector;

pub use stack_pool::{Stack, StackPool, StackSizeClass};
pub use arc_lite::ArcLite;

#[cfg(feature = "work-stealing")]
pub use epoch::{Guard, Atomic, pin_thread, unpin_thread};

#[cfg(feature = "work-stealing")]
pub use hazard::{HazardPointer, HazardAtomic, init_thread as hazard_init_thread, cleanup_thread as hazard_cleanup_thread};

#[cfg(debug_assertions)]
pub use race_detector::{RaceDetector, RaceDetectorStats, OrderingValidator, RACE_DETECTOR};
//! Memory management abstractions for thread safety.
//!
//! This module provides safe abstractions for managing thread stacks,
//! reference counting, and memory reclamation in a no_std environment.

pub mod stack_pool;
pub mod arc_lite;

#[cfg(feature = "work-stealing")]
pub mod epoch;

pub use stack_pool::{Stack, StackPool, StackSizeClass};
pub use arc_lite::ArcLite;
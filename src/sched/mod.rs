//! New scheduler implementations with lock-free data structures.
//!
//! This module provides high-performance schedulers that use lock-free
//! algorithms to minimize contention and improve scalability.

pub mod trait_def;
pub mod rr;
#[cfg(feature = "work-stealing")]
pub mod worksteal;

pub use trait_def::{Scheduler, CpuId, priority};
pub use rr::RoundRobinScheduler;

#[cfg(feature = "work-stealing")]
pub use worksteal::WorkStealingScheduler;

/// Default scheduler selection based on available features.
#[cfg(feature = "work-stealing")]
pub type DefaultScheduler = WorkStealingScheduler;

#[cfg(not(feature = "work-stealing"))]
pub type DefaultScheduler = RoundRobinScheduler;
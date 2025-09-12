#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![forbid(missing_docs, unreachable_pub)]

//! A `no_std` preemptive multithreading library built from scratch for OS kernels and embedded systems.
//!
//! This library provides preemptive multithreading capabilities without requiring the standard library,
//! making it suitable for embedded systems, OS kernels, and other resource-constrained environments.
//!
//! # Features
//!
//! - `std-shim`: Enable compatibility layer for standard library
//! - `x86_64`: Enable x86_64 architecture support  
//! - `arm64`: Enable ARM64 architecture support
//! - `riscv64`: Enable RISC-V 64-bit architecture support
//! - `full-fpu`: Enable full floating point unit save/restore
//! - `mmu`: Enable memory management unit features like guard pages
//! - `work-stealing`: Enable work-stealing scheduler implementation
//! - `hardened`: Enable security hardening features
//!
//! # Architecture
//!
//! The library is organized around several key abstractions:
//! - Architecture-specific context switching and interrupt handling
//! - Pluggable schedulers with different algorithms
//! - Safe memory management for thread stacks and resources
//! - Preemptive scheduling with configurable time slices

pub mod arch;
pub mod atomic_scheduler;
pub mod context;
pub mod context_full;
pub mod error;
pub mod kernel;
pub mod platform_timer;
pub mod preemption;
pub mod safe_api;
pub mod scheduler;
pub mod signal_safe;
pub mod stack_guard;
pub mod sync;
pub mod thread;

#[cfg(all(test, feature = "std"))]
mod tests;

#[cfg(test)]
extern crate std;

#[cfg(all(not(test), not(feature = "std")))]
use core::panic::PanicInfo;

#[cfg(all(not(test), not(feature = "std")))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub use atomic_scheduler::{AtomicScheduler, ATOMIC_SCHEDULER};
pub use error::{ThreadError, ThreadResult};  
pub use platform_timer::{init_preemption_timer, stop_preemption_timer, preemption_checkpoint};
pub use safe_api::{
    exit_thread as safe_exit, yield_now, Mutex, MutexGuard, ThreadBuilder, ThreadHandle, ThreadPool,
};
pub use scheduler::{Scheduler, SCHEDULER};
pub use stack_guard::{ProtectedStack, StackGuard, StackStats, StackStatus};
pub use sync::{exit_thread, yield_thread};
pub use thread::{Thread, ThreadId, ThreadState};

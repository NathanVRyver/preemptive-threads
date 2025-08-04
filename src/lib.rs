#![no_std]

pub mod context;
pub mod context_full;
pub mod error;
pub mod preemption;
pub mod scheduler;
pub mod atomic_scheduler;
pub mod signal_safe;
pub mod stack_guard;
pub mod safe_api;
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

pub use error::{ThreadError, ThreadResult};
pub use scheduler::{Scheduler, SCHEDULER};
pub use atomic_scheduler::{AtomicScheduler, ATOMIC_SCHEDULER};
pub use sync::{exit_thread, yield_thread};
pub use thread::{Thread, ThreadId, ThreadState};
pub use safe_api::{ThreadBuilder, ThreadHandle, Mutex, MutexGuard, ThreadPool, yield_now, exit_thread as safe_exit};
pub use stack_guard::{ProtectedStack, StackGuard, StackStatus, StackStats};

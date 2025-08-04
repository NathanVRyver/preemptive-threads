#![no_std]

pub mod atomic_scheduler;
pub mod context;
pub mod context_full;
pub mod error;
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

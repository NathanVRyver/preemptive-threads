#![no_std]

pub mod context;
pub mod error;
pub mod preemption;
pub mod scheduler;
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
pub use sync::{exit_thread, yield_thread};
pub use thread::{Thread, ThreadId, ThreadState};

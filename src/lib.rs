#![no_std]

pub mod thread;
pub mod scheduler;
pub mod context;
pub mod sync;
pub mod preemption;
pub mod error;

#[cfg(all(test, feature = "std"))]
mod tests;

#[cfg(test)]
extern crate std;


#[cfg(all(not(test), not(feature = "std")))]
use core::panic::PanicInfo;

#[panic_handler]
#[cfg(all(not(test), not(feature = "std")))]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub use thread::{Thread, ThreadId, ThreadState};
pub use scheduler::{Scheduler, SCHEDULER};
pub use sync::{yield_thread, exit_thread};
pub use error::{ThreadError, ThreadResult};
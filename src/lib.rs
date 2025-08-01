#![no_std]
#![feature(naked_functions)]

pub mod thread;
pub mod scheduler;
pub mod context;
pub mod sync;

#[cfg(test)]
extern crate std;

use core::panic::PanicInfo;

#[panic_handler]
#[cfg(not(test))]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

pub use thread::{Thread, ThreadId, ThreadState};
pub use scheduler::{Scheduler, SCHEDULER};
pub use sync::{yield_thread, exit_thread};
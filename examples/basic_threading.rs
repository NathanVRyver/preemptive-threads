//! Basic threading example showing cooperative multitasking
//!
//! This example demonstrates:
//! - Creating multiple threads with different priorities
//! - Cooperative yielding between threads
//! - Basic thread lifecycle management

extern crate preemptive_threads;

use preemptive_threads::{
    scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut STACK1: [u8; 32 * 1024] = [0; 32 * 1024];
static mut STACK2: [u8; 32 * 1024] = [0; 32 * 1024];

fn print_str(msg: &[u8]) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    unsafe {
        write(1, msg.as_ptr(), msg.len());
    }
}

fn worker_thread_1() {
    for i in 0..5 {
        let msg = match i {
            0 => b"Worker 1: Starting work\n" as &[u8],
            1 => b"Worker 1: Processing data\n" as &[u8],
            2 => b"Worker 1: Computing results\n" as &[u8],
            3 => b"Worker 1: Finalizing\n" as &[u8],
            _ => b"Worker 1: Work complete\n" as &[u8],
        };
        print_str(msg);
        yield_thread();
    }
}

fn worker_thread_2() {
    for i in 0..5 {
        let msg = match i {
            0 => b"Worker 2: Initializing\n" as &[u8],
            1 => b"Worker 2: Loading resources\n" as &[u8],
            2 => b"Worker 2: Executing task\n" as &[u8],
            3 => b"Worker 2: Cleaning up\n" as &[u8],
            _ => b"Worker 2: Task finished\n" as &[u8],
        };
        print_str(msg);
        yield_thread();
    }
}

fn main() {
    print_str(b"=== Basic Threading Example ===\n");
    print_str(b"Creating two cooperative threads...\n\n");

    unsafe {
        let scheduler = SCHEDULER.get();

        // Spawn threads with equal priority
        scheduler
            .spawn_thread(&mut STACK1, worker_thread_1, 1)
            .unwrap();
        scheduler
            .spawn_thread(&mut STACK2, worker_thread_2, 1)
            .unwrap();

        // Run the scheduler
        let mut active_threads = 2;
        while active_threads > 0 {
            if let Some(next_id) = scheduler.schedule() {
                let is_runnable = scheduler.get_thread(next_id)
                    .map_or(false, |t| t.is_runnable());
                
                if is_runnable {
                    scheduler.set_current_thread(Some(next_id));
                    let thread_context = scheduler.get_thread(next_id).unwrap();
                    let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
                    preemptive_threads::context::switch_context(
                        dummy_context.as_ptr() as *mut _,
                        &thread_context.context as *const _,
                    );
                } else {
                    active_threads -= 1;
                }
            } else {
                break;
            }
        }
    }

    print_str(b"\nAll threads completed successfully!\n");
}

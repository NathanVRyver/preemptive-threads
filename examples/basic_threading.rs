//! Basic threading example showing cooperative multitasking
//! 
//! This example demonstrates:
//! - Creating multiple threads with different priorities
//! - Cooperative yielding between threads
//! - Basic thread lifecycle management

extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext};

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
            0 => b"Worker 1: Starting work\n",
            1 => b"Worker 1: Processing data\n", 
            2 => b"Worker 1: Computing results\n",
            3 => b"Worker 1: Finalizing\n",
            _ => b"Worker 1: Work complete\n",
        };
        print_str(msg);
        yield_thread();
    }
}

fn worker_thread_2() {
    for i in 0..5 {
        let msg = match i {
            0 => b"Worker 2: Initializing\n",
            1 => b"Worker 2: Loading resources\n",
            2 => b"Worker 2: Executing task\n", 
            3 => b"Worker 2: Cleaning up\n",
            _ => b"Worker 2: Task finished\n",
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
        scheduler.spawn_thread(&mut STACK1, worker_thread_1, 1).unwrap();
        scheduler.spawn_thread(&mut STACK2, worker_thread_2, 1).unwrap();
        
        // Run the scheduler
        let mut active_threads = 2;
        while active_threads > 0 {
            if let Some(next_id) = scheduler.schedule() {
                if let Some(thread) = scheduler.get_thread(next_id) {
                    if thread.is_runnable() {
                        scheduler.set_current_thread(Some(next_id));
                        let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
                        preemptive_mlthreading_rust::context::switch_context(
                            dummy_context.as_ptr() as *mut _,
                            &thread.context as *const _
                        );
                    } else {
                        active_threads -= 1;
                    }
                }
            } else {
                break;
            }
        }
    }
    
    print_str(b"\nAll threads completed successfully!\n");
}
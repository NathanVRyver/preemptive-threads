extern crate preemptive_threads;

use preemptive_threads::{scheduler::SCHEDULER, sync::yield_thread};

static mut STACK1: [u8; 32 * 1024] = [0; 32 * 1024];
static mut STACK2: [u8; 32 * 1024] = [0; 32 * 1024];

fn worker_thread_1() {
    for i in 0..5 {
        println!("Thread 1: Iteration {}", i);
        yield_thread();
    }
    println!("Thread 1: Complete");
}

fn worker_thread_2() {
    for i in 0..5 {
        println!("Thread 2: Step {}", i);
        yield_thread();
    }
    println!("Thread 2: Done");
}

fn main() {
    unsafe {
        let scheduler = SCHEDULER.get();

        // Spawn two threads with equal priority
        scheduler
            .spawn_thread((&raw mut STACK1).as_mut().unwrap(), worker_thread_1, 1)
            .unwrap();
        scheduler
            .spawn_thread((&raw mut STACK2).as_mut().unwrap(), worker_thread_2, 1)
            .unwrap();

        // Start the first thread
        if let Some(first_thread) = scheduler.schedule() {
            scheduler.set_current_thread(Some(first_thread));
            let dummy_context = core::mem::MaybeUninit::<preemptive_threads::thread::ThreadContext>::uninit();
            let to_thread = scheduler.get_thread(first_thread).unwrap();
            preemptive_threads::context::switch_context(
                dummy_context.as_ptr() as *mut _,
                &to_thread.context as *const _,
            );
        }
    }

    println!("All threads completed!");
}

extern crate preemptive_threads;

use preemptive_threads::{
    preemption::Preemption, scheduler::SCHEDULER, thread::ThreadContext,
};

static mut STACK1: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK2: [u8; 64 * 1024] = [0; 64 * 1024];
static mut PREEMPTION: Preemption = Preemption::new();

fn infinite_loop_thread1() {
    let mut counter = 0u64;
    loop {
        counter += 1;
        if counter % 1000000 == 0 {
            print_str(b"Thread 1 still running\n");
        }
    }
}

fn infinite_loop_thread2() {
    let mut counter = 0u64;
    loop {
        counter += 1;
        if counter % 1000000 == 0 {
            print_str(b"Thread 2 still running\n");
        }
    }
}

fn print_str(msg: &[u8]) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    unsafe {
        write(1, msg.as_ptr(), msg.len());
    }
}

fn main() {
    print_str(b"Starting preemption test...\n");

    unsafe {
        let scheduler = SCHEDULER.get();

        scheduler
            .spawn_thread(&mut STACK1, infinite_loop_thread1, 1)
            .unwrap();
        scheduler
            .spawn_thread(&mut STACK2, infinite_loop_thread2, 1)
            .unwrap();

        PREEMPTION.enable(10000);

        if let Some(first_thread) = scheduler.schedule() {
            scheduler.set_current_thread(Some(first_thread));
            let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
            let to_thread = scheduler.get_thread(first_thread).unwrap();
            preemptive_threads::context::switch_context(
                dummy_context.as_ptr() as *mut _,
                &to_thread.context as *const _,
            );
        }
    }
}

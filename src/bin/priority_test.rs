extern crate preemptive_threads;

use preemptive_threads::{
    scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut STACK1: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK2: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK3: [u8; 64 * 1024] = [0; 64 * 1024];

fn high_priority_thread() {
    for i in 0..5 {
        print_with_priority(b"HIGH", i);
        yield_thread();
    }
}

fn medium_priority_thread() {
    for i in 0..5 {
        print_with_priority(b"MED ", i);
        yield_thread();
    }
}

fn low_priority_thread() {
    for i in 0..5 {
        print_with_priority(b"LOW ", i);
        yield_thread();
    }
}

fn print_with_priority(priority: &[u8], num: usize) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    let mut buffer = [0u8; 32];
    let mut pos = 0;

    for &byte in priority {
        buffer[pos] = byte;
        pos += 1;
    }

    buffer[pos] = b':';
    pos += 1;
    buffer[pos] = b' ';
    pos += 1;

    if num >= 10 {
        buffer[pos] = b'0' + (num / 10) as u8;
        pos += 1;
    }
    buffer[pos] = b'0' + (num % 10) as u8;
    pos += 1;
    buffer[pos] = b'\n';
    pos += 1;

    unsafe {
        write(1, buffer.as_ptr(), pos);
    }
}

fn main() {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    let msg = b"Starting priority test...\n";
    unsafe {
        write(1, msg.as_ptr(), msg.len());
    }

    unsafe {
        let scheduler = SCHEDULER.get();

        scheduler
            .spawn_thread(&mut STACK1, low_priority_thread, 1)
            .unwrap();
        scheduler
            .spawn_thread(&mut STACK2, medium_priority_thread, 5)
            .unwrap();
        scheduler
            .spawn_thread(&mut STACK3, high_priority_thread, 10)
            .unwrap();

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

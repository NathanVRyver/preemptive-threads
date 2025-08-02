extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{
    scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut STACK1: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK2: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK3: [u8; 64 * 1024] = [0; 64 * 1024];
static mut COUNTER: usize = 0;

fn thread1() {
    for i in 0..10 {
        unsafe {
            COUNTER += 1;
            print_number(1, i);
        }
        yield_thread();
    }
}

fn thread2() {
    for i in 0..10 {
        unsafe {
            COUNTER += 1;
            print_number(2, i);
        }
        yield_thread();
    }
}

fn thread3() {
    for i in 0..10 {
        unsafe {
            COUNTER += 1;
            print_number(3, i);
        }
        yield_thread();
    }
}

fn print_number(thread_id: usize, num: usize) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    let mut buffer = [0u8; 32];
    let msg = b"Thread ";
    let mut pos = 0;

    for &byte in msg {
        buffer[pos] = byte;
        pos += 1;
    }

    buffer[pos] = b'0' + thread_id as u8;
    pos += 1;
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
    unsafe {
        let scheduler = SCHEDULER.get();

        scheduler.spawn_thread(&mut STACK1, thread1, 1).unwrap();
        scheduler.spawn_thread(&mut STACK2, thread2, 1).unwrap();
        scheduler.spawn_thread(&mut STACK3, thread3, 1).unwrap();

        if let Some(first_thread) = scheduler.schedule() {
            scheduler.set_current_thread(Some(first_thread));
            let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
            let to_thread = scheduler.get_thread(first_thread).unwrap();
            preemptive_mlthreading_rust::context::switch_context(
                dummy_context.as_ptr() as *mut _,
                &to_thread.context as *const _,
            );
        }
    }
}

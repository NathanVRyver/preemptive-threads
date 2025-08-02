extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{
    scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

const NUM_THREADS: usize = 10;
static mut STACKS: [[u8; 16 * 1024]; NUM_THREADS] = [[0; 16 * 1024]; NUM_THREADS];
static mut THREAD_COUNTERS: [usize; NUM_THREADS] = [0; NUM_THREADS];

fn worker_thread(id: usize) {
    for _ in 0..100 {
        unsafe {
            THREAD_COUNTERS[id] += 1;
        }
        yield_thread();
    }

    print_thread_stats(id);
}

fn print_thread_stats(id: usize) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    let mut buffer = [0u8; 64];
    let msg = b"Thread ";
    let mut pos = 0;

    for &byte in msg {
        buffer[pos] = byte;
        pos += 1;
    }

    buffer[pos] = b'0' + id as u8;
    pos += 1;
    buffer[pos] = b' ';
    pos += 1;
    buffer[pos] = b'c';
    pos += 1;
    buffer[pos] = b'o';
    pos += 1;
    buffer[pos] = b'u';
    pos += 1;
    buffer[pos] = b'n';
    pos += 1;
    buffer[pos] = b't';
    pos += 1;
    buffer[pos] = b':';
    pos += 1;
    buffer[pos] = b' ';
    pos += 1;

    let count = unsafe { THREAD_COUNTERS[id] };
    if count >= 100 {
        buffer[pos] = b'0' + (count / 100) as u8;
        pos += 1;
    }
    if count >= 10 {
        buffer[pos] = b'0' + ((count / 10) % 10) as u8;
        pos += 1;
    }
    buffer[pos] = b'0' + (count % 10) as u8;
    pos += 1;
    buffer[pos] = b'\n';
    pos += 1;

    unsafe {
        write(1, buffer.as_ptr(), pos);
    }
}

fn thread0() {
    worker_thread(0);
}
fn thread1() {
    worker_thread(1);
}
fn thread2() {
    worker_thread(2);
}
fn thread3() {
    worker_thread(3);
}
fn thread4() {
    worker_thread(4);
}
fn thread5() {
    worker_thread(5);
}
fn thread6() {
    worker_thread(6);
}
fn thread7() {
    worker_thread(7);
}
fn thread8() {
    worker_thread(8);
}
fn thread9() {
    worker_thread(9);
}

fn main() {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    let msg = b"Starting stress test with 10 threads...\n";
    unsafe {
        write(1, msg.as_ptr(), msg.len());
    }

    unsafe {
        let scheduler = SCHEDULER.get();

        let thread_fns = [
            thread0, thread1, thread2, thread3, thread4, thread5, thread6, thread7, thread8,
            thread9,
        ];

        for i in 0..NUM_THREADS {
            scheduler
                .spawn_thread(&mut STACKS[i], thread_fns[i], 1)
                .unwrap();
        }

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

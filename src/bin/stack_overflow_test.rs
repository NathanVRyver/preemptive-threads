extern crate preemptive_threads;

use preemptive_threads::{
    scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut STACK1: [u8; 4 * 1024] = [0; 4 * 1024];

fn recursive_thread() {
    print_str(b"Starting recursive function...\n");
    deep_recursion(0);
}

fn deep_recursion(depth: u32) {
    let large_array = [0u8; 512];

    if depth % 10 == 0 {
        let mut buffer = [0u8; 32];
        let msg = b"Depth: ";
        let mut pos = 0;

        for &byte in msg {
            buffer[pos] = byte;
            pos += 1;
        }

        let depth_str = format_number(depth);
        for &byte in &depth_str {
            if byte != 0 {
                buffer[pos] = byte;
                pos += 1;
            }
        }

        buffer[pos] = b'\n';
        pos += 1;

        print_buffer(&buffer[..pos]);
        yield_thread();
    }

    if depth < 1000 {
        deep_recursion(depth + 1);
    }

    let _ = large_array[0];
}

fn format_number(mut num: u32) -> [u8; 16] {
    let mut buffer = [0u8; 16];
    let mut pos = 15;

    if num == 0 {
        buffer[pos] = b'0';
        return buffer;
    }

    while num > 0 {
        buffer[pos] = b'0' + (num % 10) as u8;
        num /= 10;
        if pos > 0 {
            pos -= 1;
        }
    }

    buffer
}

fn print_buffer(data: &[u8]) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    unsafe {
        write(1, data.as_ptr(), data.len());
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
    print_str(b"Starting stack overflow test (small 4KB stack)...\n");

    unsafe {
        let scheduler = SCHEDULER.get();

        scheduler
            .spawn_thread(&mut STACK1, recursive_thread, 1)
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

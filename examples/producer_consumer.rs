//! Producer-Consumer example using thread coordination
//!
//! This example demonstrates:
//! - Thread coordination patterns
//! - Shared data structures
//! - Work queue simulation

extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{
    scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut PRODUCER_STACK: [u8; 32 * 1024] = [0; 32 * 1024];
static mut CONSUMER_STACK: [u8; 32 * 1024] = [0; 32 * 1024];

static mut WORK_QUEUE: [u32; 16] = [0; 16];
static mut QUEUE_HEAD: usize = 0;
static mut QUEUE_TAIL: usize = 0;
static mut ITEMS_PRODUCED: u32 = 0;
static mut ITEMS_CONSUMED: u32 = 0;

fn print_str(msg: &[u8]) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    unsafe {
        write(1, msg.as_ptr(), msg.len());
    }
}

fn print_number(num: u32) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }

    let mut buffer = [0u8; 16];
    let mut pos = 0;

    if num == 0 {
        buffer[0] = b'0';
        pos = 1;
    } else {
        let mut temp = num;
        let mut digits = [0u8; 10];
        let mut digit_count = 0;

        while temp > 0 {
            digits[digit_count] = (temp % 10) as u8 + b'0';
            temp /= 10;
            digit_count += 1;
        }

        for i in 0..digit_count {
            buffer[pos] = digits[digit_count - 1 - i];
            pos += 1;
        }
    }

    unsafe {
        write(1, buffer.as_ptr(), pos);
    }
}

fn queue_put(item: u32) -> bool {
    unsafe {
        let next_tail = (QUEUE_TAIL + 1) % 16;
        if next_tail == QUEUE_HEAD {
            return false; // Queue full
        }

        WORK_QUEUE[QUEUE_TAIL] = item;
        QUEUE_TAIL = next_tail;
        true
    }
}

fn queue_get() -> Option<u32> {
    unsafe {
        if QUEUE_HEAD == QUEUE_TAIL {
            return None; // Queue empty
        }

        let item = WORK_QUEUE[QUEUE_HEAD];
        QUEUE_HEAD = (QUEUE_HEAD + 1) % 16;
        Some(item)
    }
}

fn producer_thread() {
    for i in 1..=20 {
        // Simulate work to produce item
        for _ in 0..1000 {
            // Busy work
        }

        // Try to put item in queue
        while !queue_put(i) {
            print_str(b"Producer: Queue full, waiting...\n");
            yield_thread();
        }

        unsafe {
            ITEMS_PRODUCED += 1;
        }

        print_str(b"Producer: Created item ");
        print_number(i);
        print_str(b"\n");

        yield_thread();
    }

    print_str(b"Producer: Finished producing all items\n");
}

fn consumer_thread() {
    loop {
        if let Some(item) = queue_get() {
            // Simulate processing the item
            for _ in 0..500 {
                // Busy work
            }

            unsafe {
                ITEMS_CONSUMED += 1;
            }

            print_str(b"Consumer: Processed item ");
            print_number(item);
            print_str(b"\n");

            // Check if we've consumed all items
            unsafe {
                if ITEMS_CONSUMED >= 20 {
                    break;
                }
            }
        } else {
            print_str(b"Consumer: Queue empty, waiting...\n");
        }

        yield_thread();
    }

    print_str(b"Consumer: Finished processing all items\n");
}

fn main() {
    print_str(b"=== Producer-Consumer Example ===\n");
    print_str(b"Producer will create 20 items, Consumer will process them\n\n");

    unsafe {
        let scheduler = SCHEDULER.get();

        // Spawn producer and consumer threads
        scheduler
            .spawn_thread(&mut PRODUCER_STACK, producer_thread, 1)
            .unwrap();
        scheduler
            .spawn_thread(&mut CONSUMER_STACK, consumer_thread, 1)
            .unwrap();

        // Run the scheduler
        let mut iterations = 0;
        loop {
            if let Some(next_id) = scheduler.schedule() {
                if let Some(thread) = scheduler.get_thread(next_id) {
                    if thread.is_runnable() {
                        scheduler.set_current_thread(Some(next_id));
                        let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
                        preemptive_mlthreading_rust::context::switch_context(
                            dummy_context.as_ptr() as *mut _,
                            &thread.context as *const _,
                        );
                    }
                }
            } else {
                break;
            }

            iterations += 1;
            if iterations > 10000 || (ITEMS_PRODUCED >= 20 && ITEMS_CONSUMED >= 20) {
                break;
            }
        }
    }

    print_str(b"\nFinal Statistics:\n");
    print_str(b"Items Produced: ");
    unsafe {
        print_number(ITEMS_PRODUCED);
    }
    print_str(b"\nItems Consumed: ");
    unsafe {
        print_number(ITEMS_CONSUMED);
    }
    print_str(b"\n\nExample completed!\n");
}

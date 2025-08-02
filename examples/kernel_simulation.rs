//! Kernel-style threading simulation
//!
//! This example demonstrates how the library could be used in an OS kernel:
//! - Multiple process simulation
//! - System call handling
//! - Interrupt-like preemption

extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{
    preemption::Preemption, scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut PROCESS_STACKS: [[u8; 16 * 1024]; 4] = [[0; 16 * 1024]; 4];
static mut PREEMPTION: Preemption = Preemption::new();

static mut SYSTEM_TICK: u32 = 0;
static mut PROCESS_STATS: [ProcessStats; 4] = [ProcessStats::new(); 4];

#[derive(Clone, Copy)]
struct ProcessStats {
    cpu_time: u32,
    syscalls: u32,
    yields: u32,
}

impl ProcessStats {
    const fn new() -> Self {
        ProcessStats {
            cpu_time: 0,
            syscalls: 0,
            yields: 0,
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

    buffer[pos] = b'\n';
    pos += 1;

    unsafe {
        write(1, buffer.as_ptr(), pos);
    }
}

fn syscall_print(process_id: u8, msg: &[u8]) {
    unsafe {
        PROCESS_STATS[process_id as usize].syscalls += 1;
    }

    print_str(b"[PID ");
    print_str(&[b'0' + process_id]);
    print_str(b"] ");
    print_str(msg);
}

fn syscall_yield(process_id: u8) {
    unsafe {
        PROCESS_STATS[process_id as usize].yields += 1;
    }
    yield_thread();
}

fn process_0() {
    for i in 0..10 {
        syscall_print(0, b"System process running\n");

        // Simulate system work
        for _ in 0..5000 {
            unsafe {
                PROCESS_STATS[0].cpu_time += 1;
            }
        }

        if i % 3 == 0 {
            syscall_yield(0);
        }
    }

    syscall_print(0, b"System process terminated\n");
}

fn process_1() {
    for i in 0..8 {
        syscall_print(1, b"User application A executing\n");

        // Simulate user work
        for _ in 0..3000 {
            unsafe {
                PROCESS_STATS[1].cpu_time += 1;
            }
        }

        if i % 2 == 0 {
            syscall_yield(1);
        }
    }

    syscall_print(1, b"User application A finished\n");
}

fn process_2() {
    for i in 0..6 {
        syscall_print(2, b"Background service running\n");

        // Simulate background work
        for _ in 0..2000 {
            unsafe {
                PROCESS_STATS[2].cpu_time += 1;
            }
        }

        syscall_yield(2);
    }

    syscall_print(2, b"Background service stopped\n");
}

fn process_3() {
    for i in 0..12 {
        syscall_print(3, b"Batch job processing\n");

        // Simulate batch processing
        for _ in 0..1000 {
            unsafe {
                PROCESS_STATS[3].cpu_time += 1;
            }
        }

        if i % 4 == 0 {
            syscall_yield(3);
        }
    }

    syscall_print(3, b"Batch job completed\n");
}

fn main() {
    print_str(b"=== Kernel Threading Simulation ===\n");
    print_str(b"Simulating OS kernel with multiple processes\n");
    print_str(b"Enabling preemptive scheduling...\n\n");

    unsafe {
        let scheduler = SCHEDULER.get();

        // Create processes with different priorities (like a real kernel)
        scheduler
            .spawn_thread(&mut PROCESS_STACKS[0], process_0, 10)
            .unwrap(); // System (highest)
        scheduler
            .spawn_thread(&mut PROCESS_STACKS[1], process_1, 5)
            .unwrap(); // User app
        scheduler
            .spawn_thread(&mut PROCESS_STACKS[2], process_2, 3)
            .unwrap(); // Background
        scheduler
            .spawn_thread(&mut PROCESS_STACKS[3], process_3, 1)
            .unwrap(); // Batch (lowest)

        // Enable preemption (simulate timer interrupts)
        PREEMPTION.enable(5000); // 5ms time slices

        // Kernel scheduler loop
        let mut scheduler_iterations = 0;
        loop {
            unsafe {
                SYSTEM_TICK += 1;
            }

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
                break; // No more runnable processes
            }

            scheduler_iterations += 1;
            if scheduler_iterations > 50000 {
                break; // Safety limit
            }
        }

        PREEMPTION.disable();
    }

    print_str(b"\n=== System Statistics ===\n");
    unsafe {
        for i in 0..4 {
            print_str(b"Process ");
            print_str(&[b'0' + i as u8]);
            print_str(b": CPU=");
            print_number(PROCESS_STATS[i].cpu_time);
            print_str(b"  Syscalls=");
            print_number(PROCESS_STATS[i].syscalls);
            print_str(b"  Yields=");
            print_number(PROCESS_STATS[i].yields);
        }

        print_str(b"System Ticks: ");
        print_number(SYSTEM_TICK);
    }

    print_str(b"\nKernel simulation completed!\n");
}

extern crate preemptive_threads;

use preemptive_threads::{
    preemption::Preemption, scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext,
};

static mut STACKS: [[u8; 32 * 1024]; 8] = [[0; 32 * 1024]; 8];
static mut PREEMPTION: Preemption = Preemption::new();
static mut THREAD_COUNTERS: [u32; 8] = [0; 8];

fn print_str(msg: &[u8]) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    unsafe {
        write(1, msg.as_ptr(), msg.len());
    }
}

fn print_colored(msg: &[u8], color: u8) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    let mut buffer = [0u8; 256];
    let mut pos = 0;

    buffer[pos] = 0x1b;
    pos += 1;
    buffer[pos] = b'[';
    pos += 1;
    buffer[pos] = b'3';
    pos += 1;
    buffer[pos] = color;
    pos += 1;
    buffer[pos] = b'm';
    pos += 1;

    for &byte in msg {
        if pos < 250 {
            buffer[pos] = byte;
            pos += 1;
        }
    }

    buffer[pos] = 0x1b;
    pos += 1;
    buffer[pos] = b'[';
    pos += 1;
    buffer[pos] = b'0';
    pos += 1;
    buffer[pos] = b'm';
    pos += 1;

    unsafe {
        write(1, buffer.as_ptr(), pos);
    }
}

fn worker_thread(id: u8, work_type: u8) {
    match work_type {
        0 => cpu_intensive_worker(id),
        1 => cooperative_worker(id),
        2 => priority_worker(id),
        _ => memory_worker(id),
    }
}

fn cpu_intensive_worker(id: u8) {
    let colors = [b'1', b'2', b'3', b'4', b'5', b'6'];
    let color = colors[id as usize % colors.len()];

    for round in 0..20 {
        let mut sum = 0u64;
        for i in 0..50000 {
            sum = sum.wrapping_add(i);
        }

        unsafe {
            THREAD_COUNTERS[id as usize] += 1;
        }

        let mut msg = [0u8; 64];
        let msg_str = b"CPU Thread ";
        let mut pos = 0;
        for &b in msg_str {
            msg[pos] = b;
            pos += 1;
        }
        msg[pos] = b'0' + id;
        pos += 1;
        let progress_str = b" completed round ";
        for &b in progress_str {
            msg[pos] = b;
            pos += 1;
        }
        if round >= 10 {
            msg[pos] = b'0' + (round / 10) as u8;
            pos += 1;
        }
        msg[pos] = b'0' + (round % 10) as u8;
        pos += 1;
        msg[pos] = b'\n';
        pos += 1;

        print_colored(&msg[..pos], color);

        let _ = sum;
        yield_thread();
    }
}

fn cooperative_worker(id: u8) {
    let colors = [b'1', b'2', b'3', b'4'];
    let color = colors[id as usize % colors.len()];

    for i in 0..15 {
        unsafe {
            THREAD_COUNTERS[id as usize] += 1;
        }

        let mut msg = [0u8; 64];
        let msg_str = b"Cooperative ";
        let mut pos = 0;
        for &b in msg_str {
            msg[pos] = b;
            pos += 1;
        }
        msg[pos] = b'0' + id;
        pos += 1;
        let iter_str = b" iteration ";
        for &b in iter_str {
            msg[pos] = b;
            pos += 1;
        }
        if i >= 10 {
            msg[pos] = b'0' + (i / 10) as u8;
            pos += 1;
        }
        msg[pos] = b'0' + (i % 10) as u8;
        pos += 1;
        let done_str = b" (yielding)\n";
        for &b in done_str {
            msg[pos] = b;
            pos += 1;
        }

        print_colored(&msg[..pos], color);
        yield_thread();
    }
}

fn priority_worker(id: u8) {
    let priority = match id {
        0 => (b"HIGH", b'1'),
        1 => (b"MED ", b'3'),
        _ => (b"LOW ", b'6'),
    };

    for i in 0..10 {
        unsafe {
            THREAD_COUNTERS[id as usize] += 1;
        }

        let mut msg = [0u8; 64];
        let mut pos = 0;
        for &b in priority.0 {
            msg[pos] = b;
            pos += 1;
        }
        let priority_str = b" priority thread ";
        for &b in priority_str {
            msg[pos] = b;
            pos += 1;
        }
        msg[pos] = b'0' + id;
        pos += 1;
        let work_str = b" doing work ";
        for &b in work_str {
            msg[pos] = b;
            pos += 1;
        }
        msg[pos] = b'0' + (i % 10) as u8;
        pos += 1;
        msg[pos] = b'\n';
        pos += 1;

        print_colored(&msg[..pos], priority.1);
        yield_thread();
    }
}

fn memory_worker(id: u8) {
    let color = b'5';
    let mut data = [0u8; 1024];

    for round in 0..10 {
        for i in 0..1024 {
            data[i] = ((i + round * 13) % 256) as u8;
        }

        let mut checksum = 0u32;
        for &byte in &data {
            checksum = checksum.wrapping_add(byte as u32);
        }

        unsafe {
            THREAD_COUNTERS[id as usize] += 1;
        }

        let mut msg = [0u8; 64];
        let msg_str = b"Memory worker ";
        let mut pos = 0;
        for &b in msg_str {
            msg[pos] = b;
            pos += 1;
        }
        msg[pos] = b'0' + id;
        pos += 1;
        let checksum_str = b" checksum: ";
        for &b in checksum_str {
            msg[pos] = b;
            pos += 1;
        }

        let mut temp = checksum;
        let mut digits = [0u8; 10];
        let mut digit_count = 0;
        while temp > 0 {
            digits[digit_count] = (temp % 10) as u8 + b'0';
            temp /= 10;
            digit_count += 1;
        }
        for i in 0..digit_count {
            msg[pos] = digits[digit_count - 1 - i];
            pos += 1;
        }
        msg[pos] = b'\n';
        pos += 1;

        print_colored(&msg[..pos], color);
        yield_thread();
    }
}

fn worker0() {
    worker_thread(0, 0);
}
fn worker1() {
    worker_thread(1, 0);
}
fn worker2() {
    worker_thread(2, 0);
}
fn coop0() {
    cooperative_worker(0);
}
fn coop1() {
    cooperative_worker(1);
}
fn prio0() {
    priority_worker(0);
}
fn prio1() {
    priority_worker(1);
}
fn prio2() {
    priority_worker(2);
}

fn run_demo_phase(phase: u32) {
    match phase {
        0 => {
            print_str(b"\n");
            print_colored(b"=== PHASE 1: CPU-Intensive Multithreading ===\n", b'7');
            print_str(b"Spawning 3 CPU-intensive threads...\n\n");

            unsafe {
                let scheduler = SCHEDULER.get();
                scheduler.spawn_thread(&mut STACKS[0], worker0, 1).unwrap();
                scheduler.spawn_thread(&mut STACKS[1], worker1, 1).unwrap();
                scheduler.spawn_thread(&mut STACKS[2], worker2, 1).unwrap();
            }
        }
        1 => {
            print_str(b"\n");
            print_colored(b"=== PHASE 2: Cooperative Threading ===\n", b'7');
            print_str(b"Spawning cooperative threads that yield voluntarily...\n\n");

            unsafe {
                let scheduler = SCHEDULER.get();
                scheduler.spawn_thread(&mut STACKS[3], coop0, 1).unwrap();
                scheduler.spawn_thread(&mut STACKS[4], coop1, 1).unwrap();
            }
        }
        2 => {
            print_str(b"\n");
            print_colored(b"=== PHASE 3: Priority-Based Scheduling ===\n", b'7');
            print_str(b"Testing priority scheduling (HIGH=10, MED=5, LOW=1)...\n\n");

            unsafe {
                let scheduler = SCHEDULER.get();
                scheduler.spawn_thread(&mut STACKS[5], prio2, 1).unwrap();
                scheduler.spawn_thread(&mut STACKS[6], prio1, 5).unwrap();
                scheduler.spawn_thread(&mut STACKS[7], prio0, 10).unwrap();
            }
        }
        3 => {
            print_str(b"\n");
            print_colored(b"=== PHASE 4: Preemptive Scheduling ===\n", b'7');
            print_str(b"Enabling 10ms time slices for preemptive multitasking...\n\n");

            unsafe {
                PREEMPTION.enable(10000);
            }
        }
        _ => {}
    }
}

fn main() {
    print_colored(b"PREEMPTIVE MULTITHREADING LIBRARY DEMO\n", b'6');
    print_str(b"=========================================\n");
    print_str(b"Watch multiple threads execute concurrently!\n");
    print_str(b"Colors show different threads working simultaneously.\n\n");

    unsafe {
        let scheduler = SCHEDULER.get();

        for phase in 0..4 {
            run_demo_phase(phase);

            let mut active_threads;
            loop {
                if let Some(next_id) = scheduler.schedule() {
                    let is_runnable = scheduler
                        .get_thread(next_id)
                        .map_or(false, |t| t.is_runnable());

                    if is_runnable {
                        scheduler.set_current_thread(Some(next_id));
                        let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
                        let thread = scheduler.get_thread(next_id).unwrap();
                        preemptive_threads::context::switch_context(
                            dummy_context.as_ptr() as *mut _,
                            &thread.context as *const _,
                        );
                    }
                } else {
                    break;
                }

                active_threads = 0;
                for i in 0..8 {
                    if let Some(thread) = scheduler.get_thread(i) {
                        if thread.is_runnable() {
                            active_threads += 1;
                        }
                    }
                }

                if active_threads == 0 {
                    break;
                }
            }

            let mut msg = [0u8; 64];
            let phase_str = b"Phase ";
            let mut pos = 0;
            for &b in phase_str {
                msg[pos] = b;
                pos += 1;
            }
            msg[pos] = b'0' + (phase + 1) as u8;
            pos += 1;
            let complete_str = b" completed!\n";
            for &b in complete_str {
                msg[pos] = b;
                pos += 1;
            }

            print_colored(&msg[..pos], b'2');
        }

        print_str(b"\n");
        print_colored(b"=== FINAL STATISTICS ===\n", b'7');
        for i in 0..8 {
            if THREAD_COUNTERS[i] > 0 {
                let mut msg = [0u8; 64];
                let thread_str = b"Thread ";
                let mut pos = 0;
                for &b in thread_str {
                    msg[pos] = b;
                    pos += 1;
                }
                msg[pos] = b'0' + i as u8;
                pos += 1;
                let completed_str = b" completed ";
                for &b in completed_str {
                    msg[pos] = b;
                    pos += 1;
                }

                let count = THREAD_COUNTERS[i];
                if count >= 10 {
                    msg[pos] = b'0' + (count / 10) as u8;
                    pos += 1;
                }
                msg[pos] = b'0' + (count % 10) as u8;
                pos += 1;
                let tasks_str = b" tasks\n";
                for &b in tasks_str {
                    msg[pos] = b;
                    pos += 1;
                }

                print_str(&msg[..pos]);
            }
        }

        print_str(b"\n");
        print_colored(b"Demo complete! All threads executed successfully.\n", b'2');
        print_str(b"This demonstrates cooperative, preemptive, and priority scheduling.\n");
    }
}

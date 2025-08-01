extern crate preemptive_mlthreading_rust;

use preemptive_mlthreading_rust::{scheduler::SCHEDULER, sync::yield_thread, thread::ThreadContext};

static mut STACKS: [[u8; 32 * 1024]; 16] = [[0; 32 * 1024]; 16];
static mut BENCHMARK_RESULTS: [u64; 16] = [0; 16];

fn get_timestamp() -> u64 {
    unsafe {
        let mut timestamp: u64;
        core::arch::asm!(
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") timestamp,
            out("rdx") _,
        );
        timestamp
    }
}

fn context_switch_benchmark() {
    let start = get_timestamp();
    
    for _ in 0..1000 {
        yield_thread();
    }
    
    let end = get_timestamp();
    let thread_id = unsafe { SCHEDULER.get().get_current_thread().unwrap_or(0) };
    unsafe {
        BENCHMARK_RESULTS[thread_id] = end - start;
    }
}

fn cpu_intensive_benchmark() {
    let start = get_timestamp();
    
    let mut sum = 0u64;
    for i in 0..100000 {
        sum = sum.wrapping_add(i);
        if i % 10000 == 0 {
            yield_thread();
        }
    }
    
    let end = get_timestamp();
    let thread_id = unsafe { SCHEDULER.get().get_current_thread().unwrap_or(0) };
    unsafe {
        BENCHMARK_RESULTS[thread_id] = end - start;
    }
}

fn memory_benchmark() {
    let start = get_timestamp();
    
    let mut data = [0u8; 4096];
    for i in 0..1000 {
        data[i % 4096] = (i % 256) as u8;
        if i % 100 == 0 {
            yield_thread();
        }
    }
    
    let end = get_timestamp();
    let thread_id = unsafe { SCHEDULER.get().get_current_thread().unwrap_or(0) };
    unsafe {
        BENCHMARK_RESULTS[thread_id] = end - start;
    }
    
    let _ = data[0];
}

fn print_number(num: u64) {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    
    let mut buffer = [0u8; 32];
    let mut pos = 0;
    
    if num == 0 {
        buffer[0] = b'0';
        pos = 1;
    } else {
        let mut temp = num;
        let mut digits = [0u8; 20];
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

fn print_benchmark_results() {
    extern "C" {
        fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    }
    
    let header = b"\n=== BENCHMARK RESULTS ===\n";
    unsafe { write(1, header.as_ptr(), header.len()); }
    
    let context_msg = b"Context Switch (1000 switches): ";
    unsafe { write(1, context_msg.as_ptr(), context_msg.len()); }
    print_number(unsafe { BENCHMARK_RESULTS[0] });
    let cycles_msg = b" cycles\n";
    unsafe { write(1, cycles_msg.as_ptr(), cycles_msg.len()); }
    
    let cpu_msg = b"CPU Intensive (100k ops): ";
    unsafe { write(1, cpu_msg.as_ptr(), cpu_msg.len()); }
    print_number(unsafe { BENCHMARK_RESULTS[1] });
    unsafe { write(1, cycles_msg.as_ptr(), cycles_msg.len()); }
    
    let mem_msg = b"Memory Access (1k writes): ";
    unsafe { write(1, mem_msg.as_ptr(), mem_msg.len()); }
    print_number(unsafe { BENCHMARK_RESULTS[2] });
    unsafe { write(1, cycles_msg.as_ptr(), cycles_msg.len()); }
    
    let per_switch = unsafe { BENCHMARK_RESULTS[0] } / 1000;
    let switch_msg = b"Average per context switch: ";
    unsafe { write(1, switch_msg.as_ptr(), switch_msg.len()); }
    print_number(per_switch);
    unsafe { write(1, cycles_msg.as_ptr(), cycles_msg.len()); }
    
    let end_msg = b"========================\n";
    unsafe { write(1, end_msg.as_ptr(), end_msg.len()); }
}

fn benchmark_runner() {
    let msg = b"Starting benchmarks...\n";
    extern "C" { fn write(fd: i32, buf: *const u8, count: usize) -> isize; }
    unsafe { write(1, msg.as_ptr(), msg.len()); }
    
    unsafe {
        let scheduler = SCHEDULER.get();
        
        scheduler.spawn_thread(&mut STACKS[0], context_switch_benchmark, 1).unwrap();
        scheduler.spawn_thread(&mut STACKS[1], cpu_intensive_benchmark, 1).unwrap();
        scheduler.spawn_thread(&mut STACKS[2], memory_benchmark, 1).unwrap();
        
        let mut completed = 0;
        while completed < 3 {
            if let Some(next_id) = scheduler.schedule() {
                if scheduler.get_thread(next_id).map_or(false, |t| t.is_runnable()) {
                    scheduler.set_current_thread(Some(next_id));
                    let dummy_context = core::mem::MaybeUninit::<ThreadContext>::uninit();
                    let to_thread = scheduler.get_thread(next_id).unwrap();
                    preemptive_mlthreading_rust::context::switch_context(
                        dummy_context.as_ptr() as *mut _,
                        &to_thread.context as *const _
                    );
                } else {
                    completed += 1;
                }
            } else {
                break;
            }
        }
    }
    
    print_benchmark_results();
}

fn main() {
    benchmark_runner();
}
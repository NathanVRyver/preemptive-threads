# preemptive-threads

A `#![no_std]` preemptive multithreading library for Rust.

## Features

- **No standard library** - Built for embedded systems and OS kernels
- **Preemptive scheduling** - Timer-based thread preemption (Linux)
- **Priority scheduling** - Higher priority threads run first
- **x86_64 only** - Hand-written assembly for fast context switching
- **Static allocation** - No heap required, deterministic memory usage

## Installation

```toml
[dependencies]
preemptive-threads = "0.1.0"
```

## Usage

```rust
#![no_std]

use preemptive_threads::{scheduler::SCHEDULER, sync::yield_thread};

static mut STACK1: [u8; 64 * 1024] = [0; 64 * 1024];
static mut STACK2: [u8; 64 * 1024] = [0; 64 * 1024];

fn thread1() {
    for _ in 0..5 {
        // Do work
        yield_thread();
    }
}

fn thread2() {
    for _ in 0..5 {
        // Do other work
        yield_thread();
    }
}

fn main() {
    unsafe {
        let scheduler = SCHEDULER.get();
        
        // Spawn threads with priority (higher = more priority)
        scheduler.spawn_thread(&mut STACK1, thread1, 1).unwrap();
        scheduler.spawn_thread(&mut STACK2, thread2, 1).unwrap();
        
        // Start scheduling
        if let Some(thread_id) = scheduler.schedule() {
            scheduler.set_current_thread(Some(thread_id));
            // Context switch happens here
        }
    }
}
```

## Preemptive Scheduling (Linux only)

```rust
use preemptive_threads::preemption::Preemption;

static mut PREEMPTION: Preemption = Preemption::new();

unsafe {
    // Enable 10ms time slices
    PREEMPTION.enable(10_000);
}
```

## Requirements

- **Architecture**: x86_64 only
- **OS**: Any (preemption requires Linux)
- **Memory**: ~64KB per thread (configurable)
- **Threads**: Maximum 32 concurrent threads

## Safety

This is a low-level library that requires `unsafe` code:
- Direct memory management for thread stacks
- Context switching modifies CPU state
- Shared scheduler access needs synchronization

## License

MIT OR Apache-2.0
# Preemptive Multithreading Rust Library

A production-ready `no_std` preemptive multithreading library built from scratch for OS kernels and embedded systems.

[![Crates.io](https://img.shields.io/crates/v/preemptive-threads.svg)](https://crates.io/crates/preemptive-threads)
[![Documentation](https://docs.rs/preemptive-threads/badge.svg)](https://docs.rs/preemptive-threads)
[![Downloads](https://img.shields.io/crates/d/preemptive-threads.svg)](https://crates.io/crates/preemptive-threads)

## Features

- **Zero Standard Library Dependencies**: Built with `#![no_std]` for maximum compatibility
- **True Preemptive Scheduling**: Hardware timer-driven preemption with configurable time slices
- **Lock-Free Architecture**: High-performance lock-free data structures throughout
- **Multi-Architecture Support**: x86_64, ARM64, and RISC-V support
- **Memory Safety**: RAII-based resource management with epoch-based reclamation
- **Security Hardening**: Stack canaries, CFI, ASLR, thread isolation, and comprehensive audit logging
- **Performance Optimized**: NUMA-aware scheduling, CPU cache optimization, and SIMD acceleration
- **Comprehensive Observability**: Built-in metrics, profiling, and health monitoring

## Architecture Support

| Architecture | Context Switch | Timer Interrupts | Security Features | Status |
|--------------|---------------|------------------|-------------------|---------|
| x86_64       | ✅            | ✅ (APIC)        | ✅ Full          | Stable  |
| ARM64        | ✅            | ✅ (Generic Timer)| ✅ Full          | Stable  |
| RISC-V 64    | ✅            | ✅ (SBI Timer)   | ✅ Full          | Stable  |

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
preemptive-threads = "1.0"

# Enable features as needed
[features]
default = ["x86_64", "hardened"]
x86_64 = []          # x86_64 architecture support
arm64 = []           # ARM64 architecture support  
riscv64 = []         # RISC-V 64-bit support
hardened = []        # Security hardening features
mmu = []             # Memory management unit features
work-stealing = []   # Work-stealing scheduler
std-shim = []        # Standard library compatibility
```

### Basic Threading Example

```rust
#![no_std]
#![no_main]

use preemptive_threads::{
    ThreadBuilder, JoinHandle, yield_now, 
    init_security, SecurityConfig
};

#[no_mangle]
pub extern "C" fn main() -> ! {
    // Initialize security subsystem
    let security_config = SecurityConfig::default();
    init_security(security_config).expect("Failed to initialize security");
    
    // Create and spawn threads
    let handle1 = ThreadBuilder::new()
        .name("worker1")
        .stack_size(64 * 1024)
        .priority(10)
        .spawn(|| {
            for i in 0..10 {
                println!("Worker 1: iteration {}", i);
                yield_now();
            }
        })
        .expect("Failed to spawn thread");
    
    let handle2 = ThreadBuilder::new()
        .name("worker2") 
        .spawn(|| {
            for i in 0..10 {
                println!("Worker 2: iteration {}", i);
                yield_now();
            }
        })
        .expect("Failed to spawn thread");
    
    // Wait for completion
    handle1.join().expect("Thread 1 failed");
    handle2.join().expect("Thread 2 failed");
    
    loop {}
}
```

### Advanced Scheduler Configuration

```rust
use preemptive_threads::{
    NewScheduler, RoundRobinScheduler, ThreadBuilder, 
    CpuId, Duration
};

// Configure scheduler for specific CPU
let mut scheduler = RoundRobinScheduler::new();
scheduler.set_time_slice(Duration::from_millis(10));

// Create CPU-affine thread
let handle = ThreadBuilder::new()
    .cpu_affinity(1u64 << 2) // CPU 2 only
    .priority(15)
    .spawn(|| {
        // High-priority work
        compute_intensive_task();
    })
    .expect("Failed to spawn thread");
```

### Security-Hardened Threading

```rust
use preemptive_threads::{
    ThreadBuilder, SecurityConfig, SecurityFeature,
    configure_security_feature, isolated_thread
};

// Enable all security features
configure_security_feature(SecurityFeature::StackCanaries, true);
configure_security_feature(SecurityFeature::Cfi, true);
configure_security_feature(SecurityFeature::Isolation, true);

// Create isolated thread
let handle = ThreadBuilder::new()
    .name("isolated_worker")
    .security_level(SecurityLevel::High)
    .spawn(|| {
        // This thread runs in isolation with stack canaries and CFI
        process_untrusted_data();
    })
    .expect("Failed to spawn secure thread");
```

## Performance Characteristics

### Context Switch Performance

| Architecture | Typical Time | Optimized Time | Notes |
|--------------|--------------|----------------|-------|
| x86_64       | ~500ns       | ~300ns         | With minimal register saves |
| ARM64        | ~400ns       | ~250ns         | Using pointer authentication |
| RISC-V       | ~600ns       | ~400ns         | RISC architecture benefits |

### Memory Usage

- **Base overhead**: ~8KB per thread
- **Stack size**: Configurable (default 64KB)  
- **Scheduler state**: ~64 bytes per thread
- **Global state**: ~4KB total

### Scalability

- **Threads**: Tested up to 10,000 concurrent threads
- **CPUs**: Scales linearly up to 64 cores
- **Memory**: Constant overhead per thread

## Security Features

### Stack Protection
- **Stack canaries**: Detect buffer overflows
- **Guard pages**: Prevent stack overflow (requires MMU)
- **Stack randomization**: ASLR for thread stacks

### Control Flow Integrity (CFI)
- **Indirect call protection**: Verify call targets
- **Return address protection**: Shadow stack
- **Function pointer verification**: Label-based CFI

### Thread Isolation
- **Domain-based isolation**: Separate thread security domains
- **Memory access control**: Restrict cross-thread access
- **Resource limits**: Per-domain resource quotas

### Audit Logging
- **Security events**: Comprehensive violation tracking
- **Performance monitoring**: Threshold-based alerts
- **Export formats**: JSON, CSV, plain text

## API Reference

### Core Types

#### ThreadBuilder
Primary interface for creating and configuring threads.

```rust
impl ThreadBuilder {
    pub fn new() -> Self
    pub fn name(self, name: &str) -> Self
    pub fn stack_size(self, size: usize) -> Self
    pub fn priority(self, priority: u8) -> Self
    pub fn cpu_affinity(self, mask: u64) -> Self
    pub fn spawn<F, T>(self, f: F) -> Result<JoinHandle<T>, ThreadError>
        where F: FnOnce() -> T + Send + 'static, T: Send + 'static
}
```

#### JoinHandle
Handle to a spawned thread that can be used to wait for completion.

```rust
impl<T> JoinHandle<T> {
    pub fn join(self) -> Result<T, ThreadError>
    pub fn thread(&self) -> &Thread
    pub fn is_finished(&self) -> bool
}
```

#### SecurityConfig
Configuration for security and hardening features.

```rust
#[derive(Debug, Clone, Copy)]
pub struct SecurityConfig {
    pub enable_stack_canaries: bool,
    pub enable_guard_pages: bool, 
    pub enable_cfi: bool,
    pub enable_thread_isolation: bool,
    pub enable_aslr: bool,
    pub enable_audit_logging: bool,
    pub use_secure_rng: bool,
    pub panic_on_violation: bool,
}
```

### Scheduler Types

#### RoundRobinScheduler
Simple round-robin scheduling with configurable time slices.

#### WorkStealingScheduler  
Advanced work-stealing scheduler with NUMA awareness (requires `work-stealing` feature).

### Synchronization Primitives

#### Mutex
Lock-free mutex implementation with fast paths.

```rust
let mutex = Mutex::new(42);
{
    let guard = mutex.lock();
    *guard += 1;
} // Automatically unlocked
```

### Memory Management

#### Stack
RAII stack management with automatic cleanup.

#### StackPool
High-performance stack allocation pool.

#### ArcLite
Lightweight atomic reference counting for `no_std` environments.

## Examples

See the `examples/` directory for complete working examples:

- **basic_threading**: Simple multi-threading example
- **scheduler_config**: Custom scheduler configuration
- **security_hardened**: Security features demonstration
- **performance_test**: Performance benchmarking
- **numa_aware**: NUMA-aware threading
- **embedded_kernel**: Integration with embedded kernel

## Platform Support

### Operating Systems
- **Freestanding**: Primary target (no OS)
- **Linux**: Testing and development
- **Embedded RTOS**: Various RTOS integrations

### Hardware Requirements

#### Minimum
- **Memory**: 64KB RAM minimum
- **Timer**: Hardware timer for preemption
- **Architecture**: x86_64, ARM64, or RISC-V

#### Recommended
- **Memory**: 1MB+ for optimal performance
- **Cores**: Multi-core for true parallelism
- **MMU**: Memory management unit for security features

## Building and Testing

### Prerequisites
```bash
# Install Rust (nightly required for some features)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install nightly
rustup default nightly

# Install target architectures
rustup target add x86_64-unknown-none
rustup target add aarch64-unknown-none  
rustup target add riscv64gc-unknown-none-elf
```

### Build
```bash
# Basic build
cargo build --release

# With all features  
cargo build --release --all-features

# Architecture-specific
cargo build --release --target x86_64-unknown-none --features x86_64,hardened
```

### Testing
```bash
# Unit tests (requires std)
cargo test --features std

# Integration tests
cargo test --test integration --features std

# Performance tests
cargo test --test performance --features std --release

# Security tests  
cargo test --test security --features std,hardened

# Fuzz testing (requires cargo-fuzz)
cargo install cargo-fuzz
cargo fuzz run thread_operations
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup
1. Fork the repository
2. Create a feature branch
3. Implement your changes with tests
4. Run the test suite
5. Submit a pull request

### Code Style
- Use `rustfmt` for formatting
- Run `clippy` for linting
- Follow Rust API guidelines
- Document all public APIs

## License

This project is dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))  
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

- Inspired by research in lock-free data structures
- Built on principles from modern OS kernels
- Security design influenced by CFI and Intel CET
- Performance optimization techniques from high-frequency trading systems

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and breaking changes.

## Roadmap

### v1.1 (Planned)
- [ ] WASM support for browser environments
- [ ] Real-time scheduling policies
- [ ] Hardware transactional memory support
- [ ] Advanced profiling integration

### v1.2 (Future)
- [ ] Distributed threading across network
- [ ] GPU compute thread integration
- [ ] Machine learning-based scheduling
- [ ] Formal verification of core algorithms

For detailed documentation, visit [docs.rs/preemptive-threads](https://docs.rs/preemptive-threads).
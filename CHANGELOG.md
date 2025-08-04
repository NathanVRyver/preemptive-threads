# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2025-01-04

### Added
- Lock-free atomic scheduler with per-priority circular buffers
- Full CPU context saving including FPU/SSE state via FXSAVE/FXRSTOR
- Enhanced stack protection with guard pages and canary values
- Signal-safe preemption handler that defers scheduling outside signal context
- Safe API abstractions including ThreadBuilder, Mutex, and ThreadPool
- Comprehensive error types for better error handling
- Stack watermark tracking for high water mark analysis

### Changed
- Replaced global mutable singleton with thread-safe atomic operations
- Replaced O(n²) scheduling algorithm with O(1) priority queue
- Improved context switching to save/restore RFLAGS correctly
- Tests no longer use unsafe transmute

### Fixed
- Race conditions in global scheduler access
- Memory safety issues with unsynchronized access
- Signal handler now only uses async-signal-safe operations
- Context switching now preserves full CPU state
- Stack overflow detection now checks entire guard region

## [0.1.1] - 2025-01-04

### Fixed
- Added bounds checking to prevent out-of-bounds access
- Added check for current thread existence before attempting exit
- Added type annotation for MaybeUninit for better type safety
- Added error handling and reentrancy protection to Linux preemption


## [0.1.0] - 2025-01-02

### Added
- Initial release of preemptive multithreading library
- Core threading primitives with `#![no_std]` support
- x86_64 assembly-based context switching (50-100 CPU cycles)
- Priority-based round-robin scheduler
- Stack overflow detection with guard values
- Cooperative yielding and thread lifecycle management
- Preemptive scheduling support (Linux only via SIGALRM)
- Comprehensive error handling without panics
- Thread join functionality
- Performance benchmarks and production readiness assessment

### Features
- Zero heap allocation
- Static memory allocation (up to 32 threads)
- Configurable stack sizes (default 64KB)
- Minimal dependencies (no_std compatible)
- Thread priorities (0-255)

### Known Limitations
- x86_64 platform only
- Single-core operation only
- Maximum 32 concurrent threads
- No thread-local storage
- Preemption not available on macOS

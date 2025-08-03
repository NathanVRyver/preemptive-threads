# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
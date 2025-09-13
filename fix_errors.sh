#!/bin/bash

# Fix critical compilation errors for v0.5.0

echo "Fixing compilation errors..."

# Fix unsafe function calls by wrapping in unsafe blocks
sed -i.bak 's/    generic_memcpy(dst, src, len);/    unsafe { generic_memcpy(dst, src, len); }/' src/perf/cpu_dispatch.rs
sed -i.bak 's/    generic_memset(dst, val, len);/    unsafe { generic_memset(dst, val, len); }/' src/perf/cpu_dispatch.rs

# Fix RISC-V unsafe pointer arithmetic
sed -i.bak 's/let _end = start.add(len);/let _end = unsafe { start.add(len) };/' src/arch/riscv.rs

# Fix context switch calls
sed -i.bak 's/A::context_switch(prev_context, next_context);/unsafe { A::context_switch(prev_context, next_context); }/' src/perf/context_switch_opt.rs

# Fix inline assembly
sed -i.bak 's/            core::arch::asm!/            unsafe { core::arch::asm!/' src/perf/context_switch_opt.rs
sed -i.bak 's/            );/            ); }/' src/perf/context_switch_opt.rs

# Clean up backup files
find . -name "*.bak" -delete

echo "Critical errors fixed. Running cargo build..."
cargo build --release --all-features
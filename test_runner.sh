#!/bin/bash

echo "🧵 Preemptive Multithreading Library Test Suite 🧵"
echo "================================================="

echo ""
echo "📝 Running unit tests..."
cargo test

echo ""
echo "🔍 Checking code formatting..."
cargo fmt --all -- --check

echo ""
echo "📋 Running clippy lints..."
cargo clippy --all-targets --all-features -- -D warnings

echo ""
echo "🏗️  Building all binaries..."
cargo build --bin example --features std
cargo build --bin benchmark --features std
cargo build --bin interactive_demo --features std
cargo build --bin stress_test --features std
cargo build --bin priority_test --features std
cargo build --bin stack_overflow_test --features std
cargo build --bin test_preemption --features std

echo ""
echo "📚 Building documentation..."
cargo doc --no-deps

echo ""
echo "🚀 Quick Demo (Basic Threading)..."
echo "Running basic cooperative threading example:"
cargo run --bin example --features std

echo ""
echo "✅ All tests completed!"
echo ""
echo "To see the full interactive demo:"
echo "  cargo run --bin interactive_demo --features std"
echo ""
echo "To run benchmarks:"
echo "  cargo run --bin benchmark --features std"
echo ""
echo "To test different scenarios:"
echo "  cargo run --bin stress_test --features std      # 10 threads stress test"
echo "  cargo run --bin priority_test --features std    # Priority scheduling demo"
echo "  cargo run --bin test_preemption --features std  # Preemptive scheduling"
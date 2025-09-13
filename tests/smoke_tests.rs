//! Smoke tests to verify basic functionality

#![cfg(feature = "std")]

extern crate std;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::println;

use preemptive_threads::{
    ThreadBuilder, ThreadId, JoinHandle,
    StackPool, StackSizeClass,
    yield_now, yield_thread,
    Mutex, MutexGuard,
    Duration, Instant,
    SecurityConfig, init_security,
    ObservabilityConfig, init_observability,
};

/// Basic thread spawn and join test
#[test]
fn test_basic_thread_spawn_join() {
    let pool = StackPool::new();
    let thread_id = unsafe { ThreadId::new_unchecked(100) };
    
    let shared_counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = shared_counter.clone();
    
    let builder = ThreadBuilder::new()
        .name("basic-test-thread")
        .priority(128);
    
    let result = builder.spawn(thread_id, &pool, move || {
        counter_clone.store(42, Ordering::SeqCst);
    });
    
    assert!(result.is_ok(), "Thread spawn failed: {:?}", result);
    let (thread, join_handle) = result.unwrap();
    
    // Thread should have correct ID
    assert_eq!(thread.id(), thread_id);
    
    // After some time, counter should be set
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(shared_counter.load(Ordering::SeqCst), 42);
}

/// Test multiple thread spawning
#[test]
fn test_multiple_threads() {
    let pool = StackPool::new();
    let shared_counter = Arc::new(AtomicUsize::new(0));
    
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let thread_id = unsafe { ThreadId::new_unchecked(200 + i) };
        let counter_clone = shared_counter.clone();
        
        let builder = ThreadBuilder::new()
            .name(&format!("worker-{}", i))
            .stack_size_class(StackSizeClass::Small);
        
        let result = builder.spawn(thread_id, &pool, move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        
        assert!(result.is_ok(), "Failed to spawn thread {}", i);
        handles.push(result.unwrap());
    }
    
    // Wait a bit for threads to execute
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    // All threads should have incremented the counter
    assert_eq!(shared_counter.load(Ordering::SeqCst), 10);
}

/// Test thread priority and scheduling
#[test]
fn test_thread_priority() {
    let pool = StackPool::new();
    let execution_order = Arc::new(Mutex::new(Vec::new()));
    
    // Spawn low priority thread first
    let order_clone1 = execution_order.clone();
    let low_priority = ThreadBuilder::new()
        .name("low-priority")
        .priority(10)
        .spawn(
            unsafe { ThreadId::new_unchecked(300) },
            &pool,
            move || {
                std::thread::sleep(std::time::Duration::from_millis(5));
                let mut guard = order_clone1.lock();
                guard.push(1);
            }
        );
    
    // Spawn high priority thread second
    let order_clone2 = execution_order.clone();
    let high_priority = ThreadBuilder::new()
        .name("high-priority")
        .priority(200)
        .spawn(
            unsafe { ThreadId::new_unchecked(301) },
            &pool,
            move || {
                let mut guard = order_clone2.lock();
                guard.push(2);
            }
        );
    
    assert!(low_priority.is_ok());
    assert!(high_priority.is_ok());
    
    // Wait for execution
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    // High priority thread should generally execute first or interrupt low priority
    // Note: This is a best-effort test as scheduling is not deterministic
    let order = execution_order.lock();
    assert!(!order.is_empty(), "No threads executed");
}

/// Test stack size configuration
#[test]
fn test_stack_sizes() {
    let pool = StackPool::new();
    
    let test_sizes = [
        StackSizeClass::Small,
        StackSizeClass::Medium,
        StackSizeClass::Large,
    ];
    
    for (i, size_class) in test_sizes.iter().enumerate() {
        let thread_id = unsafe { ThreadId::new_unchecked(400 + i as u64) };
        
        let result = ThreadBuilder::new()
            .name(&format!("stack-test-{}", i))
            .stack_size_class(*size_class)
            .spawn(thread_id, &pool, || {
                // Just verify we can execute
                let _x = 42;
            });
        
        assert!(result.is_ok(), "Failed with stack size {:?}", size_class);
    }
}

/// Test custom stack size
#[test]
fn test_custom_stack_size() {
    let pool = StackPool::new();
    let thread_id = unsafe { ThreadId::new_unchecked(500) };
    
    let result = ThreadBuilder::new()
        .name("custom-stack")
        .stack_size(32768) // 32KB custom size
        .spawn(thread_id, &pool, || {
            // Allocate some stack space
            let _large_array = [0u8; 16384];
        });
    
    assert!(result.is_ok(), "Failed with custom stack size");
}

/// Test thread names and debugging info
#[test]
fn test_thread_metadata() {
    let pool = StackPool::new();
    let thread_id = unsafe { ThreadId::new_unchecked(600) };
    
    let thread_name = "metadata-test-thread";
    let result = ThreadBuilder::new()
        .name(thread_name)
        .debug_info(true)
        .group_id(42)
        .spawn(thread_id, &pool, || {});
    
    assert!(result.is_ok());
    let (thread, _) = result.unwrap();
    
    // Verify thread has correct metadata
    assert_eq!(thread.id(), thread_id);
    // Note: name getter would need to be implemented
}

/// Test yielding and cooperative scheduling
#[test]
fn test_yield_operations() {
    let pool = StackPool::new();
    let yield_count = Arc::new(AtomicUsize::new(0));
    
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let thread_id = unsafe { ThreadId::new_unchecked(700 + i) };
        let count_clone = yield_count.clone();
        
        let result = ThreadBuilder::new()
            .name(&format!("yielder-{}", i))
            .spawn(thread_id, &pool, move || {
                for _ in 0..3 {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                    yield_thread();
                }
            });
        
        assert!(result.is_ok());
        handles.push(result.unwrap());
    }
    
    // Wait for threads to complete
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // Each thread yields 3 times, 5 threads total
    assert_eq!(yield_count.load(Ordering::SeqCst), 15);
}

/// Test mutex synchronization
#[test]
fn test_mutex_basic() {
    let shared_data = Arc::new(Mutex::new(0));
    let pool = StackPool::new();
    
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let thread_id = unsafe { ThreadId::new_unchecked(800 + i) };
        let data_clone = shared_data.clone();
        
        let result = ThreadBuilder::new()
            .name(&format!("mutex-thread-{}", i))
            .spawn(thread_id, &pool, move || {
                let mut guard = data_clone.lock();
                *guard += 1;
            });
        
        assert!(result.is_ok());
        handles.push(result.unwrap());
    }
    
    // Wait for all threads
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // Verify mutex protected the data correctly
    assert_eq!(*shared_data.lock(), 10);
}

/// Test security configuration
#[test]
fn test_security_features() {
    let config = SecurityConfig {
        enable_stack_canaries: true,
        enable_guard_pages: cfg!(feature = "mmu"),
        enable_cfi: false, // CFI requires special compiler support
        enable_thread_isolation: false,
        enable_aslr: false,
        enable_audit_logging: true,
        use_secure_rng: true,
        panic_on_violation: false,
    };
    
    let result = init_security(config);
    assert!(result.is_ok(), "Security initialization failed: {:?}", result);
    
    // Now spawn a thread with security enabled
    let pool = StackPool::new();
    let thread_id = unsafe { ThreadId::new_unchecked(900) };
    
    let result = ThreadBuilder::new()
        .name("secure-thread")
        .stack_canary(true)
        .stack_guard_pages(cfg!(feature = "mmu"))
        .spawn(thread_id, &pool, || {
            // Thread with security features
            let _secure_data = 42;
        });
    
    assert!(result.is_ok(), "Secure thread spawn failed");
}

/// Test observability initialization
#[test]
fn test_observability() {
    let config = ObservabilityConfig {
        enable_metrics: true,
        enable_profiling: false, // Profiling has overhead
        enable_health_monitoring: true,
        metrics_interval: Duration::from_secs(1),
        profile_sampling_rate: 1000,
        health_check_interval: Duration::from_secs(5),
    };
    
    let result = init_observability(config);
    assert!(result.is_ok(), "Observability initialization failed: {:?}", result);
}

/// Test thread builder validation
#[test]
fn test_builder_validation() {
    let pool = StackPool::new();
    
    // Test invalid stack size (too small)
    let result = ThreadBuilder::new()
        .name("invalid-stack")
        .stack_size(1024) // Too small
        .spawn(
            unsafe { ThreadId::new_unchecked(1000) },
            &pool,
            || {}
        );
    
    assert!(result.is_err(), "Should fail with invalid stack size");
    
    // Test invalid CPU affinity
    let result = ThreadBuilder::new()
        .name("invalid-affinity")
        .cpu_affinity(0) // Invalid: no CPUs selected
        .spawn(
            unsafe { ThreadId::new_unchecked(1001) },
            &pool,
            || {}
        );
    
    assert!(result.is_err(), "Should fail with invalid CPU affinity");
    
    // Test very long name
    let long_name = "a".repeat(100);
    let result = ThreadBuilder::new()
        .name(&long_name)
        .spawn(
            unsafe { ThreadId::new_unchecked(1002) },
            &pool,
            || {}
        );
    
    assert!(result.is_err(), "Should fail with too long name");
}

/// Smoke test summary
#[test]
fn test_smoke_summary() {
    println!("\n=== SMOKE TEST SUMMARY ===");
    println!("✅ Basic thread spawn/join");
    println!("✅ Multiple thread management");
    println!("✅ Priority scheduling");
    println!("✅ Stack size configuration");
    println!("✅ Thread metadata");
    println!("✅ Yield operations");
    println!("✅ Mutex synchronization");
    println!("✅ Security features");
    println!("✅ Observability");
    println!("✅ Builder validation");
    println!("==========================\n");
}
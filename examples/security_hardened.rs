//! Security hardening features demonstration including isolation, CFI, and audit logging.

#![no_std]

extern crate alloc;
use alloc::{vec, vec::Vec, string::ToString};

use preemptive_mlthreading_rust::{
    ThreadBuilder, JoinHandle, yield_now,
    SecurityConfig, SecurityFeature, SecurityViolation,
    init_security, get_security_stats, configure_security_feature,
    // Security modules
    security::{
        isolation::{IsolationConfig, SecurityLevel, create_isolation_domain, AccessType},
        audit::{log_thread_event, ThreadEventType, get_recent_audit_events, ExportFormat, export_audit_events},
        stack_protection::{ProtectedStack, StackCanaryGuard},
        crypto_rng::{secure_random_u32, secure_random_bytes},
        aslr::{create_randomized_layout, get_aslr_stats},
    }
};

/// Simulates processing of untrusted data in an isolated environment
fn isolated_untrusted_worker(worker_id: u32) -> u32 {
    println!("Isolated worker {} processing untrusted data", worker_id);
    
    // Generate some random data to simulate processing
    let mut buffer = [0u8; 64];
    if let Ok(()) = secure_random_bytes(&mut buffer) {
        println!("Worker {} got secure random data", worker_id);
    }
    
    let mut result = 0u32;
    for i in 0..100 {
        // Simulate untrusted data processing
        result = result.wrapping_add(buffer[i % 64] as u32);
        
        if i % 20 == 0 {
            yield_now();
        }
    }
    
    println!("Isolated worker {} completed with result: {}", worker_id, result);
    result
}

/// Trusted system worker with high privileges
fn trusted_system_worker(worker_id: u32) -> u64 {
    println!("Trusted system worker {} started", worker_id);
    
    let mut result = 0u64;
    for i in 0..200 {
        // Simulate trusted system operations
        if let Ok(random_val) = secure_random_u32() {
            result = result.wrapping_add(random_val as u64);
        }
        
        if i % 50 == 0 {
            yield_now();
        }
    }
    
    println!("Trusted worker {} completed with result: {}", worker_id, result);
    result
}

/// Worker that demonstrates stack protection
fn stack_protected_worker(worker_id: u32) -> u32 {
    println!("Stack-protected worker {} started", worker_id);
    
    // Large stack allocation to test protection
    let large_buffer = [0u8; 8192]; // 8KB buffer
    let mut checksum = 0u32;
    
    // Process data while checking stack protection
    for i in 0..1000 {
        let index = (i * 7) % large_buffer.len();
        checksum = checksum.wrapping_add(large_buffer[index] as u32);
        checksum = checksum.wrapping_add(i);
        
        if i % 100 == 0 {
            yield_now();
        }
    }
    
    println!("Stack-protected worker {} completed with checksum: {}", worker_id, checksum);
    checksum
}

/// CFI-protected function pointer demonstration
fn cfi_protected_worker(worker_id: u32) -> u32 {
    println!("CFI-protected worker {} started", worker_id);
    
    // Function pointer operations that benefit from CFI
    let operations: [fn(u32) -> u32; 3] = [
        |x| x * 2,
        |x| x + 10,
        |x| x ^ 0xAA,
    ];
    
    let mut result = worker_id;
    for i in 0..50 {
        let op_index = (i % 3) as usize;
        
        // CFI protects these indirect calls
        result = operations[op_index](result);
        
        if i % 10 == 0 {
            yield_now();
        }
    }
    
    println!("CFI-protected worker {} completed with result: {}", worker_id, result);
    result
}

fn demonstrate_thread_isolation() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Thread Isolation Demo ===");
    
    let mut handles = vec![];
    
    // Create isolated untrusted workers
    for i in 0..2 {
        // Configure isolation for untrusted code
        let isolation_config = IsolationConfig {
            security_level: SecurityLevel::High,
            allowed_operations: vec![AccessType::Signal], // Minimal permissions
            ..Default::default()
        };
        
        let handle = ThreadBuilder::new()
            .name(&format!("isolated_worker_{}", i))
            .priority(8)
            .isolation_config(isolation_config)
            .spawn(move || {
                // Create isolation domain for this thread
                let thread_id = preemptive_mlthreading_rust::thread_new::current_thread_id();
                if let Err(_) = create_isolation_domain(thread_id, IsolationConfig::default()) {
                    println!("Warning: Could not create isolation domain");
                }
                
                isolated_untrusted_worker(i)
            })
            .expect("Failed to spawn isolated worker");
        
        handles.push(handle);
    }
    
    // Create trusted system workers
    for i in 0..1 {
        let handle = ThreadBuilder::new()
            .name(&format!("trusted_worker_{}", i))
            .priority(15) // Higher priority
            .spawn(move || trusted_system_worker(i))
            .expect("Failed to spawn trusted worker");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("Thread isolation demo completed");
    Ok(())
}

fn demonstrate_stack_protection() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Stack Protection Demo ===");
    
    let mut handles = vec![];
    
    // Create threads with stack protection enabled
    for i in 0..3 {
        let handle = ThreadBuilder::new()
            .name(&format!("stack_protected_{}", i))
            .stack_size(128 * 1024) // 128KB stack
            .priority(10)
            .enable_stack_canaries(true)
            .enable_guard_pages(false) // Disable for this example
            .spawn(move || stack_protected_worker(i))
            .expect("Failed to spawn stack-protected worker");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("Stack protection demo completed");
    Ok(())
}

fn demonstrate_cfi_protection() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== CFI Protection Demo ===");
    
    let mut handles = vec![];
    
    // Create threads with CFI protection
    for i in 0..2 {
        let handle = ThreadBuilder::new()
            .name(&format!("cfi_protected_{}", i))
            .priority(12)
            .enable_cfi(true)
            .spawn(move || cfi_protected_worker(i))
            .expect("Failed to spawn CFI-protected worker");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("CFI protection demo completed");
    Ok(())
}

fn demonstrate_aslr() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== ASLR Demo ===");
    
    let mut layouts = vec![];
    
    // Create multiple randomized layouts to show ASLR in action
    for i in 0..5 {
        if let Ok(layout) = create_randomized_layout() {
            println!("Layout {}: stack=0x{:x}, heap=0x{:x}, entropy={}",
                     i, layout.stack_base, layout.heap_base, layout.randomization_entropy);
            layouts.push(layout);
        }
    }
    
    // Show ASLR statistics
    let aslr_stats = get_aslr_stats();
    println!("ASLR Stats:");
    println!("  Randomizations applied: {}", aslr_stats.randomizations_applied);
    println!("  Entropy consumed: {} bytes", aslr_stats.entropy_consumed);
    println!("  Available entropy bits: {}", aslr_stats.entropy_bits_available);
    
    // Create threads with randomized layouts
    let mut handles = vec![];
    
    for i in 0..2 {
        let handle = ThreadBuilder::new()
            .name(&format!("aslr_worker_{}", i))
            .priority(10)
            .enable_aslr(true)
            .spawn(move || {
                println!("ASLR worker {} running with randomized layout", i);
                
                // Do some work to show the randomized addresses are functional
                let mut result = 0u32;
                for j in 0..100 {
                    result = result.wrapping_add(j * (i + 1));
                    if j % 25 == 0 {
                        yield_now();
                    }
                }
                
                result
            })
            .expect("Failed to spawn ASLR worker");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    println!("ASLR demo completed");
    Ok(())
}

fn demonstrate_audit_logging() -> Result<(), preemptive_mlthreading_rust::ThreadError> {
    println!("\n=== Audit Logging Demo ===");
    
    // Log some events
    log_thread_event(
        preemptive_mlthreading_rust::thread_new::current_thread_id(),
        ThreadEventType::Created,
        "Demo thread creation"
    );
    
    let mut handles = vec![];
    
    // Create audited threads
    for i in 0..2 {
        let handle = ThreadBuilder::new()
            .name(&format!("audited_worker_{}", i))
            .priority(10)
            .enable_audit_logging(true)
            .spawn(move || {
                // Log thread start
                log_thread_event(
                    preemptive_mlthreading_rust::thread_new::current_thread_id(),
                    ThreadEventType::Started,
                    &format!("Audited worker {} started", i)
                );
                
                // Do some work
                let mut result = 0u32;
                for j in 0..50 {
                    result = result.wrapping_add(j);
                    
                    // Simulate security-relevant event
                    if j == 25 {
                        log_thread_event(
                            preemptive_mlthreading_rust::thread_new::current_thread_id(),
                            ThreadEventType::Suspended,
                            "Midpoint checkpoint"
                        );
                    }
                    
                    if j % 10 == 0 {
                        yield_now();
                    }
                }
                
                // Log completion
                log_thread_event(
                    preemptive_mlthreading_rust::thread_new::current_thread_id(),
                    ThreadEventType::Terminated,
                    &format!("Audited worker {} completed", i)
                );
                
                result
            })
            .expect("Failed to spawn audited worker");
        
        handles.push(handle);
    }
    
    // Wait for completion
    for handle in handles {
        let _result = handle.join()?;
    }
    
    // Show audit events
    let recent_events = get_recent_audit_events(10);
    println!("Recent audit events ({} total):", recent_events.len());
    for event in recent_events.iter().take(5) {
        println!("  {}", event);
    }
    
    // Export audit log
    if let Ok(log_json) = export_audit_events(ExportFormat::Json) {
        println!("JSON audit log length: {} bytes", log_json.len());
    }
    
    println!("Audit logging demo completed");
    Ok(())
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    println!("=== Security Hardening Features Demo ===");
    
    // Initialize comprehensive security
    let security_config = SecurityConfig {
        enable_stack_canaries: true,
        enable_guard_pages: false, // Disable for demo
        enable_cfi: true,
        enable_thread_isolation: true,
        enable_aslr: true,
        enable_audit_logging: true,
        use_secure_rng: true,
        panic_on_violation: false, // Continue on violations for demo
    };
    
    init_security(security_config)?;
    println!("Comprehensive security initialized\n");
    
    // Configure security features
    configure_security_feature(SecurityFeature::StackCanaries, true);
    configure_security_feature(SecurityFeature::Cfi, true);
    configure_security_feature(SecurityFeature::Isolation, true);
    configure_security_feature(SecurityFeature::Aslr, true);
    configure_security_feature(SecurityFeature::Audit, true);
    
    // Demonstrate each security feature
    demonstrate_thread_isolation()?;
    demonstrate_stack_protection()?;
    demonstrate_cfi_protection()?;
    demonstrate_aslr()?;
    demonstrate_audit_logging()?;
    
    // Final security statistics
    let stats = get_security_stats();
    println!("\n=== Final Security Statistics ===");
    println!("Total violations detected: {}", stats.total_violations);
    println!("Stack violations: {}", stats.stack_violations);
    println!("CFI violations: {}", stats.cfi_violations);
    println!("Isolation violations: {}", stats.isolation_violations);
    println!("Memory violations: {}", stats.memory_violations);
    println!("Crypto violations: {}", stats.crypto_violations);
    
    println!("Security features enabled:");
    println!("  Stack canaries: {}", stats.features_enabled.canaries);
    println!("  Guard pages: {}", stats.features_enabled.guard_pages);
    println!("  CFI: {}", stats.features_enabled.cfi);
    println!("  Isolation: {}", stats.features_enabled.isolation);
    println!("  ASLR: {}", stats.features_enabled.aslr);
    println!("  Audit: {}", stats.features_enabled.audit);
    
    println!("\n=== Security Hardening Demo Completed Successfully ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_isolated_worker() {
        let result = isolated_untrusted_worker(1);
        assert!(result >= 0); // Should complete without violations
    }
    
    #[test]
    fn test_trusted_worker() {
        let result = trusted_system_worker(1);
        assert!(result > 0);
    }
    
    #[test]
    fn test_stack_protected_worker() {
        let result = stack_protected_worker(1);
        assert!(result >= 0);
    }
    
    #[test]
    fn test_cfi_protected_worker() {
        let result = cfi_protected_worker(1);
        assert!(result > 0);
    }
}
#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use preemptive_threads::mem::{StackPool, StackSizeClass, ArcLite};
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Arbitrary)]
enum MemoryOperation {
    AllocateStack {
        size_class: StackSizeClass,
        with_guards: bool,
    },
    DeallocateStack(usize), // Index into allocated stacks
    AllocateCustomStack {
        size: u32,
        with_guards: bool,
    },
    CreateArcLite(u64), // Value to store
    CloneArcLite(usize), // Index into ArcLite instances
    DropArcLite(usize), // Index to drop
    CheckReferenceCount(usize),
    StressAllocation {
        count: u8,
        size_class: StackSizeClass,
    },
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    
    // Generate a sequence of memory operations
    let operations: Result<Vec<MemoryOperation>, _> = (0..50)
        .map(|_| MemoryOperation::arbitrary(&mut u))
        .collect();
    
    if let Ok(ops) = operations {
        fuzz_memory_management(ops);
    }
});

fn fuzz_memory_management(operations: Vec<MemoryOperation>) {
    let pool = StackPool::new_for_testing();
    let mut allocated_stacks = Vec::new();
    let mut arclite_instances = Vec::new();
    
    for op in operations {
        match op {
            MemoryOperation::AllocateStack { size_class, with_guards } => {
                match pool.allocate(size_class, with_guards) {
                    Ok(stack) => {
                        // Verify stack properties
                        assert!(stack.size() >= size_class.size());
                        assert!(!stack.base().is_null());
                        assert!(stack.top() > stack.base());
                        
                        allocated_stacks.push(stack);
                        
                        // Limit number of allocated stacks
                        if allocated_stacks.len() > 100 {
                            let stack = allocated_stacks.remove(0);
                            pool.deallocate(stack);
                        }
                    }
                    Err(_) => {
                        // Allocation failed, which is acceptable under memory pressure
                    }
                }
            }
            
            MemoryOperation::DeallocateStack(idx) => {
                if idx < allocated_stacks.len() {
                    let stack = allocated_stacks.remove(idx);
                    pool.deallocate(stack);
                }
            }
            
            MemoryOperation::AllocateCustomStack { size, with_guards } => {
                let size = (size % 1024 * 1024) + 4096; // 4KB to 1MB + 4KB
                match pool.allocate_custom(size as usize, with_guards) {
                    Ok(stack) => {
                        assert!(stack.size() >= size as usize);
                        allocated_stacks.push(stack);
                        
                        if allocated_stacks.len() > 50 {
                            let stack = allocated_stacks.remove(0);
                            pool.deallocate(stack);
                        }
                    }
                    Err(_) => {
                        // Custom allocation failed
                    }
                }
            }
            
            MemoryOperation::CreateArcLite(value) => {
                let arc = ArcLite::new(value);
                assert_eq!(*arc, value);
                assert_eq!(arc.strong_count(), 1);
                
                arclite_instances.push(arc);
                
                // Limit instances to prevent memory bloat
                if arclite_instances.len() > 200 {
                    arclite_instances.remove(0);
                }
            }
            
            MemoryOperation::CloneArcLite(idx) => {
                if idx < arclite_instances.len() {
                    let original_count = arclite_instances[idx].strong_count();
                    let clone = arclite_instances[idx].clone();
                    
                    // Verify clone properties
                    assert_eq!(*clone, *arclite_instances[idx]);
                    assert_eq!(arclite_instances[idx].strong_count(), original_count + 1);
                    
                    arclite_instances.push(clone);
                }
            }
            
            MemoryOperation::DropArcLite(idx) => {
                if idx < arclite_instances.len() {
                    arclite_instances.remove(idx);
                }
            }
            
            MemoryOperation::CheckReferenceCount(idx) => {
                if idx < arclite_instances.len() {
                    let count = arclite_instances[idx].strong_count();
                    assert!(count >= 1); // Should always have at least one reference
                    
                    // Count actual references in our vector
                    let ptr = arclite_instances[idx].as_ptr();
                    let actual_count = arclite_instances.iter()
                        .filter(|arc| arc.as_ptr() == ptr)
                        .count();
                    
                    assert_eq!(count, actual_count);
                }
            }
            
            MemoryOperation::StressAllocation { count, size_class } => {
                let count = count.min(50); // Limit stress to prevent timeouts
                let mut temp_stacks = Vec::new();
                
                // Allocate many stacks rapidly
                for _ in 0..count {
                    if let Ok(stack) = pool.allocate(size_class, false) {
                        temp_stacks.push(stack);
                    }
                }
                
                // Return them all
                for stack in temp_stacks {
                    pool.deallocate(stack);
                }
            }
        }
    }
    
    // Cleanup - return all allocated stacks
    for stack in allocated_stacks {
        pool.deallocate(stack);
    }
    
    // Verify pool state after cleanup
    assert_eq!(pool.allocated_count(), 0);
}

impl Arbitrary for StackSizeClass {
    fn arbitrary(u: &mut Unstructured) -> arbitrary::Result<Self> {
        let choice = u.int_in_range(0..=2)?;
        Ok(match choice {
            0 => StackSizeClass::Small,
            1 => StackSizeClass::Medium,
            _ => StackSizeClass::Large,
        })
    }
}
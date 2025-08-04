#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack_guard::{ProtectedStack, StackGuard};
    use crate::atomic_scheduler::ATOMIC_SCHEDULER;
    use crate::safe_api::{ThreadBuilder, Mutex};
    use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};
    
    /// Stack allocation helper for tests
    struct TestStack {
        memory: Box<[u8; 65536]>,
    }
    
    impl TestStack {
        fn new() -> Self {
            Self {
                memory: Box::new([0; 65536]),
            }
        }
        
        fn as_static(&'static mut self) -> &'static mut [u8] {
            // Safe because we ensure the lifetime in tests
            unsafe {
                core::slice::from_raw_parts_mut(
                    self.memory.as_mut_ptr(),
                    self.memory.len()
                )
            }
        }
    }
    
    #[test]
    fn test_atomic_scheduler_basic() {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        
        // Use leaked memory for 'static requirement in tests
        let stack1 = Box::leak(Box::new([0u8; 65536]));
        let stack2 = Box::leak(Box::new([0u8; 65536]));
        
        fn thread1_fn() {
            for _ in 0..10 {
                COUNTER.fetch_add(1, Ordering::Relaxed);
                crate::sync::yield_thread();
            }
        }
        
        fn thread2_fn() {
            for _ in 0..10 {
                COUNTER.fetch_add(10, Ordering::Relaxed);
                crate::sync::yield_thread();
            }
        }
        
        let thread1 = ATOMIC_SCHEDULER.spawn_thread(stack1, thread1_fn, 5).unwrap();
        let thread2 = ATOMIC_SCHEDULER.spawn_thread(stack2, thread2_fn, 5).unwrap();
        
        // Simulate scheduling
        for _ in 0..50 {
            if let Some(next_thread) = ATOMIC_SCHEDULER.schedule() {
                // In real usage, we'd switch context here
                println!("Would schedule thread {}", next_thread);
            }
        }
        
        assert!(COUNTER.load(Ordering::Relaxed) > 0);
    }
    
    #[test]
    fn test_priority_queue() {
        use crate::atomic_scheduler::PriorityQueue;
        
        let queue = PriorityQueue::new();
        
        // Add threads with different priorities
        assert!(queue.enqueue(1, 7)); // High priority
        assert!(queue.enqueue(2, 3)); // Low priority
        assert!(queue.enqueue(3, 5)); // Medium priority
        
        // Should dequeue in priority order
        assert_eq!(queue.dequeue(), Some(1)); // Priority 7
        assert_eq!(queue.dequeue(), Some(3)); // Priority 5
        assert_eq!(queue.dequeue(), Some(2)); // Priority 3
        assert_eq!(queue.dequeue(), None);
    }
    
    #[test]
    fn test_stack_guard() {
        let mut memory = [0u8; 8192];
        let guard = StackGuard::default();
        
        unsafe {
            let stack = ProtectedStack::new(&mut memory, guard);
            
            // Check initial state
            match stack.check_overflow() {
                crate::stack_guard::StackStatus::Ok { used_bytes, free_bytes } => {
                    assert!(used_bytes < 100);
                    assert!(free_bytes > 4000);
                }
                _ => panic!("Stack should be OK initially"),
            }
            
            // Get stats
            let stats = stack.get_stats();
            assert_eq!(stats.total_size, 8192);
            assert!(stats.usable_size < 8192); // Less due to guards
        }
    }
    
    #[test]
    fn test_mutex() {
        static DATA: Mutex<u32> = Mutex::new(0);
        static FLAG: AtomicBool = AtomicBool::new(false);
        
        // Lock and modify
        {
            let mut guard = DATA.lock();
            *guard = 42;
            
            // Try lock should fail while locked
            assert!(DATA.try_lock().is_none());
        }
        
        // Lock released, try_lock should succeed
        if let Some(guard) = DATA.try_lock() {
            assert_eq!(*guard, 42);
            FLAG.store(true, Ordering::Relaxed);
        }
        
        assert!(FLAG.load(Ordering::Relaxed));
    }
    
    #[test]
    fn test_circular_buffer() {
        use crate::atomic_scheduler::CircularBuffer;
        
        let buffer = CircularBuffer::new();
        
        // Test enqueue/dequeue
        assert!(buffer.enqueue(1));
        assert!(buffer.enqueue(2));
        assert!(buffer.enqueue(3));
        
        assert_eq!(buffer.dequeue(), Some(1));
        assert_eq!(buffer.dequeue(), Some(2));
        assert_eq!(buffer.dequeue(), Some(3));
        assert_eq!(buffer.dequeue(), None);
        
        // Test wrap around
        for i in 0..30 {
            assert!(buffer.enqueue(i));
        }
        
        for i in 0..30 {
            assert_eq!(buffer.dequeue(), Some(i));
        }
    }
    
    #[test]
    fn test_signal_safe_preemption() {
        use crate::signal_safe;
        
        // Test preemption counter
        let initial_count = signal_safe::get_preemption_count();
        
        // Simulate preemption
        assert!(!signal_safe::is_preemption_pending());
        
        // Would be set by signal handler
        unsafe {
            signal_safe::signal_safe_handler(0);
        }
        
        assert!(signal_safe::is_preemption_pending());
        assert_eq!(signal_safe::get_preemption_count(), initial_count + 1);
        
        // Clear pending
        signal_safe::clear_preemption_pending();
        assert!(!signal_safe::is_preemption_pending());
    }
    
    #[test]
    fn test_thread_builder() {
        let builder = ThreadBuilder::new()
            .stack_size(128 * 1024)
            .priority(7)
            .name("test_thread");
        
        // Can't actually spawn in no_std tests, but API should compile
        match builder.spawn(|| {
            println!("Hello from thread");
        }) {
            Err(crate::error::ThreadError::NotImplemented) => {
                // Expected in no_std
            }
            _ => panic!("Should return NotImplemented"),
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::{thread::*, scheduler::*, error::*};
    use std::{vec, vec::Vec};

    #[test]
    fn test_thread_creation() {
        let mut stack = vec![0u8; 32 * 1024];
        let stack: &'static mut [u8] = unsafe { std::mem::transmute(stack.as_mut_slice()) };
        
        fn dummy_fn() {}
        
        let thread = Thread::new(0, stack, dummy_fn, 1);
        
        assert_eq!(thread.id, 0);
        assert_eq!(thread.state, ThreadState::Ready);
        assert_eq!(thread.priority, 1);
        assert!(!thread.check_stack_overflow());
    }

    #[test]
    fn test_thread_states() {
        assert_eq!(ThreadState::Ready, ThreadState::Ready);
        assert_ne!(ThreadState::Ready, ThreadState::Running);
        assert_ne!(ThreadState::Ready, ThreadState::Blocked);
        assert_ne!(ThreadState::Ready, ThreadState::Finished);
    }

    #[test]
    fn test_thread_is_runnable() {
        let mut stack = vec![0u8; 32 * 1024];
        let stack: &'static mut [u8] = unsafe { std::mem::transmute(stack.as_mut_slice()) };
        
        fn dummy_fn() {}
        
        let mut thread = Thread::new(0, stack, dummy_fn, 1);
        
        // Ready state should be runnable
        assert!(thread.is_runnable());
        
        // Running state should be runnable
        thread.state = ThreadState::Running;
        assert!(thread.is_runnable());
        
        // Blocked state should not be runnable
        thread.state = ThreadState::Blocked;
        assert!(!thread.is_runnable());
        
        // Finished state should not be runnable
        thread.state = ThreadState::Finished;
        assert!(!thread.is_runnable());
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new();
        assert_eq!(scheduler.get_current_thread(), None);
    }

    #[test]
    fn test_thread_spawn() {
        let mut scheduler = Scheduler::new();
        let mut stack = vec![0u8; 32 * 1024];
        let stack: &'static mut [u8] = unsafe { std::mem::transmute(stack.as_mut_slice()) };
        
        fn test_thread() {}
        
        let result = scheduler.spawn_thread(stack, test_thread, 1);
        assert!(result.is_ok());
        
        let thread_id = result.unwrap();
        let thread = scheduler.get_thread(thread_id);
        assert!(thread.is_some());
        assert_eq!(thread.unwrap().id, thread_id);
    }

    #[test]
    fn test_max_threads_limit() {
        let mut scheduler = Scheduler::new();
        let mut stacks = vec![vec![0u8; 1024]; 33];
        
        fn test_thread() {}
        
        // Spawn maximum number of threads
        for i in 0..32 {
            let stack: &'static mut [u8] = unsafe { 
                std::mem::transmute(stacks[i].as_mut_slice()) 
            };
            let result = scheduler.spawn_thread(stack, test_thread, 1);
            assert!(result.is_ok(), "Failed to spawn thread {}", i);
        }
        
        // 33rd thread should fail
        let stack: &'static mut [u8] = unsafe { 
            std::mem::transmute(stacks[32].as_mut_slice()) 
        };
        let result = scheduler.spawn_thread(stack, test_thread, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ThreadError::MaxThreadsReached);
    }

    #[test]
    fn test_priority_scheduling() {
        let mut scheduler = Scheduler::new();
        let mut stacks = vec![vec![0u8; 1024]; 3];
        
        fn test_thread() {}
        
        // Spawn threads with different priorities
        let stack1: &'static mut [u8] = unsafe { 
            std::mem::transmute(stacks[0].as_mut_slice()) 
        };
        let _low_priority = scheduler.spawn_thread(stack1, test_thread, 1).unwrap();
        
        let stack2: &'static mut [u8] = unsafe { 
            std::mem::transmute(stacks[1].as_mut_slice()) 
        };
        let high_priority = scheduler.spawn_thread(stack2, test_thread, 10).unwrap();
        
        let stack3: &'static mut [u8] = unsafe { 
            std::mem::transmute(stacks[2].as_mut_slice()) 
        };
        let med_priority = scheduler.spawn_thread(stack3, test_thread, 5).unwrap();
        
        // High priority thread should be scheduled first
        let next = scheduler.schedule();
        assert_eq!(next, Some(high_priority));
        
        // Set high priority to running to simulate execution
        scheduler.set_current_thread(Some(high_priority));
        let thread = scheduler.get_thread_mut(high_priority).unwrap();
        thread.state = ThreadState::Running;
        
        // Next schedule should return medium priority
        let next = scheduler.schedule();
        assert_eq!(next, Some(med_priority));
    }

    #[test]
    fn test_error_types() {
        assert_eq!(ThreadError::MaxThreadsReached.as_str(), "Maximum number of threads reached");
        assert_eq!(ThreadError::InvalidThreadId.as_str(), "Invalid thread ID provided");
        assert_eq!(ThreadError::ThreadNotRunnable.as_str(), "Thread is not in a runnable state");
        assert_eq!(ThreadError::StackOverflow.as_str(), "Stack overflow detected");
        assert_eq!(ThreadError::SchedulerFull.as_str(), "Scheduler queue is full");
    }

    #[test]
    fn test_stack_overflow_detection() {
        let mut stack = vec![0u8; 1024];
        let stack: &'static mut [u8] = unsafe { std::mem::transmute(stack.as_mut_slice()) };
        
        fn dummy_fn() {}
        
        let thread = Thread::new(0, stack, dummy_fn, 1);
        
        // Initially no overflow
        assert!(!thread.check_stack_overflow());
        
        // Corrupt the stack guard
        unsafe {
            *(thread.stack_bottom as *mut u64) = 0xBADCAFE;
        }
        
        // Now should detect overflow
        assert!(thread.check_stack_overflow());
    }

    #[test]
    fn test_thread_lifecycle() {
        let mut scheduler = Scheduler::new();
        let mut stack = vec![0u8; 32 * 1024];
        let stack: &'static mut [u8] = unsafe { std::mem::transmute(stack.as_mut_slice()) };
        
        fn test_thread() {}
        
        let thread_id = scheduler.spawn_thread(stack, test_thread, 1).unwrap();
        
        // Thread should start in Ready state
        let thread = scheduler.get_thread(thread_id).unwrap();
        assert_eq!(thread.state, ThreadState::Ready);
        assert!(thread.is_runnable());
        
        // Set to running
        scheduler.set_current_thread(Some(thread_id));
        let thread = scheduler.get_thread(thread_id).unwrap();
        assert_eq!(thread.state, ThreadState::Running);
        assert!(thread.is_runnable());
        
        // Exit thread
        scheduler.exit_current_thread();
        let thread = scheduler.get_thread(thread_id).unwrap();
        assert_eq!(thread.state, ThreadState::Finished);
        assert!(!thread.is_runnable());
    }

    #[test]
    fn test_scheduler_round_robin() {
        let mut scheduler = Scheduler::new();
        let mut stacks = vec![vec![0u8; 1024]; 3];
        
        fn test_thread() {}
        
        // Spawn three threads with equal priority
        let mut thread_ids = Vec::new();
        for i in 0..3 {
            let stack: &'static mut [u8] = unsafe { 
                std::mem::transmute(stacks[i].as_mut_slice()) 
            };
            let id = scheduler.spawn_thread(stack, test_thread, 1).unwrap();
            thread_ids.push(id);
        }
        
        // Verify round-robin scheduling
        let first = scheduler.schedule().unwrap();
        scheduler.set_current_thread(Some(first));
        scheduler.get_thread_mut(first).unwrap().state = ThreadState::Running;
        
        let second = scheduler.schedule().unwrap();
        assert_ne!(first, second);
        
        scheduler.set_current_thread(Some(second));
        scheduler.get_thread_mut(second).unwrap().state = ThreadState::Running;
        
        let third = scheduler.schedule().unwrap();
        assert_ne!(first, third);
        assert_ne!(second, third);
    }
}
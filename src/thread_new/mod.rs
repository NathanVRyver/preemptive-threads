//! New thread abstraction with RAII and memory safety.
//!
//! This module provides the new thread implementation that uses RAII
//! for resource management and eliminates manual memory management.

use crate::mem::{ArcLite, Stack};
use crate::arch::Arch;
// PhantomData and AtomicUsize imports not needed yet
// use core::marker::PhantomData;
use portable_atomic::{AtomicU8, Ordering};

pub mod handle;
pub mod inner;
pub mod builder;

pub use handle::JoinHandle;
pub use builder::ThreadBuilder;

/// Unique identifier for threads.
///
/// Thread IDs are never reused and are guaranteed to be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadId(core::num::NonZeroUsize);

impl ThreadId {
    /// Create a new thread ID.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `id` is non-zero and unique.
    pub unsafe fn new_unchecked(id: usize) -> Self {
        Self(unsafe { core::num::NonZeroUsize::new_unchecked(id) })
    }
    
    /// Get the raw ID value.
    pub fn get(self) -> usize {
        self.0.get()
    }
}

/// Thread execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ThreadState {
    /// Thread is ready to run
    Ready = 0,
    /// Thread is currently running  
    Running = 1,
    /// Thread is blocked waiting for something
    Blocked = 2,
    /// Thread has finished execution
    Finished = 3,
}

/// Main thread handle with RAII resource management.
///
/// This represents a thread and automatically manages its resources
/// through reference counting. When the last reference is dropped,
/// the thread's stack and other resources are automatically cleaned up.
pub struct Thread {
    /// Reference to the thread's internal data
    inner: ArcLite<ThreadInner>,
}

/// Internal thread data shared between Thread and JoinHandle.
pub struct ThreadInner {
    /// Unique thread identifier
    pub id: ThreadId,
    /// Current execution state
    pub state: AtomicU8,
    /// Thread priority (higher = more important)
    pub priority: AtomicU8,
    /// Thread's stack
    pub stack: Option<Stack>,
    /// Architecture-specific saved context
    pub context: Option<*mut <crate::arch::DefaultArch as Arch>::SavedContext>,
    /// Entry point function (simplified for now)
    pub entry_point: Option<fn()>,
    /// Join result storage
    pub join_result: spin::Mutex<Option<()>>, // TODO: Support return values
}

impl Thread {
    /// Create a new thread with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this thread
    /// * `stack` - Stack allocated for this thread
    /// * `entry_point` - Function to execute in this thread
    /// * `priority` - Thread priority (0-255, higher = more important)
    ///
    /// # Returns
    ///
    /// A new Thread instance and corresponding JoinHandle.
    pub fn new(
        id: ThreadId,
        stack: Stack,
        entry_point: fn(),
        priority: u8,
    ) -> (Self, JoinHandle) {
        let inner = ThreadInner {
            id,
            state: AtomicU8::new(ThreadState::Ready as u8),
            priority: AtomicU8::new(priority),
            stack: Some(stack),
            context: None, // Will be initialized when first context switch occurs
            entry_point: Some(entry_point),
            join_result: spin::Mutex::new(None),
        };
        
        let inner_arc = ArcLite::new(inner);
        
        let thread = Self {
            inner: inner_arc.clone(),
        };
        
        let join_handle = JoinHandle {
            inner: inner_arc,
        };
        
        (thread, join_handle)
    }
    
    /// Get the thread's unique identifier.
    pub fn id(&self) -> ThreadId {
        self.inner.id
    }
    
    /// Get the thread's current state.
    pub fn state(&self) -> ThreadState {
        let state_val = self.inner.state.load(Ordering::Acquire);
        match state_val {
            0 => ThreadState::Ready,
            1 => ThreadState::Running,
            2 => ThreadState::Blocked,
            3 => ThreadState::Finished,
            _ => ThreadState::Ready, // Default fallback
        }
    }
    
    /// Set the thread's state.
    ///
    /// # Arguments
    ///
    /// * `new_state` - The new state to set
    pub fn set_state(&self, new_state: ThreadState) {
        self.inner.state.store(new_state as u8, Ordering::Release);
    }
    
    /// Get the thread's priority.
    pub fn priority(&self) -> u8 {
        self.inner.priority.load(Ordering::Acquire)
    }
    
    /// Set the thread's priority.
    ///
    /// # Arguments
    ///
    /// * `new_priority` - The new priority (0-255, higher = more important)
    pub fn set_priority(&self, new_priority: u8) {
        self.inner.priority.store(new_priority, Ordering::Release);
    }
    
    /// Check if this thread is runnable (ready or running).
    pub fn is_runnable(&self) -> bool {
        matches!(self.state(), ThreadState::Ready | ThreadState::Running)
    }
    
    /// Get a pointer to the thread's saved context.
    ///
    /// # Returns
    ///
    /// A pointer to the saved context, or null if not initialized.
    pub fn context_ptr(&self) -> *mut <crate::arch::DefaultArch as Arch>::SavedContext {
        // TODO: This is unsafe and needs proper synchronization
        // For now, return a null pointer as a placeholder
        core::ptr::null_mut()
    }
    
    /// Get the thread's stack bottom (initial stack pointer).
    pub fn stack_bottom(&self) -> Option<*mut u8> {
        self.inner.stack.as_ref().map(|stack| stack.stack_bottom())
    }
    
    /// Check if the thread's stack canary is intact (stack overflow detection).
    pub fn check_stack_integrity(&self) -> bool {
        if let Some(ref stack) = self.inner.stack {
            // Use a fixed canary value for now
            let canary = 0xDEADBEEFCAFEBABE;
            stack.check_canary(canary)
        } else {
            false
        }
    }
}

impl Clone for Thread {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

unsafe impl Send for ThreadInner {}
unsafe impl Sync for ThreadInner {}

/// A reference to a thread that is currently ready to run.
///
/// This type represents a thread that is in the scheduler's ready queue
/// and can be selected to run on a CPU.
pub struct ReadyRef(pub Thread);

/// A reference to a thread that is currently running on a CPU.
///
/// This type represents a thread that is actively executing on a CPU.
pub struct RunningRef(pub Thread);

impl ReadyRef {
    /// Convert this ready reference to a running reference.
    ///
    /// This should be called when the scheduler selects this thread to run.
    pub fn start_running(self) -> RunningRef {
        self.0.set_state(ThreadState::Running);
        RunningRef(self.0)
    }
}

impl RunningRef {
    /// Convert this running reference back to a ready reference.
    ///
    /// This should be called when the thread is preempted or yields.
    pub fn stop_running(self) -> ReadyRef {
        self.0.set_state(ThreadState::Ready);
        ReadyRef(self.0)
    }
    
    /// Mark this thread as blocked.
    ///
    /// This should be called when the thread blocks on I/O or synchronization.
    pub fn block(self) {
        self.0.set_state(ThreadState::Blocked);
    }
    
    /// Mark this thread as finished.
    ///
    /// This should be called when the thread's entry point returns.
    pub fn finish(self) {
        self.0.set_state(ThreadState::Finished);
        
        // Signal any joiners that we're done
        if let Some(mut join_result) = self.0.inner.join_result.try_lock() {
            *join_result = Some(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{StackPool, StackSizeClass};
    
    #[cfg(feature = "std-shim")]
    #[test]
    fn test_thread_creation() {
        let pool = StackPool::new();
        let stack = pool.allocate(StackSizeClass::Small).unwrap();
        let thread_id = unsafe { ThreadId::new_unchecked(1) };
        
        let (thread, _join_handle) = Thread::new(
            thread_id,
            stack,
            || println!("Hello from thread!"),
            128,
        );
        
        assert_eq!(thread.id(), thread_id);
        assert_eq!(thread.state(), ThreadState::Ready);
        assert_eq!(thread.priority(), 128);
        assert!(thread.is_runnable());
    }
    
    #[cfg(feature = "std-shim")]
    #[test]
    fn test_thread_state_transitions() {
        let pool = StackPool::new();
        let stack = pool.allocate(StackSizeClass::Small).unwrap();
        let thread_id = unsafe { ThreadId::new_unchecked(1) };
        
        let (thread, _join_handle) = Thread::new(
            thread_id,
            stack,
            || {},
            128,
        );
        
        // Test state transitions
        assert_eq!(thread.state(), ThreadState::Ready);
        
        thread.set_state(ThreadState::Running);
        assert_eq!(thread.state(), ThreadState::Running);
        
        thread.set_state(ThreadState::Blocked);
        assert_eq!(thread.state(), ThreadState::Blocked);
        assert!(!thread.is_runnable());
        
        thread.set_state(ThreadState::Finished);
        assert_eq!(thread.state(), ThreadState::Finished);
        assert!(!thread.is_runnable());
    }
}
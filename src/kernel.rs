//! Kernel abstraction for managing the threading system.
//!
//! This module provides the main `Kernel` struct that coordinates all
//! threading operations and eliminates global singleton state.

use crate::arch::Arch;
use core::marker::PhantomData;
use portable_atomic::{AtomicBool, AtomicUsize, Ordering};

/// Trait for scheduler implementations.
///
/// This trait defines the interface that all scheduler implementations
/// must provide for thread management.
pub trait Scheduler: Send + Sync {
    /// Thread reference type used by this scheduler.
    type ThreadRef;
    
    /// Enqueue a thread for execution.
    ///
    /// # Arguments
    /// 
    /// * `thread` - Thread to be scheduled for execution
    fn enqueue(&self, thread: Self::ThreadRef);
    
    /// Pick the next thread to run on the given CPU.
    ///
    /// # Arguments
    ///
    /// * `cpu_id` - ID of the CPU requesting the next thread
    ///
    /// # Returns
    ///
    /// The next thread to run, or `None` if no threads are ready.
    fn pick_next(&self, cpu_id: usize) -> Option<Self::ThreadRef>;
    
    /// Handle a scheduler tick for the current thread.
    ///
    /// This is called periodically (typically from timer interrupts)
    /// to allow the scheduler to make preemption decisions.
    ///
    /// # Arguments
    ///
    /// * `current` - Reference to the currently running thread
    fn on_tick(&self, current: Self::ThreadRef);
    
    /// Set the priority of a thread.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - ID of the thread to modify
    /// * `priority` - New priority value (higher values = higher priority)
    fn set_priority(&self, thread_id: ThreadId, priority: u8);
}

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

/// Main kernel handle that manages the threading system.
///
/// This struct coordinates all threading operations and provides a safe
/// interface to the underlying scheduler and architecture abstractions.
///
/// # Type Parameters
///
/// * `A` - Architecture implementation
/// * `S` - Scheduler implementation
pub struct Kernel<A: Arch, S: Scheduler> {
    /// Scheduler instance
    scheduler: S,
    /// Architecture marker (zero-sized)
    _arch: PhantomData<A>,
    /// Whether the kernel has been initialized
    initialized: AtomicBool,
    /// Next thread ID to assign
    next_thread_id: AtomicUsize,
}

impl<A: Arch, S: Scheduler> Kernel<A, S> {
    /// Create a new kernel instance.
    ///
    /// # Arguments
    ///
    /// * `scheduler` - Scheduler implementation to use
    ///
    /// # Returns
    ///
    /// A new kernel instance ready for initialization.
    pub const fn new(scheduler: S) -> Self {
        Self {
            scheduler,
            _arch: PhantomData,
            initialized: AtomicBool::new(false),
            next_thread_id: AtomicUsize::new(1), // Start from 1, never use 0
        }
    }
    
    /// Initialize the kernel.
    ///
    /// This must be called before any threading operations can be performed.
    /// It sets up architecture-specific features and prepares the scheduler.
    ///
    /// # Returns
    ///
    /// `Ok(())` if initialization succeeds, `Err(())` if already initialized.
    pub fn init(&self) -> Result<(), ()> {
        if self.initialized.compare_exchange(
            false, 
            true, 
            Ordering::AcqRel, 
            Ordering::Acquire
        ).is_ok() {
            // TODO: Initialize architecture-specific features
            // TODO: Set up timer interrupts for preemption
            // TODO: Initialize scheduler
            Ok(())
        } else {
            Err(()) // Already initialized
        }
    }
    
    /// Check if the kernel has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
    
    /// Generate a new unique thread ID.
    ///
    /// Thread IDs are never reused and are guaranteed to be unique
    /// for the lifetime of the kernel instance.
    pub fn next_thread_id(&self) -> ThreadId {
        let id = self.next_thread_id.fetch_add(1, Ordering::AcqRel);
        // Safety: We start from 1 and only increment, so this will never be zero
        unsafe { ThreadId::new_unchecked(id) }
    }
    
    /// Get a reference to the scheduler.
    pub fn scheduler(&self) -> &S {
        &self.scheduler
    }
    
    /// Spawn a new thread.
    ///
    /// # Arguments
    ///
    /// * `entry_point` - Function to run in the new thread
    /// * `stack_size` - Size of stack to allocate (0 for default)
    ///
    /// # Returns
    ///
    /// Thread ID of the newly created thread, or an error if creation fails.
    pub fn spawn<F>(&self, _entry_point: F, _stack_size: usize) -> Result<ThreadId, SpawnError>
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.is_initialized() {
            return Err(SpawnError::NotInitialized);
        }
        
        // TODO: Implement thread creation
        // - Allocate stack
        // - Set up initial context  
        // - Create thread structure
        // - Add to scheduler
        unimplemented!("Thread spawning not yet implemented")
    }
    
    /// Yield the current thread, allowing other threads to run.
    pub fn yield_now(&self) {
        if !self.is_initialized() {
            return; // Can't yield if not initialized
        }
        
        // TODO: Implement cooperative yielding
        unimplemented!("Yielding not yet implemented")
    }
    
    /// Handle a timer interrupt for preemptive scheduling.
    ///
    /// This should be called from the architecture-specific timer interrupt handler.
    ///
    /// # Safety
    ///
    /// Must be called from an interrupt context.
    pub unsafe fn handle_timer_interrupt(&self) {
        if !self.is_initialized() {
            return;
        }
        
        // TODO: Implement preemptive scheduling logic
        unimplemented!("Timer interrupt handling not yet implemented")
    }
}

/// Errors that can occur when spawning threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// Kernel has not been initialized
    NotInitialized,
    /// Out of memory for stack allocation
    OutOfMemory,
    /// Maximum number of threads reached
    TooManyThreads,
    /// Invalid stack size
    InvalidStackSize,
}

// Safety: Kernel can be shared between threads as long as the scheduler is thread-safe
unsafe impl<A: Arch, S: Scheduler> Send for Kernel<A, S> {}
unsafe impl<A: Arch, S: Scheduler> Sync for Kernel<A, S> {}
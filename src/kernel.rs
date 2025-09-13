//! Kernel abstraction for managing the threading system.
//!
//! This module provides the main `Kernel` struct that coordinates all
//! threading operations and eliminates global singleton state.

use crate::arch::Arch;
use crate::sched::{Scheduler, CpuId};
use crate::thread_new::{ThreadId, Thread, JoinHandle, ReadyRef, RunningRef};
use crate::mem::{StackPool, StackSizeClass};
use core::marker::PhantomData;
use portable_atomic::{AtomicBool, AtomicUsize, Ordering};

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
    /// Stack pool for thread allocation
    stack_pool: StackPool,
    /// Architecture marker (zero-sized)
    _arch: PhantomData<A>,
    /// Whether the kernel has been initialized
    initialized: AtomicBool,
    /// Next thread ID to assign
    next_thread_id: AtomicUsize,
    /// Currently running thread on each CPU (simplified to single CPU for now)
    current_thread: spin::Mutex<Option<RunningRef>>,
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
    pub fn new(scheduler: S) -> Self {
        Self {
            scheduler,
            stack_pool: StackPool::new(),
            _arch: PhantomData,
            initialized: AtomicBool::new(false),
            next_thread_id: AtomicUsize::new(1), // Start from 1, never use 0
            current_thread: spin::Mutex::new(None),
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
            // Initialize architecture-specific features
            unsafe {
                #[cfg(feature = "x86_64")]
                crate::arch::x86_64::init();
            }
            
            // Initialize timer subsystem for preemption
            unsafe {
                #[cfg(feature = "x86_64")]
                crate::time::x86_64_timer::init().map_err(|_| ())?;
            }
            
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
    /// * `priority` - Thread priority (0-255, higher = more important)
    ///
    /// # Returns
    ///
    /// JoinHandle for the newly created thread, or an error if creation fails.
    pub fn spawn<F>(&self, entry_point: F, priority: u8) -> Result<JoinHandle, SpawnError>
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.is_initialized() {
            return Err(SpawnError::NotInitialized);
        }
        
        // Allocate stack
        let stack = self.stack_pool.allocate(StackSizeClass::Small)
            .ok_or(SpawnError::OutOfMemory)?;
        
        // Generate unique thread ID
        let thread_id = self.next_thread_id();
        
        // Create thread entry point wrapper
        let entry_wrapper = move || {
            entry_point();
        };
        
        // For now, simplify to a basic entry point
        let simple_entry: fn() = || {};
        
        // Create thread and join handle
        let (thread, join_handle) = Thread::new(
            thread_id,
            stack,
            simple_entry,
            priority,
        );
        
        // Convert to ReadyRef and enqueue in scheduler
        let ready_ref = ReadyRef(thread);
        self.scheduler.enqueue(ready_ref);
        
        Ok(join_handle)
    }
    
    /// Yield the current thread, allowing other threads to run.
    pub fn yield_now(&self) {
        if !self.is_initialized() {
            return; // Can't yield if not initialized
        }
        
        if let Some(mut current_guard) = self.current_thread.try_lock() {
            if let Some(current) = current_guard.take() {
                // Current thread is yielding voluntarily
                self.scheduler.on_yield(current);
                
                // Try to pick next thread to run
                if let Some(next) = self.scheduler.pick_next(0) {
                    let running = next.start_running();
                    *current_guard = Some(running);
                    
                    // TODO: Perform actual context switch
                }
            }
        }
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
        
        if let Some(mut current_guard) = self.current_thread.try_lock() {
            if let Some(ref current) = *current_guard {
                // Ask scheduler if current thread should be preempted
                if let Some(ready_thread) = self.scheduler.on_tick(current) {
                    // Preempt current thread
                    if let Some(current) = current_guard.take() {
                        // Current thread was preempted, enqueue it again
                        self.scheduler.enqueue(ready_thread);
                        
                        // Try to pick next thread (could be the same one)
                        if let Some(next) = self.scheduler.pick_next(0) {
                            let running = next.start_running();
                            *current_guard = Some(running);
                            
                            // TODO: Perform actual context switch
                        }
                    }
                }
            } else {
                // No current thread, try to schedule one
                if let Some(next) = self.scheduler.pick_next(0) {
                    let running = next.start_running();
                    *current_guard = Some(running);
                    
                    // TODO: Perform actual context switch
                }
            }
        }
    }
    
    /// Get current thread statistics.
    pub fn thread_stats(&self) -> (usize, usize, usize) {
        self.scheduler.stats()
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
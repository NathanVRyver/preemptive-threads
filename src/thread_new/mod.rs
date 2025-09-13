//! New thread abstraction with RAII and memory safety.
//!
//! This module provides the new thread implementation that uses RAII
//! for resource management and eliminates manual memory management.

use crate::mem::{ArcLite, Stack};
use crate::arch::Arch;
use crate::time::{TimeSlice, Instant, Duration};
use crate::observability::metrics::GLOBAL_METRICS;
use crate::observability::resource_limits::GLOBAL_RESOURCE_LIMITER;
// PhantomData and AtomicUsize imports not needed yet
// use core::marker::PhantomData;
use portable_atomic::{AtomicU8, AtomicU64, AtomicUsize, AtomicBool, Ordering};
extern crate alloc;
use alloc::string::String;
use alloc::collections::BTreeMap;

pub mod handle;
pub mod inner;
pub mod builder;

pub use handle::JoinHandle;
pub use builder::ThreadBuilder;

static CURRENT_THREAD_ID: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(1);

/// Get current thread ID (placeholder implementation).
pub fn current_thread_id() -> ThreadId {
    let id = CURRENT_THREAD_ID.load(portable_atomic::Ordering::Relaxed);
    ThreadId::new(id)
}

/// Unique identifier for threads.
///
/// Thread IDs are never reused and are guaranteed to be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadId(core::num::NonZeroUsize);

impl core::fmt::Display for ThreadId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ThreadId {
    /// Create a new thread ID from a u64.
    pub fn new(id: u64) -> Self {
        let id_usize = id as usize;
        if id_usize == 0 {
            Self(unsafe { core::num::NonZeroUsize::new_unchecked(1) })
        } else {
            Self(unsafe { core::num::NonZeroUsize::new_unchecked(id_usize) })
        }
    }
    
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
    
    /// Get the ID as u64.
    pub fn as_u64(self) -> u64 {
        self.0.get() as u64
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
    /// Time slice tracking for scheduling
    pub time_slice: TimeSlice,
    /// Thread name for debugging
    pub name: spin::Mutex<Option<String>>,
    /// CPU affinity mask
    pub cpu_affinity: AtomicU64,
    /// Thread group ID
    pub group_id: AtomicU64,
    /// Whether this thread is critical
    pub critical: AtomicBool,
    /// Whether this thread can be preempted
    pub preemptible: AtomicBool,
    /// Reserved TLS size
    pub tls_size: AtomicUsize,
    /// Debug info enabled
    pub debug_info: AtomicBool,
    /// Real-time priority
    pub rt_priority: AtomicU8,
    /// Nice value
    pub nice_value: portable_atomic::AtomicI8,
    /// Inherit signal mask
    pub inherit_signal_mask: AtomicBool,
    /// Environment variables
    pub environment: spin::Mutex<Option<BTreeMap<String, String>>>,
    /// Resource limits
    pub max_cpu_time: AtomicU64,
    pub max_memory: AtomicUsize,
    pub max_files: AtomicU64,
    pub max_children: AtomicU64,
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
            time_slice: TimeSlice::new(priority),
            name: spin::Mutex::new(None),
            cpu_affinity: AtomicU64::new(0), // 0 means no affinity
            group_id: AtomicU64::new(0),
            critical: AtomicBool::new(false),
            preemptible: AtomicBool::new(true),
            tls_size: AtomicUsize::new(0),
            debug_info: AtomicBool::new(cfg!(debug_assertions)),
            rt_priority: AtomicU8::new(0),
            nice_value: portable_atomic::AtomicI8::new(0),
            inherit_signal_mask: AtomicBool::new(true),
            environment: spin::Mutex::new(None),
            max_cpu_time: AtomicU64::new(0), // 0 means no limit
            max_memory: AtomicUsize::new(0),
            max_files: AtomicU64::new(0),
            max_children: AtomicU64::new(0),
        };
        
        let inner_arc = ArcLite::new(inner);
        
        let thread = Self {
            inner: inner_arc.clone(),
        };
        
        let join_handle = JoinHandle {
            inner: inner_arc,
        };
        
        // Register thread with observability systems
        GLOBAL_METRICS.register_thread(id);
        GLOBAL_RESOURCE_LIMITER.register_thread(id);
        
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
        self.inner.time_slice.set_priority(new_priority);
        
        // Record priority change in metrics
        if GLOBAL_METRICS.is_enabled() {
            // Priority changes are tracked automatically in thread metrics
        }
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
    
    /// Start a new time slice for this thread.
    ///
    /// This should be called when the thread is scheduled to run.
    pub fn start_time_slice(&self) {
        let current_time = Instant::now();
        self.inner.time_slice.start_slice(current_time);
        
        // Record context switch in metrics (this is when we switch TO this thread)
        GLOBAL_METRICS.record_context_switch(self.id(), true); // Assume voluntary for now
    }
    
    /// Update the thread's virtual runtime and check if preemption is needed.
    ///
    /// # Returns
    ///
    /// `true` if the thread's time slice has expired and it should be preempted.
    pub fn should_preempt(&self) -> bool {
        let current_time = Instant::now();
        let should_preempt = self.inner.time_slice.update_vruntime(current_time);
        
        if should_preempt {
            // Record involuntary preemption
            GLOBAL_METRICS.record_context_switch(self.id(), false);
        }
        
        should_preempt
    }
    
    /// Get the thread's current virtual runtime.
    ///
    /// This is used by the scheduler for fair scheduling decisions.
    pub fn vruntime(&self) -> u64 {
        self.inner.time_slice.vruntime()
    }
    
    /// Set the thread name for debugging purposes.
    pub fn set_name(&self, name: String) {
        if let Some(mut thread_name) = self.inner.name.try_lock() {
            *thread_name = Some(name);
        }
    }
    
    /// Get the thread name.
    pub fn name(&self) -> Option<String> {
        self.inner.name.try_lock().and_then(|name| name.clone())
    }
    
    /// Set CPU affinity mask.
    pub fn set_cpu_affinity(&self, affinity: u64) {
        self.inner.cpu_affinity.store(affinity, Ordering::Release);
    }
    
    /// Get CPU affinity mask.
    pub fn cpu_affinity(&self) -> u64 {
        self.inner.cpu_affinity.load(Ordering::Acquire)
    }
    
    /// Set thread group ID.
    pub fn set_group_id(&self, group_id: u32) {
        self.inner.group_id.store(group_id as u64, Ordering::Release);
    }
    
    /// Get thread group ID.
    pub fn group_id(&self) -> u32 {
        self.inner.group_id.load(Ordering::Acquire) as u32
    }
    
    /// Set custom time slice duration.
    pub fn set_time_slice(&self, duration: Duration) {
        self.inner.time_slice.set_custom_duration(duration);
    }
    
    /// Set whether this thread is critical.
    pub fn set_critical(&self, critical: bool) {
        self.inner.critical.store(critical, Ordering::Release);
    }
    
    /// Check if this thread is critical.
    pub fn is_critical(&self) -> bool {
        self.inner.critical.load(Ordering::Acquire)
    }
    
    /// Set whether this thread can be preempted.
    pub fn set_preemptible(&self, preemptible: bool) {
        self.inner.preemptible.store(preemptible, Ordering::Release);
    }
    
    /// Check if this thread can be preempted.
    pub fn is_preemptible(&self) -> bool {
        self.inner.preemptible.load(Ordering::Acquire)
    }
    
    /// Reserve thread-local storage space.
    pub fn reserve_tls(&self, size: usize) {
        self.inner.tls_size.store(size, Ordering::Release);
    }
    
    /// Get reserved TLS size.
    pub fn tls_size(&self) -> usize {
        self.inner.tls_size.load(Ordering::Acquire)
    }
    
    /// Enable or disable debug information.
    pub fn set_debug_info(&self, enabled: bool) {
        self.inner.debug_info.store(enabled, Ordering::Release);
    }
    
    /// Check if debug information is enabled.
    pub fn debug_info_enabled(&self) -> bool {
        self.inner.debug_info.load(Ordering::Acquire)
    }
    
    /// Set real-time priority.
    pub fn set_realtime_priority(&self, rt_priority: u8) {
        self.inner.rt_priority.store(rt_priority, Ordering::Release);
    }
    
    /// Get real-time priority.
    pub fn realtime_priority(&self) -> u8 {
        self.inner.rt_priority.load(Ordering::Acquire)
    }
    
    /// Set nice value for process priority.
    pub fn set_nice_value(&self, nice: i8) {
        self.inner.nice_value.store(nice, Ordering::Release);
    }
    
    /// Get nice value.
    pub fn nice_value(&self) -> i8 {
        self.inner.nice_value.load(Ordering::Acquire)
    }
    
    /// Set whether to inherit parent's signal mask.
    pub fn set_inherit_signal_mask(&self, inherit: bool) {
        self.inner.inherit_signal_mask.store(inherit, Ordering::Release);
    }
    
    /// Check if inheriting parent's signal mask.
    pub fn inherits_signal_mask(&self) -> bool {
        self.inner.inherit_signal_mask.load(Ordering::Acquire)
    }
    
    /// Set custom environment variables.
    pub fn set_environment(&self, env: BTreeMap<String, String>) {
        if let Some(mut environment) = self.inner.environment.try_lock() {
            *environment = Some(env);
        }
    }
    
    /// Get environment variables.
    pub fn environment(&self) -> Option<BTreeMap<String, String>> {
        self.inner.environment.try_lock().and_then(|env| env.clone())
    }
    
    /// Set maximum CPU time limit.
    pub fn set_max_cpu_time(&self, max_time: u64) {
        self.inner.max_cpu_time.store(max_time, Ordering::Release);
    }
    
    /// Get maximum CPU time limit.
    pub fn max_cpu_time(&self) -> u64 {
        self.inner.max_cpu_time.load(Ordering::Acquire)
    }
    
    /// Set maximum memory usage limit.
    pub fn set_max_memory(&self, max_memory: usize) {
        self.inner.max_memory.store(max_memory, Ordering::Release);
    }
    
    /// Get maximum memory usage limit.
    pub fn max_memory(&self) -> usize {
        self.inner.max_memory.load(Ordering::Acquire)
    }
    
    /// Set maximum file descriptors limit.
    pub fn set_max_files(&self, max_files: u32) {
        self.inner.max_files.store(max_files as u64, Ordering::Release);
    }
    
    /// Get maximum file descriptors limit.
    pub fn max_files(&self) -> u32 {
        self.inner.max_files.load(Ordering::Acquire) as u32
    }
    
    /// Set maximum child threads limit.
    pub fn set_max_children(&self, max_children: u32) {
        self.inner.max_children.store(max_children as u64, Ordering::Release);
    }
    
    /// Get maximum child threads limit.
    pub fn max_children(&self) -> u32 {
        self.inner.max_children.load(Ordering::Acquire) as u32
    }
    
    /// Record memory allocation for this thread.
    pub fn record_memory_allocation(&self, size: u64) {
        use crate::observability::resource_limits::{ResourceType, GLOBAL_RESOURCE_LIMITER};
        
        // Check resource limits
        if let Err(_) = GLOBAL_RESOURCE_LIMITER.check_resource_limit(
            self.id(),
            ResourceType::Memory,
            size,
        ) {
            // Handle limit violation - could log or take action
        }
        
        // Update resource usage
        GLOBAL_RESOURCE_LIMITER.update_resource_usage(
            self.id(),
            ResourceType::Memory,
            size,
        );
    }
    
    /// Record memory deallocation for this thread.
    pub fn record_memory_deallocation(&self, size: u64) {
        use crate::observability::resource_limits::{ResourceType, GLOBAL_RESOURCE_LIMITER};
        
        // Update resource usage (subtract from current usage)
        let current_usage = GLOBAL_RESOURCE_LIMITER.get_system_usage()
            .total_memory_usage.load(Ordering::Acquire);
        
        let new_usage = if current_usage >= size {
            current_usage - size
        } else {
            0
        };
        
        GLOBAL_RESOURCE_LIMITER.update_resource_usage(
            self.id(),
            ResourceType::Memory,
            new_usage,
        );
    }
    
    /// Record CPU time usage for this thread.
    pub fn record_cpu_time(&self, duration: Duration, user_mode: bool) {
        // Record in metrics
        GLOBAL_METRICS.record_cpu_time(self.id(), duration, user_mode);
        
        // Update resource limits
        use crate::observability::resource_limits::{ResourceType, GLOBAL_RESOURCE_LIMITER};
        GLOBAL_RESOURCE_LIMITER.update_resource_usage(
            self.id(),
            ResourceType::CpuTime,
            duration.as_nanos(),
        );
    }
    
    /// Update stack usage for this thread.
    pub fn update_stack_usage_metrics(&self, current_usage: usize) {
        GLOBAL_METRICS.update_stack_usage(self.id(), current_usage);
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

impl Drop for ThreadInner {
    fn drop(&mut self) {
        // Unregister thread from observability systems
        GLOBAL_METRICS.unregister_thread(self.id);
        GLOBAL_RESOURCE_LIMITER.unregister_thread(self.id);
    }
}

/// A reference to a thread that is currently ready to run.
///
/// This type represents a thread that is in the scheduler's ready queue
/// and can be selected to run on a CPU.
#[derive(Clone)]
pub struct ReadyRef(pub Thread);

/// A reference to a thread that is currently running on a CPU.
///
/// This type represents a thread that is actively executing on a CPU.
#[derive(Clone)]
pub struct RunningRef(pub Thread);

impl ReadyRef {
    /// Convert this ready reference to a running reference.
    ///
    /// This should be called when the scheduler selects this thread to run.
    pub fn start_running(self) -> RunningRef {
        self.0.set_state(ThreadState::Running);
        self.0.start_time_slice();
        RunningRef(self.0)
    }
    
    /// Get the thread's priority.
    pub fn priority(&self) -> u8 {
        self.0.priority()
    }
    
    /// Get the thread's unique identifier.
    pub fn id(&self) -> ThreadId {
        self.0.id()
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
    
    /// Check if this thread should be preempted.
    ///
    /// This updates the thread's virtual runtime and returns true if
    /// the time slice has expired.
    pub fn should_preempt(&self) -> bool {
        self.0.should_preempt()
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
    
    /// Prepare this thread for preemption.
    ///
    /// This saves the current state and returns a ReadyRef that can be re-enqueued.
    pub fn prepare_preemption(&self) -> ReadyRef {
        let ready = ReadyRef(self.0.clone());
        ready.0.set_state(ThreadState::Ready);
        ready
    }
    
    /// Get the thread's priority.
    pub fn priority(&self) -> u8 {
        self.0.priority()
    }
    
    /// Get the thread's unique identifier.
    pub fn id(&self) -> ThreadId {
        self.0.id()
    }
    
    /// Get the CPU this thread last ran on.
    ///
    /// For now, return 0 as a placeholder. In a real implementation,
    /// this would track the actual CPU assignment.
    pub fn last_cpu(&self) -> usize {
        0 // TODO: Track actual CPU assignment
    }
    
    /// Get access to the thread's time slice for scheduler decisions.
    pub fn time_slice(&self) -> &TimeSlice {
        &self.0.inner.time_slice
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{StackPool, StackSizeClass};
    
    #[cfg(feature = "std-shim")]
    #[test]
    fn test_thread_creation() {
        use std::println;
        let pool = StackPool::new();
        let stack = pool.allocate(StackSizeClass::Small).unwrap();
        let thread_id = unsafe { ThreadId::new_unchecked(1) };
        
        let (thread, _join_handle) = Thread::new(
            thread_id,
            stack,
            || { println!("Hello from thread!"); },
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
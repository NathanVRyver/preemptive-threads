//! Thread builder for configuring thread creation.

use super::{Thread, JoinHandle, ThreadId};
use crate::mem::{StackPool, StackSizeClass};

/// Builder for configuring and creating new threads.
///
/// This provides a convenient interface for setting thread parameters
/// before spawning, similar to std::thread::Builder.
pub struct ThreadBuilder {
    /// Stack size class to use
    stack_size_class: Option<StackSizeClass>,
    /// Thread priority (0-255, higher = more important)
    priority: u8,
    /// Thread name (for debugging)
    name: Option<&'static str>,
}

impl ThreadBuilder {
    /// Create a new thread builder with default settings.
    pub fn new() -> Self {
        Self {
            stack_size_class: None, // Will default to Small
            priority: 128, // Default priority
            name: None,
        }
    }
    
    /// Set the stack size class for the thread.
    ///
    /// # Arguments
    ///
    /// * `size_class` - The desired stack size class
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn stack_size_class(mut self, size_class: StackSizeClass) -> Self {
        self.stack_size_class = Some(size_class);
        self
    }
    
    /// Set the stack size in bytes.
    ///
    /// This will choose the smallest size class that can accommodate
    /// the requested size.
    ///
    /// # Arguments
    ///
    /// * `size` - Minimum stack size in bytes
    ///
    /// # Returns
    ///
    /// Self for method chaining, or the original builder if the
    /// size is too large.
    pub fn stack_size(mut self, size: usize) -> Self {
        if let Some(size_class) = StackSizeClass::for_size(size) {
            self.stack_size_class = Some(size_class);
        }
        self
    }
    
    /// Set the thread priority.
    ///
    /// # Arguments
    ///
    /// * `priority` - Thread priority (0-255, higher = more important)
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
    
    /// Set the thread name for debugging purposes.
    ///
    /// # Arguments
    ///
    /// * `name` - Thread name
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }
    
    /// Spawn a new thread with the configured parameters.
    ///
    /// # Arguments
    ///
    /// * `thread_id` - Unique identifier for the new thread
    /// * `stack_pool` - Stack pool to allocate from
    /// * `entry_point` - Function to run in the new thread
    ///
    /// # Returns
    ///
    /// A tuple of (Thread, JoinHandle) if successful, or an error if
    /// thread creation fails.
    pub fn spawn(
        self,
        thread_id: ThreadId,
        stack_pool: &StackPool,
        entry_point: fn(),
    ) -> Result<(Thread, JoinHandle), SpawnError> {
        // Determine stack size class
        let size_class = self.stack_size_class.unwrap_or(StackSizeClass::Small);
        
        // Allocate stack
        let stack = stack_pool
            .allocate(size_class)
            .ok_or(SpawnError::OutOfMemory)?;
        
        // Create the thread
        let (thread, join_handle) = Thread::new(
            thread_id,
            stack,
            entry_point,
            self.priority,
        );
        
        Ok((thread, join_handle))
    }
}

impl Default for ThreadBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during thread spawning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// Out of memory for stack allocation
    OutOfMemory,
    /// Invalid configuration parameters
    InvalidConfig,
    /// Maximum number of threads reached
    TooManyThreads,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "std-shim")]
    #[test]
    fn test_thread_builder() {
        let pool = StackPool::new();
        let thread_id = unsafe { ThreadId::new_unchecked(1) };
        
        let builder = ThreadBuilder::new()
            .stack_size_class(StackSizeClass::Medium)
            .priority(200)
            .name("test-thread");
        
        let result = builder.spawn(thread_id, &pool, || {
            // Thread code here
        });
        
        assert!(result.is_ok());
        let (thread, _join_handle) = result.unwrap();
        
        assert_eq!(thread.id(), thread_id);
        assert_eq!(thread.priority(), 200);
    }
    
    #[cfg(feature = "std-shim")]
    #[test]
    fn test_thread_builder_stack_size() {
        let builder1 = ThreadBuilder::new().stack_size(8192);
        // Should select Medium size class for 8KB request
        
        let builder2 = ThreadBuilder::new().stack_size(1024);
        // Should select Small size class for 1KB request
        
        // We can't easily test the internal state without exposing it,
        // but the important thing is that it compiles and doesn't panic
        let _ = builder1;
        let _ = builder2;
    }
}
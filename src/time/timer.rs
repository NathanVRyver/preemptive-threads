//! Timer configuration and interrupt management.

use super::{Duration, Instant};
use crate::arch::Arch;

/// Timer configuration for preemptive scheduling.
#[derive(Debug, Clone)]
pub struct TimerConfig {
    /// Timer frequency in Hz (interrupts per second)
    pub frequency: u32,
    /// Timer interrupt vector number
    pub vector: u8,
    /// Whether high-precision timing is enabled
    pub high_precision: bool,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            frequency: super::TIMER_FREQUENCY_HZ,
            vector: 0xEF, // Default timer vector
            high_precision: true,
        }
    }
}

/// Timer abstraction for different hardware timers.
pub trait Timer {
    /// Initialize the timer with the given configuration.
    ///
    /// # Safety
    ///
    /// This function configures hardware timers and interrupt vectors.
    /// It must only be called once during system initialization with
    /// interrupts disabled.
    unsafe fn init(&mut self, config: &TimerConfig) -> Result<(), TimerError>;
    
    /// Start the timer.
    ///
    /// The timer will begin generating interrupts at the configured frequency.
    fn start(&mut self) -> Result<(), TimerError>;
    
    /// Stop the timer.
    ///
    /// This stops interrupt generation but preserves timer configuration.
    fn stop(&mut self) -> Result<(), TimerError>;
    
    /// Get the current timer count.
    ///
    /// This provides access to the hardware timer's current count value
    /// for high-precision timing measurements.
    fn current_count(&self) -> u64;
    
    /// Convert timer counts to nanoseconds.
    fn counts_to_nanos(&self, counts: u64) -> u64;
    
    /// Convert nanoseconds to timer counts.
    fn nanos_to_counts(&self, nanos: u64) -> u64;
    
    /// Set up a one-shot timer for the given duration.
    ///
    /// This is used for implementing sleep and timeout functionality.
    fn set_oneshot(&mut self, duration: Duration) -> Result<(), TimerError>;
}

/// Errors that can occur during timer operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
    /// Timer is not initialized
    NotInitialized,
    /// Invalid configuration parameters
    InvalidConfig,
    /// Hardware timer is not available
    NotAvailable,
    /// Timer is already running
    AlreadyRunning,
    /// Timer is not running
    NotRunning,
    /// Frequency is out of supported range
    UnsupportedFrequency,
}

/// Preemption guard for disabling preemption without disabling interrupts.
///
/// This allows critical sections that need to prevent preemption but still
/// allow interrupt handling (e.g., for device drivers).
pub struct PreemptGuard {
    /// Previous preemption state
    was_enabled: bool,
}

impl PreemptGuard {
    /// Enter a preemption-disabled critical section.
    ///
    /// # Returns
    ///
    /// A guard that will re-enable preemption when dropped.
    pub fn enter() -> Self {
        let was_enabled = disable_preemption();
        Self { was_enabled }
    }
    
    /// Check if preemption is currently disabled.
    pub fn is_disabled() -> bool {
        !is_preemption_enabled()
    }
}

impl Drop for PreemptGuard {
    fn drop(&mut self) {
        if self.was_enabled {
            enable_preemption();
        }
    }
}

/// Interrupt guard for disabling all interrupts.
///
/// This provides a critical section where no interrupts can occur,
/// used for the most critical kernel operations.
pub struct IrqGuard {
    /// Previous interrupt state  
    was_enabled: bool,
}

impl IrqGuard {
    /// Enter an interrupt-disabled critical section.
    ///
    /// # Returns
    ///
    /// A guard that will restore interrupt state when dropped.
    pub fn enter() -> Self {
        let was_enabled = crate::arch::DefaultArch::interrupts_enabled();
        crate::arch::DefaultArch::disable_interrupts();
        Self { was_enabled }
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        if self.was_enabled {
            crate::arch::DefaultArch::enable_interrupts();
        }
    }
}

/// Per-CPU preemption state.
///
/// This tracks whether preemption is enabled for the current CPU.
/// We use thread-local storage or per-CPU data structures for this.
static mut PREEMPTION_ENABLED: bool = true;

/// Disable preemption on the current CPU.
///
/// # Returns
///
/// Previous preemption state.
fn disable_preemption() -> bool {
    // TODO: This should be per-CPU and atomic
    // For now, use a simple global flag
    unsafe {
        let was_enabled = PREEMPTION_ENABLED;
        PREEMPTION_ENABLED = false;
        was_enabled
    }
}

/// Enable preemption on the current CPU.
fn enable_preemption() {
    unsafe {
        PREEMPTION_ENABLED = true;
    }
}

/// Check if preemption is enabled on the current CPU.
fn is_preemption_enabled() -> bool {
    unsafe { PREEMPTION_ENABLED }
}

/// Handle a timer interrupt for preemptive scheduling.
///
/// This function should be called from the architecture-specific
/// timer interrupt handler. It updates tick counters and triggers
/// scheduling decisions.
///
/// # Safety
///
/// Must be called from an interrupt context with a valid interrupt frame.
/// The caller must ensure that the interrupt frame is properly saved
/// and that this function doesn't corrupt the interrupted context.
pub unsafe fn handle_timer_interrupt() {
    // Increment global tick counter
    super::tick::GLOBAL_TICK_COUNTER.increment();
    
    // Only preempt if preemption is enabled
    if !is_preemption_enabled() {
        return;
    }
    
    // TODO: Get current kernel handle and trigger scheduling
    // This would call kernel.handle_timer_interrupt()
    
    // For now, just a placeholder
    schedule_if_needed();
}

/// Check if scheduling is needed and trigger it.
///
/// This examines the current thread's time slice and the ready queue
/// to determine if a context switch should occur.
fn schedule_if_needed() {
    // TODO: Implement scheduling logic
    // 1. Get current thread
    // 2. Update its time slice
    // 3. Check if quantum expired or higher priority thread available
    // 4. Trigger context switch if needed
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timer_config() {
        let config = TimerConfig::default();
        assert_eq!(config.frequency, super::super::TIMER_FREQUENCY_HZ);
        assert_eq!(config.vector, 0xEF);
        assert!(config.high_precision);
    }
    
    #[test]
    fn test_preempt_guard() {
        // Initially preemption should be enabled
        assert!(!PreemptGuard::is_disabled());
        
        {
            let _guard = PreemptGuard::enter();
            assert!(PreemptGuard::is_disabled());
        } // Guard dropped here
        
        // Should be re-enabled after guard drop
        assert!(!PreemptGuard::is_disabled());
    }
    
    #[test]
    fn test_timer_error_types() {
        let error = TimerError::NotInitialized;
        assert_eq!(error, TimerError::NotInitialized);
        
        let error2 = TimerError::InvalidConfig;
        assert_ne!(error, error2);
    }
}
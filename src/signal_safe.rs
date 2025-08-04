/// Signal-safe preemption handler
///
/// This module provides backward compatibility with the old signal-based API.
/// For new code, prefer using `crate::platform_timer` directly.
// Re-export the platform timer functions for backward compatibility
pub use crate::platform_timer::{
    clear_preemption_pending, get_preemption_count, is_preemption_pending, preemption_checkpoint,
};

#[cfg(target_os = "linux")]
pub use crate::platform_timer::signal_safe_handler;

/// Initialize signal-safe preemption (backward compatibility)
pub fn init_signal_safe_preemption(interval_ms: u64) -> Result<(), &'static str> {
    crate::platform_timer::init_preemption_timer(interval_ms)
}

/// Stop preemption timer (backward compatibility)  
pub fn stop_preemption() {
    crate::platform_timer::stop_preemption_timer();
}

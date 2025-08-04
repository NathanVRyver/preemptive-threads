use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Signal-safe preemption handler
///
/// This module implements a signal handler that only uses async-signal-safe operations:
/// - No memory allocation
/// - No mutex/lock operations
/// - Only atomic operations and simple assignments
/// - No complex scheduling logic in signal context
static PREEMPTION_PENDING: AtomicBool = AtomicBool::new(false);
static PREEMPTION_COUNT: AtomicU64 = AtomicU64::new(0);

/// Signal handler that just sets a flag - actual scheduling happens outside signal context
#[cfg(target_os = "linux")]
pub unsafe extern "C" fn signal_safe_handler(_sig: i32) {
    // Only use async-signal-safe operations here
    PREEMPTION_PENDING.store(true, Ordering::Release);
    PREEMPTION_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Check if preemption is pending (called from normal context)
pub fn is_preemption_pending() -> bool {
    PREEMPTION_PENDING.load(Ordering::Acquire)
}

/// Clear preemption pending flag
pub fn clear_preemption_pending() {
    PREEMPTION_PENDING.store(false, Ordering::Release);
}

/// Get total preemption count for statistics
pub fn get_preemption_count() -> u64 {
    PREEMPTION_COUNT.load(Ordering::Relaxed)
}

/// Preemption checkpoint - should be called regularly from normal code
/// This is where actual scheduling decisions are made, outside signal context
pub fn preemption_checkpoint() {
    if is_preemption_pending() {
        clear_preemption_pending();

        // Safe to do complex operations here - we're not in signal context
        // Yield to scheduler - the scheduler is a global static
        crate::sync::yield_thread();
    }
}

/// Initialize signal-safe preemption
#[cfg(target_os = "linux")]
pub fn init_signal_safe_preemption(interval_ms: u64) -> Result<(), &'static str> {
    use core::mem::MaybeUninit;

    unsafe {
        // Block SIGALRM during setup
        let mut set = MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        libc::sigaddset(set.as_mut_ptr(), libc::SIGALRM);

        // Install signal handler with SA_RESTART to avoid EINTR
        let mut sa = MaybeUninit::<libc::sigaction>::uninit();
        libc::sigemptyset(&mut (*sa.as_mut_ptr()).sa_mask);
        (*sa.as_mut_ptr()).sa_flags = libc::SA_RESTART;
        (*sa.as_mut_ptr()).sa_sigaction = signal_safe_handler as usize;

        if libc::sigaction(libc::SIGALRM, sa.as_ptr(), core::ptr::null_mut()) == -1 {
            return Err("Failed to install signal handler");
        }

        // Set up interval timer
        let timer = libc::itimerval {
            it_interval: libc::timeval {
                tv_sec: (interval_ms / 1000) as libc::time_t,
                tv_usec: ((interval_ms % 1000) * 1000) as libc::suseconds_t,
            },
            it_value: libc::timeval {
                tv_sec: (interval_ms / 1000) as libc::time_t,
                tv_usec: ((interval_ms % 1000) * 1000) as libc::suseconds_t,
            },
        };

        if libc::setitimer(libc::ITIMER_REAL, &timer, core::ptr::null_mut()) == -1 {
            return Err("Failed to set interval timer");
        }

        // Unblock SIGALRM
        libc::sigprocmask(libc::SIG_UNBLOCK, set.as_ptr(), core::ptr::null_mut());
    }

    Ok(())
}

/// Stop preemption timer
#[cfg(target_os = "linux")]
pub fn stop_preemption() {
    unsafe {
        let timer = libc::itimerval {
            it_interval: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            it_value: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        };
        libc::setitimer(libc::ITIMER_REAL, &timer, core::ptr::null_mut());
    }
}

/// Cooperative preemption points - insert these in long-running code
#[macro_export]
macro_rules! preemption_point {
    () => {
        $crate::signal_safe::preemption_checkpoint();
    };
}

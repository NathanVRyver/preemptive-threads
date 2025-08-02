#[cfg(target_os = "linux")]
pub struct Preemption {
    enabled: bool,
}

#[cfg(target_os = "linux")]
impl Default for Preemption {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl Preemption {
    pub const fn new() -> Self {
        Preemption { enabled: false }
    }

    /// # Safety
    /// This function sets up signal handlers and timer interrupts which can affect
    /// the entire process. Only one instance should manage preemption at a time.
    /// The caller must ensure thread safety when accessing the global scheduler.
    pub unsafe fn enable(&mut self, interval_us: u64) {
        extern "C" {
            fn signal(sig: i32, handler: extern "C" fn(i32)) -> i32;
            fn setitimer(which: i32, new_value: *const itimerval, old_value: *mut itimerval)
                -> i32;
        }

        const SIGALRM: i32 = 14;
        const ITIMER_REAL: i32 = 0;

        #[repr(C)]
        struct timeval {
            tv_sec: i64,
            tv_usec: i64,
        }

        #[repr(C)]
        struct itimerval {
            it_interval: timeval,
            it_value: timeval,
        }

        signal(SIGALRM, timer_handler);

        let timer = itimerval {
            it_interval: timeval {
                tv_sec: (interval_us / 1_000_000) as i64,
                tv_usec: (interval_us % 1_000_000) as i64,
            },
            it_value: timeval {
                tv_sec: (interval_us / 1_000_000) as i64,
                tv_usec: (interval_us % 1_000_000) as i64,
            },
        };

        setitimer(ITIMER_REAL, &timer, core::ptr::null_mut());
        self.enabled = true;
    }

    /// # Safety
    /// This function modifies process-wide timer settings. The caller must ensure
    /// no other code depends on SIGALRM or ITIMER_REAL functionality.
    pub unsafe fn disable(&mut self) {
        if !self.enabled {
            return;
        }

        extern "C" {
            fn setitimer(which: i32, new_value: *const itimerval, old_value: *mut itimerval)
                -> i32;
        }

        const ITIMER_REAL: i32 = 0;

        #[repr(C)]
        struct timeval {
            tv_sec: i64,
            tv_usec: i64,
        }

        #[repr(C)]
        struct itimerval {
            it_interval: timeval,
            it_value: timeval,
        }

        let timer = itimerval {
            it_interval: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            it_value: timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        };

        setitimer(ITIMER_REAL, &timer, core::ptr::null_mut());
        self.enabled = false;
    }
}

#[cfg(target_os = "linux")]
extern "C" fn timer_handler(_sig: i32) {
    unsafe {
        let scheduler = crate::scheduler::SCHEDULER.get();

        if let Some(current_id) = scheduler.get_current_thread() {
            if let Some(next_id) = scheduler.schedule() {
                if current_id != next_id {
                    scheduler.set_current_thread(Some(next_id));
                    let _ = scheduler.switch_context(current_id, next_id);
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct Preemption;

#[cfg(not(target_os = "linux"))]
impl Default for Preemption {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "linux"))]
impl Preemption {
    pub const fn new() -> Self {
        Preemption
    }

    /// # Safety
    /// Enables timer-based preemption. May affect signal handlers.
    pub unsafe fn enable(&mut self, _interval_us: u64) {}
    
    /// # Safety
    /// Disables timer-based preemption. May affect signal handlers.
    pub unsafe fn disable(&mut self) {}
}

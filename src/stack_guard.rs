use core::sync::atomic::{AtomicU64, Ordering};

/// Stack guard configuration
pub struct StackGuard {
    /// Guard pages at bottom of stack (in bytes)
    pub guard_size: usize,
    /// Canary values for overflow detection
    pub canary_value: u64,
    /// Red zone size (bytes below stack pointer that must not be used)
    pub red_zone: usize,
}

impl Default for StackGuard {
    fn default() -> Self {
        Self {
            guard_size: 4096, // 1 page
            canary_value: 0xDEADBEEFCAFEBABE,
            red_zone: 128, // x86_64 ABI red zone
        }
    }
}

/// Enhanced stack with multiple protection mechanisms
pub struct ProtectedStack {
    /// Base address of allocated memory
    base: *mut u8,
    /// Total size including guards
    total_size: usize,
    /// Usable stack size
    stack_size: usize,
    /// Guard configuration
    guard: StackGuard,
    /// Stack watermark for high water mark tracking
    watermark: AtomicU64,
}

impl ProtectedStack {
    /// Create a new protected stack
    ///
    /// # Safety
    /// Caller must provide a valid memory region
    pub unsafe fn new(memory: &'static mut [u8], guard: StackGuard) -> Self {
        let base = memory.as_mut_ptr();
        let total_size = memory.len();

        // Ensure we have enough space for guards
        assert!(
            total_size > guard.guard_size * 2 + 4096,
            "Stack too small for guards"
        );

        let stack_size = total_size - guard.guard_size * 2;

        // Initialize guard pages with canary pattern
        let guard_start = base;
        let _guard_end = unsafe { base.add(guard.guard_size) };

        // Fill bottom guard with canary values
        let canary_ptr = guard_start as *mut u64;
        for i in 0..(guard.guard_size / 8) {
            unsafe {
                canary_ptr.add(i).write_volatile(guard.canary_value);
            }
        }

        // Initialize watermark to stack top
        let stack_top = unsafe { base.add(total_size - guard.guard_size) as u64 };

        Self {
            base,
            total_size,
            stack_size,
            guard,
            watermark: AtomicU64::new(stack_top),
        }
    }

    /// Get usable stack memory
    pub fn get_stack(&self) -> &'static mut [u8] {
        unsafe {
            let stack_start = self.base.add(self.guard.guard_size);
            core::slice::from_raw_parts_mut(stack_start, self.stack_size)
        }
    }

    /// Check for stack overflow using multiple methods
    pub fn check_overflow(&self) -> StackStatus {
        unsafe {
            // Method 1: Check canary values
            let canary_start = self.base as *const u64;
            let canary_count = self.guard.guard_size / 8;

            let mut corrupted_canaries = 0;
            for i in 0..canary_count {
                if canary_start.add(i).read_volatile() != self.guard.canary_value {
                    corrupted_canaries += 1;
                }
            }

            if corrupted_canaries > 0 {
                return StackStatus::Corrupted {
                    corrupted_bytes: corrupted_canaries * 8,
                    location: StackCorruption::GuardPage,
                };
            }

            // Method 2: Check current stack pointer
            let current_sp = get_stack_pointer();
            let stack_bottom = self.base.add(self.guard.guard_size) as u64;

            if current_sp < stack_bottom {
                return StackStatus::Overflow {
                    overflow_bytes: (stack_bottom - current_sp) as usize,
                };
            }

            // Method 3: Update and check watermark
            self.watermark.fetch_min(current_sp, Ordering::Relaxed);
            let high_water_mark = self.watermark.load(Ordering::Relaxed);
            let used =
                (self.base.add(self.total_size - self.guard.guard_size) as u64) - high_water_mark;

            if used as usize > self.stack_size - self.guard.red_zone {
                return StackStatus::NearOverflow {
                    bytes_remaining: self.stack_size - used as usize,
                };
            }

            StackStatus::Ok {
                used_bytes: used as usize,
                free_bytes: self.stack_size - used as usize,
            }
        }
    }

    /// Get detailed stack statistics
    pub fn get_stats(&self) -> StackStats {
        let current_sp = get_stack_pointer();
        let stack_top = unsafe { self.base.add(self.total_size - self.guard.guard_size) as u64 };
        let high_water_mark = self.watermark.load(Ordering::Relaxed);

        StackStats {
            total_size: self.total_size,
            usable_size: self.stack_size,
            guard_size: self.guard.guard_size,
            current_usage: (stack_top - current_sp) as usize,
            peak_usage: (stack_top - high_water_mark) as usize,
            red_zone_size: self.guard.red_zone,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StackStatus {
    Ok {
        used_bytes: usize,
        free_bytes: usize,
    },
    NearOverflow {
        bytes_remaining: usize,
    },
    Overflow {
        overflow_bytes: usize,
    },
    Corrupted {
        corrupted_bytes: usize,
        location: StackCorruption,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum StackCorruption {
    GuardPage,
    StackFrame,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct StackStats {
    pub total_size: usize,
    pub usable_size: usize,
    pub guard_size: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub red_zone_size: usize,
}

/// Get current stack pointer
#[inline(always)]
fn get_stack_pointer() -> u64 {
    let sp: u64;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) sp);
    }
    sp
}

/// Stack allocation with automatic guard setup
#[macro_export]
macro_rules! protected_stack {
    ($size:expr) => {{
        static mut STACK_MEMORY: [u8; $size] = [0; $size];
        unsafe {
            $crate::stack_guard::ProtectedStack::new(
                &mut STACK_MEMORY,
                $crate::stack_guard::StackGuard::default(),
            )
        }
    }};
    ($size:expr, $guard_size:expr) => {{
        static mut STACK_MEMORY: [u8; $size] = [0; $size];
        unsafe {
            let guard = $crate::stack_guard::StackGuard {
                guard_size: $guard_size,
                ..$crate::stack_guard::StackGuard::default()
            };
            $crate::stack_guard::ProtectedStack::new(&mut STACK_MEMORY, guard)
        }
    }};
}

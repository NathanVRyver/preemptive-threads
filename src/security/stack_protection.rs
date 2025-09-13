//! Stack overflow protection with canaries and guard pages.

use crate::errors::ThreadError;
use crate::security::{SecurityConfig, SecurityViolation, SECURITY_STATE, handle_security_violation};
use crate::mem::Stack;
use portable_atomic::{AtomicUsize, Ordering};
use alloc::alloc;

/// Stack canary magic value for overflow detection.
const STACK_CANARY_MAGIC: u64 = 0xDEADBEEFCAFEBABE;

/// Stack protection implementation.
pub struct StackProtection {
    /// Number of canaries placed
    canaries_placed: AtomicUsize,
    /// Number of violations detected
    violations_detected: AtomicUsize,
    /// Guard pages allocated
    guard_pages_allocated: AtomicUsize,
}

impl StackProtection {
    pub const fn new() -> Self {
        Self {
            canaries_placed: AtomicUsize::new(0),
            violations_detected: AtomicUsize::new(0),
            guard_pages_allocated: AtomicUsize::new(0),
        }
    }
}

/// Global stack protection instance.
static STACK_PROTECTION: StackProtection = StackProtection::new();

/// Stack canary for overflow detection.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StackCanary {
    magic: u64,
    thread_id: u64,
    timestamp: u64,
}

impl StackCanary {
    /// Create new stack canary with unique values.
    pub fn new(thread_id: u64) -> Self {
        let timestamp = crate::time::get_monotonic_time().as_nanos() as u64;
        Self {
            magic: STACK_CANARY_MAGIC,
            thread_id,
            timestamp,
        }
    }
    
    /// Check if canary is intact.
    pub fn check(&self, expected_thread_id: u64) -> bool {
        self.magic == STACK_CANARY_MAGIC && self.thread_id == expected_thread_id
    }
    
    /// Validate canary and panic if corrupted.
    pub fn validate(&self, expected_thread_id: u64) {
        if !self.check(expected_thread_id) {
            handle_security_violation(SecurityViolation::StackCanaryViolation);
        }
    }
}

/// Stack guard page protection.
#[cfg(feature = "mmu")]
pub struct GuardPage {
    start_addr: usize,
    size: usize,
    is_protected: bool,
}

#[cfg(feature = "mmu")]
impl GuardPage {
    /// Create new guard page protection.
    pub fn new(stack_base: usize, stack_size: usize) -> Result<Self, ThreadError> {
        let page_size = get_page_size();
        
        // Place guard page at the end of stack (grows downward)
        let guard_addr = stack_base - page_size;
        
        // Make page non-readable and non-writable
        unsafe {
            if mprotect(guard_addr as *mut u8, page_size, PROT_NONE) != 0 {
                return Err(ThreadError::MemoryError("Failed to create guard page".into()));
            }
        }
        
        STACK_PROTECTION.guard_pages_allocated.fetch_add(1, Ordering::Relaxed);
        
        Ok(Self {
            start_addr: guard_addr,
            size: page_size,
            is_protected: true,
        })
    }
    
    /// Remove guard page protection.
    pub fn unprotect(&mut self) -> Result<(), ThreadError> {
        if self.is_protected {
            unsafe {
                if mprotect(self.start_addr as *mut u8, self.size, PROT_READ | PROT_WRITE) != 0 {
                    return Err(ThreadError::MemoryError("Failed to remove guard page".into()));
                }
            }
            self.is_protected = false;
        }
        Ok(())
    }
}

#[cfg(feature = "mmu")]
impl Drop for GuardPage {
    fn drop(&mut self) {
        let _ = self.unprotect();
    }
}

/// Protected stack with canaries and optional guard pages.
pub struct ProtectedStack {
    stack: Stack,
    canary_bottom: Option<StackCanary>,
    canary_top: Option<StackCanary>,
    #[cfg(feature = "mmu")]
    guard_page: Option<GuardPage>,
    thread_id: u64,
}

impl ProtectedStack {
    /// Create new protected stack.
    pub fn new(stack: Stack, thread_id: u64, config: SecurityConfig) -> Result<Self, ThreadError> {
        let mut protected = Self {
            stack,
            canary_bottom: None,
            canary_top: None,
            #[cfg(feature = "mmu")]
            guard_page: None,
            thread_id,
        };
        
        // Add stack canaries if enabled
        if config.enable_stack_canaries {
            protected.place_canaries()?;
        }
        
        // Add guard pages if enabled
        #[cfg(feature = "mmu")]
        if config.enable_guard_pages {
            let guard_page = GuardPage::new(
                protected.stack.base() as usize,
                protected.stack.size(),
            )?;
            protected.guard_page = Some(guard_page);
        }
        
        Ok(protected)
    }
    
    /// Place stack canaries at bottom and top of stack.
    fn place_canaries(&mut self) -> Result<(), ThreadError> {
        let canary = StackCanary::new(self.thread_id);
        
        // Place canary at bottom of stack
        unsafe {
            let bottom_ptr = self.stack.bottom() as *mut StackCanary;
            bottom_ptr.write(canary);
            self.canary_bottom = Some(canary);
        }
        
        // Place canary near top of stack (leave some space)
        let canary_offset = core::mem::size_of::<StackCanary>() * 2;
        unsafe {
            let top_ptr = (self.stack.top() as usize - canary_offset) as *mut StackCanary;
            top_ptr.write(canary);
            self.canary_top = Some(canary);
        }
        
        STACK_PROTECTION.canaries_placed.fetch_add(2, Ordering::Relaxed);
        Ok(())
    }
    
    /// Check stack canaries for corruption.
    pub fn check_canaries(&self) -> bool {
        let mut valid = true;
        
        if let Some(expected_canary) = &self.canary_bottom {
            unsafe {
                let bottom_ptr = self.stack.bottom() as *const StackCanary;
                let actual_canary = &*bottom_ptr;
                if !actual_canary.check(self.thread_id) {
                    valid = false;
                }
            }
        }
        
        if let Some(expected_canary) = &self.canary_top {
            let canary_offset = core::mem::size_of::<StackCanary>() * 2;
            unsafe {
                let top_ptr = (self.stack.top() as usize - canary_offset) as *const StackCanary;
                let actual_canary = &*top_ptr;
                if !actual_canary.check(self.thread_id) {
                    valid = false;
                }
            }
        }
        
        if !valid {
            STACK_PROTECTION.violations_detected.fetch_add(1, Ordering::Relaxed);
        }
        
        valid
    }
    
    /// Validate canaries and handle violations.
    pub fn validate_canaries(&self) {
        if !self.check_canaries() {
            handle_security_violation(SecurityViolation::StackCanaryViolation);
        }
    }
    
    /// Get the underlying stack.
    pub fn stack(&self) -> &Stack {
        &self.stack
    }
    
    /// Get mutable reference to underlying stack.
    pub fn stack_mut(&mut self) -> &mut Stack {
        &mut self.stack
    }
    
    /// Get usable stack range (excluding canaries).
    pub fn usable_range(&self) -> (usize, usize) {
        let start = if self.canary_bottom.is_some() {
            self.stack.bottom() as usize + core::mem::size_of::<StackCanary>()
        } else {
            self.stack.bottom() as usize
        };
        
        let end = if self.canary_top.is_some() {
            self.stack.top() as usize - core::mem::size_of::<StackCanary>() * 2
        } else {
            self.stack.top() as usize
        };
        
        (start, end)
    }
}

/// Stack protection statistics.
#[derive(Debug, Clone)]
pub struct StackProtectionStats {
    pub canaries_placed: usize,
    pub violations_detected: usize,
    pub guard_pages_allocated: usize,
    pub protection_enabled: bool,
}

/// Initialize stack protection subsystem.
pub fn init_stack_protection(config: SecurityConfig) -> Result<(), ThreadError> {
    #[cfg(feature = "mmu")]
    if config.enable_guard_pages {
        // Verify MMU is available
        if !is_mmu_available() {
            return Err(ThreadError::UnsupportedOperation(
                "Guard pages require MMU support".into()
            ));
        }
    }
    
    // Stack protection initialized with canaries and guard pages enabled based on config
    
    Ok(())
}

/// Get stack protection statistics.
pub fn get_stack_protection_stats() -> StackProtectionStats {
    StackProtectionStats {
        canaries_placed: STACK_PROTECTION.canaries_placed.load(Ordering::Relaxed),
        violations_detected: STACK_PROTECTION.violations_detected.load(Ordering::Relaxed),
        guard_pages_allocated: STACK_PROTECTION.guard_pages_allocated.load(Ordering::Relaxed),
        protection_enabled: SECURITY_STATE.canaries_enabled.load(Ordering::Relaxed) ||
                           SECURITY_STATE.guards_enabled.load(Ordering::Relaxed),
    }
}

// Platform-specific implementations for guard pages

#[cfg(all(feature = "mmu", target_os = "linux"))]
mod linux_impl {
    use super::*;
    
    pub const PROT_NONE: i32 = 0;
    pub const PROT_READ: i32 = 1;
    pub const PROT_WRITE: i32 = 2;
    
    extern "C" {
        fn mprotect(addr: *mut u8, len: usize, prot: i32) -> i32;
        fn sysconf(name: i32) -> i64;
    }
    
    const _SC_PAGESIZE: i32 = 30;
    
    pub unsafe fn mprotect(addr: *mut u8, len: usize, prot: i32) -> i32 {
        mprotect(addr, len, prot)
    }
    
    pub fn get_page_size() -> usize {
        unsafe { sysconf(_SC_PAGESIZE) as usize }
    }
    
    pub fn is_mmu_available() -> bool {
        true // Assume MMU is available on Linux
    }
}

#[cfg(all(feature = "mmu", not(target_os = "linux")))]
mod generic_impl {
    use super::*;
    
    // Generic no-op implementations for platforms without MMU
    pub unsafe fn mprotect(_addr: *mut u8, _len: usize, _prot: i32) -> i32 {
        -1 // Always fail
    }
    
    pub fn get_page_size() -> usize {
        4096 // Common page size
    }
    
    pub fn is_mmu_available() -> bool {
        false
    }
}

#[cfg(feature = "mmu")]
use linux_impl::*;

#[cfg(not(feature = "mmu"))]
mod generic_impl {
    use super::*;

    pub(super) fn allocate_guard_pages(size: usize) -> Result<*mut u8, ThreadError> {
        // Generic implementation - allocate normal memory without actual guard pages
        let layout = core::alloc::Layout::from_size_align(size, 4096)
            .map_err(|_| ThreadError::MemoryError())?;
        let ptr = unsafe { alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            Err(ThreadError::MemoryError())
        } else {
            Ok(ptr)
        }
    }

    pub(super) fn deallocate_guard_pages(ptr: *mut u8, size: usize) {
        if !ptr.is_null() {
            let layout = core::alloc::Layout::from_size_align(size, 4096).unwrap();
            unsafe { alloc::dealloc(ptr, layout) };
        }
    }
}


/// Automatic stack canary checker that validates on drop.
pub struct StackCanaryGuard<'a> {
    protected_stack: &'a ProtectedStack,
}

impl<'a> StackCanaryGuard<'a> {
    pub fn new(protected_stack: &'a ProtectedStack) -> Self {
        Self { protected_stack }
    }
}

impl<'a> Drop for StackCanaryGuard<'a> {
    fn drop(&mut self) {
        self.protected_stack.validate_canaries();
    }
}

/// Macro to automatically check stack canaries.
#[macro_export]
macro_rules! stack_canary_guard {
    ($stack:expr) => {
        let _canary_guard = crate::security::stack_protection::StackCanaryGuard::new($stack);
    };
}
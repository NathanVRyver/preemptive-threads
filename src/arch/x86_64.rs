//! x86_64 architecture implementation.
//!
//! This module provides x86_64-specific context switching, interrupt handling,
//! and FPU management.

use super::Arch;
use core::arch::asm;

/// x86_64 architecture implementation.
pub struct X86_64Arch;

/// x86_64 saved context structure.
///
/// This structure contains all general-purpose registers and flags
/// needed to save and restore thread execution state.
#[repr(C)]
#[derive(Debug)]
pub struct X86_64Context {
    /// General-purpose registers
    pub rsp: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// RFLAGS register
    pub rflags: u64,
    
    /// Extended FPU/SSE state (when full-fpu feature is enabled)
    #[cfg(feature = "full-fpu")]
    pub fpu_state: [u8; 512], // FXSAVE area
}

impl Default for X86_64Context {
    fn default() -> Self {
        Self {
            rsp: 0,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: 0x202, // Default RFLAGS with interrupts enabled
            #[cfg(feature = "full-fpu")]
            fpu_state: [0; 512],
        }
    }
}

unsafe impl Send for X86_64Context {}
unsafe impl Sync for X86_64Context {}

impl Arch for X86_64Arch {
    type SavedContext = X86_64Context;

    /// Perform x86_64 context switch using System V ABI calling convention.
    ///
    /// # Safety
    ///
    /// - Both `prev` and `next` must point to valid, aligned X86_64Context structures
    /// - Must be called with interrupts disabled
    /// - The `next` context must represent a valid execution state
    /// - Stack pointer in `next` context must point to valid, accessible memory
    unsafe fn context_switch(prev: *mut Self::SavedContext, next: *const Self::SavedContext) {
        unsafe {
            asm!(
                // Save current context
                "pushfq",                      // Save RFLAGS on stack
                "pop {rflags}",               // Pop into temp register
                "mov qword ptr [{prev} + 56], {rflags}",   // Store RFLAGS
                "mov qword ptr [{prev} + 0], rsp",         // Save RSP
                "mov qword ptr [{prev} + 8], rbp",         // Save RBP
                "mov qword ptr [{prev} + 16], rbx",        // Save RBX
                "mov qword ptr [{prev} + 24], r12",        // Save R12
                "mov qword ptr [{prev} + 32], r13",        // Save R13
                "mov qword ptr [{prev} + 40], r14",        // Save R14
                "mov qword ptr [{prev} + 48], r15",        // Save R15
                
                // Restore next context
                "mov rsp, qword ptr [{next} + 0]",         // Restore RSP
                "mov rbp, qword ptr [{next} + 8]",         // Restore RBP
                "mov rbx, qword ptr [{next} + 16]",        // Restore RBX
                "mov r12, qword ptr [{next} + 24]",        // Restore R12
                "mov r13, qword ptr [{next} + 32]",        // Restore R13
                "mov r14, qword ptr [{next} + 40]",        // Restore R14
                "mov r15, qword ptr [{next} + 48]",        // Restore R15
                "push qword ptr [{next} + 56]",            // Push RFLAGS onto stack
                "popfq",                      // Restore RFLAGS
                
                prev = in(reg) prev,
                next = in(reg) next,
                rflags = out(reg) _,
                options(nostack)
            );
        }
    }

    #[cfg(feature = "full-fpu")]
    unsafe fn save_fpu(ctx: &mut Self::SavedContext) {
        unsafe {
            asm!(
                "fxsave [{}]",
                in(reg) ctx.fpu_state.as_mut_ptr(),
                options(nostack, preserves_flags)
            );
        }
    }

    #[cfg(feature = "full-fpu")]
    unsafe fn restore_fpu(ctx: &Self::SavedContext) {
        unsafe {
            asm!(
                "fxrstor [{}]",
                in(reg) ctx.fpu_state.as_ptr(),
                options(nostack, preserves_flags)
            );
        }
    }

    fn enable_interrupts() {
        unsafe {
            asm!("sti", options(nostack, nomem, preserves_flags));
        }
    }

    fn disable_interrupts() {
        unsafe {
            asm!("cli", options(nostack, nomem, preserves_flags));
        }
    }

    fn interrupts_enabled() -> bool {
        let flags: u64;
        unsafe {
            asm!("pushfq", "pop {}", out(reg) flags, options(nostack, nomem, preserves_flags));
        }
        (flags & 0x200) != 0 // Test interrupt flag (IF)
    }
}

/// Initialize x86_64-specific features.
///
/// This function sets up any architecture-specific features that need
/// initialization before threading can begin.
///
/// # Safety
///
/// Must be called once during system initialization with interrupts disabled.
pub unsafe fn init() {
    // Initialize timer subsystem
    #[cfg(feature = "x86_64")]
    {
        if let Err(e) = unsafe { crate::time::x86_64_timer::init() } {
            // TODO: Better error handling
            panic!("Failed to initialize x86_64 timer: {:?}", e);
        }
    }
    
    // TODO: Initialize other x86_64 features:
    // - Set up IDT entries for timer interrupt
    // - Configure APIC
    // - Set up system call interface
}

/// x86_64-specific timer interrupt handler.
///
/// This function should be called from the timer interrupt service routine
/// to handle preemptive scheduling.
///
/// # Safety
///
/// Must be called from an interrupt context with a valid interrupt stack frame.
pub unsafe fn timer_interrupt_handler() {
    // Delegate to the timer subsystem
    unsafe {
        crate::time::timer::handle_timer_interrupt();
    }
}

/// Memory barrier operations for x86_64.
pub fn memory_barrier_full() {
    unsafe {
        asm!("mfence", options(nomem, nostack));
    }
}

pub fn memory_barrier_acquire() {
    unsafe {
        asm!("lfence", options(nomem, nostack));
    }
}

pub fn memory_barrier_release() {
    unsafe {
        asm!("sfence", options(nomem, nostack));
    }
}

/// CPU cache maintenance for x86_64.
pub unsafe fn flush_dcache_range(start: *const u8, len: usize) {
    unsafe {
        let end = start.add(len);
        let mut addr = start as usize & !63; // Align to cache line (64 bytes)
        
        while addr < end as usize {
            asm!(
                "clflush ({addr})",
                addr = in(reg) addr,
                options(nomem, nostack)
            );
            addr += 64; // Next cache line
        }
        
        // Memory fence to ensure completion
        memory_barrier_full();
    }
}

/// Invalidate instruction cache for x86_64.
pub unsafe fn flush_icache() {
    // x86_64 has coherent instruction cache - no explicit flush needed
    // Just ensure all stores are visible
    memory_barrier_full();
}
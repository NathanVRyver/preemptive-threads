//! Architecture abstraction layer for context switching and interrupt handling.
//!
//! This module provides a unified interface for architecture-specific operations
//! that need to be implemented for each supported CPU architecture.

// PhantomData import not needed yet
// use core::marker::PhantomData;

/// Architecture abstraction trait.
///
/// This trait must be implemented for each supported CPU architecture to provide
/// context switching, interrupt handling, and FPU management capabilities.
///
/// # Safety
///
/// Implementations of this trait involve direct hardware manipulation and
/// inline assembly. All methods marked as unsafe have specific preconditions
/// that must be upheld by the caller.
pub trait Arch {
    /// Architecture-specific saved context type.
    ///
    /// This type must contain all CPU registers and state needed to fully
    /// restore a thread's execution context.
    type SavedContext: Send + Sync;

    /// Switch from one thread context to another.
    ///
    /// # Safety
    ///
    /// - `prev` must point to a valid, properly aligned SavedContext
    /// - `next` must point to a valid, properly aligned SavedContext  
    /// - The caller must ensure the memory pointed to by both pointers remains
    ///   valid for the duration of this call
    /// - Must be called with interrupts disabled
    /// - The `next` context must represent a valid execution state
    unsafe fn context_switch(prev: *mut Self::SavedContext, next: *const Self::SavedContext);

    /// Save floating point unit state to the given context.
    ///
    /// # Safety
    ///
    /// - `ctx` must point to a valid, properly aligned SavedContext
    /// - Must be called when the current thread owns the FPU
    /// - The context must have sufficient space for FPU state
    #[cfg(feature = "full-fpu")]
    unsafe fn save_fpu(ctx: &mut Self::SavedContext);

    /// Restore floating point unit state from the given context.
    ///
    /// # Safety
    ///
    /// - `ctx` must contain valid FPU state
    /// - Must be called before the thread uses FPU instructions
    /// - The current thread must be the owner of the FPU
    #[cfg(feature = "full-fpu")]
    unsafe fn restore_fpu(ctx: &Self::SavedContext);

    /// Enable interrupts on the current CPU.
    ///
    /// This function re-enables interrupt delivery, allowing preemption
    /// and timer interrupts to occur.
    fn enable_interrupts();

    /// Disable interrupts on the current CPU.
    ///
    /// This function prevents interrupt delivery, creating a critical section
    /// where the current thread cannot be preempted.
    fn disable_interrupts();

    /// Check if interrupts are currently enabled.
    ///
    /// Returns `true` if interrupts are enabled, `false` otherwise.
    fn interrupts_enabled() -> bool;
}

/// A no-op architecture implementation for testing and fallback purposes.
///
/// This implementation provides stub functionality and should not be used
/// in production code where real context switching is required.
pub struct NoOpArch;

impl Arch for NoOpArch {
    type SavedContext = ();

    unsafe fn context_switch(_prev: *mut Self::SavedContext, _next: *const Self::SavedContext) {
        // No-op for testing
    }

    #[cfg(feature = "full-fpu")]
    unsafe fn save_fpu(_ctx: &mut Self::SavedContext) {
        // No-op for testing  
    }

    #[cfg(feature = "full-fpu")]
    unsafe fn restore_fpu(_ctx: &Self::SavedContext) {
        // No-op for testing
    }

    fn enable_interrupts() {
        // No-op for testing
    }

    fn disable_interrupts() {
        // No-op for testing
    }

    fn interrupts_enabled() -> bool {
        true
    }
}

// Include architecture-specific implementations
#[cfg(feature = "x86_64")]
pub mod x86_64;

#[cfg(feature = "arm64")]
pub mod aarch64;

#[cfg(feature = "riscv64")]
pub mod riscv;

pub mod barriers;
pub mod detection;

// Re-export the default architecture for the current target
#[cfg(all(target_arch = "x86_64", feature = "x86_64"))]
pub use x86_64::X86_64Arch as DefaultArch;

#[cfg(all(target_arch = "aarch64", feature = "arm64"))]
pub use aarch64::Aarch64Arch as DefaultArch;

#[cfg(all(any(target_arch = "riscv64"), feature = "riscv64"))]
pub use riscv::RiscvArch as DefaultArch;

#[cfg(all(feature = "std-shim", not(any(feature = "x86_64", feature = "arm64", feature = "riscv64"))))]
pub use NoOpArch as DefaultArch;

// Fallback for when no specific architecture is enabled
#[cfg(not(any(
    all(target_arch = "x86_64", feature = "x86_64"),
    all(target_arch = "aarch64", feature = "arm64"), 
    all(any(target_arch = "riscv64"), feature = "riscv64"),
    all(feature = "std-shim", not(any(feature = "x86_64", feature = "arm64", feature = "riscv64")))
)))]
pub use NoOpArch as DefaultArch;
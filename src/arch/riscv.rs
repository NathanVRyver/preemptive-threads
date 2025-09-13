//! RISC-V architecture implementation.
//!
//! This module provides RISC-V-specific context switching, interrupt handling,
//! and vector extension support for high-performance computing.

use super::Arch;
use core::arch::asm;
use portable_atomic::{AtomicU64, Ordering};

/// RISC-V architecture implementation.
pub struct RiscvArch;

/// RISC-V saved context structure.
///
/// Contains all general-purpose registers, stack pointer, and vector state
/// needed to save and restore thread execution state.
#[repr(C)]
#[derive(Debug)]
pub struct RiscvContext {
    /// General-purpose registers x1-x31 (x0 is always zero)
    pub x: [u64; 31],
    /// Stack pointer (x2/sp)
    pub sp: u64,
    /// Program counter  
    pub pc: u64,
    /// Status register
    pub status: u64,
    
    /// Vector state (when vector extension is enabled)
    #[cfg(feature = "riscv-vector")]
    pub vector_state: [u64; 32], // v0-v31 vector registers (simplified)
    #[cfg(feature = "riscv-vector")]
    pub vstart: u64,   // Vector start index
    #[cfg(feature = "riscv-vector")]
    pub vxsat: u64,    // Vector fixed-point saturation flag
    #[cfg(feature = "riscv-vector")]
    pub vxrm: u64,     // Vector fixed-point rounding mode
    #[cfg(feature = "riscv-vector")]
    pub vl: u64,       // Vector length
    #[cfg(feature = "riscv-vector")]
    pub vtype: u64,    // Vector type
    
    /// Floating-point state (when F/D extension is enabled)
    #[cfg(feature = "riscv-float")]
    pub f: [u64; 32],  // f0-f31 floating-point registers
    #[cfg(feature = "riscv-float")]
    pub fcsr: u32,     // Floating-point control and status register
}

impl Default for RiscvContext {
    fn default() -> Self {
        Self {
            x: [0; 31],
            sp: 0,
            pc: 0,
            status: 0x00000000, // Default status (user mode, interrupts disabled)
            #[cfg(feature = "riscv-vector")]
            vector_state: [0; 32],
            #[cfg(feature = "riscv-vector")]
            vstart: 0,
            #[cfg(feature = "riscv-vector")]
            vxsat: 0,
            #[cfg(feature = "riscv-vector")]
            vxrm: 0,
            #[cfg(feature = "riscv-vector")]
            vl: 0,
            #[cfg(feature = "riscv-vector")]
            vtype: 0,
            #[cfg(feature = "riscv-float")]
            f: [0; 32],
            #[cfg(feature = "riscv-float")]
            fcsr: 0,
        }
    }
}

unsafe impl Send for RiscvContext {}
unsafe impl Sync for RiscvContext {}

impl Arch for RiscvArch {
    type SavedContext = RiscvContext;

    unsafe fn context_switch(prev: *mut Self::SavedContext, next: *const Self::SavedContext) {
        unsafe {
            asm!(
                // Save current context (x1-x31)
                "sd x1, 0({prev})",     // ra (return address)
                "sd x3, 16({prev})",    // gp (global pointer)
                "sd x4, 24({prev})",    // tp (thread pointer)
                "sd x5, 32({prev})",    // t0
                "sd x6, 40({prev})",    // t1
                "sd x7, 48({prev})",    // t2
                "sd x8, 56({prev})",    // s0/fp (frame pointer)
                "sd x9, 64({prev})",    // s1
                "sd x10, 72({prev})",   // a0
                "sd x11, 80({prev})",   // a1
                "sd x12, 88({prev})",   // a2
                "sd x13, 96({prev})",   // a3
                "sd x14, 104({prev})",  // a4
                "sd x15, 112({prev})",  // a5
                "sd x16, 120({prev})",  // a6
                "sd x17, 128({prev})",  // a7
                "sd x18, 136({prev})",  // s2
                "sd x19, 144({prev})",  // s3
                "sd x20, 152({prev})",  // s4
                "sd x21, 160({prev})",  // s5
                "sd x22, 168({prev})",  // s6
                "sd x23, 176({prev})",  // s7
                "sd x24, 184({prev})",  // s8
                "sd x25, 192({prev})",  // s9
                "sd x26, 200({prev})",  // s10
                "sd x27, 208({prev})",  // s11
                "sd x28, 216({prev})",  // t3
                "sd x29, 224({prev})",  // t4
                "sd x30, 232({prev})",  // t5
                "sd x31, 240({prev})",  // t6
                
                // Save stack pointer
                "sd sp, 248({prev})",   // sp offset
                
                // Save program counter (return address)
                "la t0, 1f",            // load return address
                "sd t0, 256({prev})",   // pc offset
                
                // Save status register
                "csrr t0, sstatus",
                "sd t0, 264({prev})",   // status offset
                
                // Load new context
                "ld x1, 0({next})",     // ra
                "ld x3, 16({next})",    // gp
                "ld x4, 24({next})",    // tp
                "ld x5, 32({next})",    // t0
                "ld x6, 40({next})",    // t1
                "ld x7, 48({next})",    // t2
                "ld x8, 56({next})",    // s0/fp
                "ld x9, 64({next})",    // s1
                "ld x10, 72({next})",   // a0
                "ld x11, 80({next})",   // a1
                "ld x12, 88({next})",   // a2
                "ld x13, 96({next})",   // a3
                "ld x14, 104({next})",  // a4
                "ld x15, 112({next})",  // a5
                "ld x16, 120({next})",  // a6
                "ld x17, 128({next})",  // a7
                "ld x18, 136({next})",  // s2
                "ld x19, 144({next})",  // s3
                "ld x20, 152({next})",  // s4
                "ld x21, 160({next})",  // s5
                "ld x22, 168({next})",  // s6
                "ld x23, 176({next})",  // s7
                "ld x24, 184({next})",  // s8
                "ld x25, 192({next})",  // s9
                "ld x26, 200({next})",  // s10
                "ld x27, 208({next})",  // s11
                "ld x28, 216({next})",  // t3
                "ld x29, 224({next})",  // t4
                "ld x30, 232({next})",  // t5
                "ld x31, 240({next})",  // t6
                
                // Load stack pointer
                "ld sp, 248({next})",   // sp
                
                // Load status register  
                "ld t0, 264({next})",   // status
                "csrw sstatus, t0",
                
                // Jump to new context
                "ld t0, 256({next})",   // load pc
                "jr t0",                // jump to new context
                
                "1:",                   // return label for save
                prev = in(reg) prev,
                next = in(reg) next,
                out("t0") _,
                options(nostack)
            );
        }
    }

    #[cfg(feature = "riscv-float")]
    unsafe fn save_fpu(ctx: &mut Self::SavedContext) {
        unsafe {
            asm!(
                // Save floating-point registers f0-f31
                "fsd f0, 272({ctx})",   // f state offset
                "fsd f1, 280({ctx})",
                "fsd f2, 288({ctx})",
                "fsd f3, 296({ctx})",
                "fsd f4, 304({ctx})",
                "fsd f5, 312({ctx})",
                "fsd f6, 320({ctx})",
                "fsd f7, 328({ctx})",
                "fsd f8, 336({ctx})",
                "fsd f9, 344({ctx})",
                "fsd f10, 352({ctx})",
                "fsd f11, 360({ctx})",
                "fsd f12, 368({ctx})",
                "fsd f13, 376({ctx})",
                "fsd f14, 384({ctx})",
                "fsd f15, 392({ctx})",
                "fsd f16, 400({ctx})",
                "fsd f17, 408({ctx})",
                "fsd f18, 416({ctx})",
                "fsd f19, 424({ctx})",
                "fsd f20, 432({ctx})",
                "fsd f21, 440({ctx})",
                "fsd f22, 448({ctx})",
                "fsd f23, 456({ctx})",
                "fsd f24, 464({ctx})",
                "fsd f25, 472({ctx})",
                "fsd f26, 480({ctx})",
                "fsd f27, 488({ctx})",
                "fsd f28, 496({ctx})",
                "fsd f29, 504({ctx})",
                "fsd f30, 512({ctx})",
                "fsd f31, 520({ctx})",
                
                // Save floating-point control and status register
                "csrr t0, fcsr",
                "sw t0, 528({ctx})",    // fcsr offset
                ctx = in(reg) ctx,
                out("t0") _,
                options(nostack)
            );
        }
    }

    #[cfg(feature = "riscv-float")]
    unsafe fn restore_fpu(ctx: &Self::SavedContext) {
        unsafe {
            asm!(
                // Restore floating-point control and status register
                "lw t0, 528({ctx})",    // fcsr offset
                "csrw fcsr, t0",
                
                // Restore floating-point registers f0-f31
                "fld f0, 272({ctx})",   // f state offset
                "fld f1, 280({ctx})",
                "fld f2, 288({ctx})",
                "fld f3, 296({ctx})",
                "fld f4, 304({ctx})",
                "fld f5, 312({ctx})",
                "fld f6, 320({ctx})",
                "fld f7, 328({ctx})",
                "fld f8, 336({ctx})",
                "fld f9, 344({ctx})",
                "fld f10, 352({ctx})",
                "fld f11, 360({ctx})",
                "fld f12, 368({ctx})",
                "fld f13, 376({ctx})",
                "fld f14, 384({ctx})",
                "fld f15, 392({ctx})",
                "fld f16, 400({ctx})",
                "fld f17, 408({ctx})",
                "fld f18, 416({ctx})",
                "fld f19, 424({ctx})",
                "fld f20, 432({ctx})",
                "fld f21, 440({ctx})",
                "fld f22, 448({ctx})",
                "fld f23, 456({ctx})",
                "fld f24, 464({ctx})",
                "fld f25, 472({ctx})",
                "fld f26, 480({ctx})",
                "fld f27, 488({ctx})",
                "fld f28, 496({ctx})",
                "fld f29, 504({ctx})",
                "fld f30, 512({ctx})",
                "fld f31, 520({ctx})",
                ctx = in(reg) ctx,
                out("t0") _,
                options(nostack)
            );
        }
    }

    fn enable_interrupts() {
        unsafe {
            asm!(
                "csrsi sstatus, 0x2",  // Set SIE bit (bit 1) in sstatus
                options(nomem, nostack)
            );
        }
    }

    fn disable_interrupts() {
        unsafe {
            asm!(
                "csrci sstatus, 0x2",  // Clear SIE bit (bit 1) in sstatus
                options(nomem, nostack)
            );
        }
    }

    fn interrupts_enabled() -> bool {
        let sstatus: u64;
        unsafe {
            asm!(
                "csrr {sstatus}, sstatus",
                sstatus = out(reg) sstatus,
                options(nostack, readonly)
            );
        }
        (sstatus & 0x2) != 0  // Check SIE bit (bit 1)
    }
}

// Timer frequency storage
static TIMER_FREQ: AtomicU64 = AtomicU64::new(0);

/// Initialize RISC-V-specific features.
pub fn init() {
    unsafe {
        // Read timer frequency (typically from device tree or platform-specific)
        // For now, assume a common 10MHz frequency
        let freq = 10_000_000u64; // 10MHz
        TIMER_FREQ.store(freq, Ordering::Relaxed);
        
        // Enable timer interrupts
        asm!(
            "csrsi sie, 0x20",  // Enable timer interrupts (STIE bit 5)
            options(nomem, nostack)
        );
    }
}

/// Set up RISC-V timer for preemption with specified interval in microseconds.
pub unsafe fn setup_preemption_timer(interval_us: u32) -> Result<(), &'static str> {
    let freq = TIMER_FREQ.load(Ordering::Relaxed);
    if freq == 0 {
        return Err("Timer frequency not initialized");
    }
    
    // Calculate ticks for the desired interval
    let ticks = (freq * interval_us as u64) / 1_000_000;
    
    unsafe {
        // Read current time
        let current: u64;
        asm!(
            "csrr {current}, time",
            current = out(reg) current,
            options(nostack, readonly)
        );
        
        // Set compare value (current + interval)  
        let compare_val = current + ticks;
        asm!(
            "csrw stimecmp, {val}",
            val = in(reg) compare_val,
            options(nomem, nostack)
        );
    }
    
    Ok(())
}

/// Get current RISC-V timestamp counter value.
pub fn get_timestamp() -> u64 {
    let time: u64;
    unsafe {
        asm!(
            "csrr {time}, time",
            time = out(reg) time,
            options(nostack, readonly)
        );
    }
    time
}

/// Convert RISC-V timer ticks to nanoseconds.
pub fn ticks_to_ns(ticks: u64) -> u64 {
    let freq = TIMER_FREQ.load(Ordering::Relaxed);
    if freq == 0 {
        return 0;
    }
    (ticks * 1_000_000_000) / freq
}

/// Convert nanoseconds to RISC-V timer ticks.
pub fn ns_to_ticks(ns: u64) -> u64 {
    let freq = TIMER_FREQ.load(Ordering::Relaxed);
    if freq == 0 {
        return 0;
    }
    (ns * freq) / 1_000_000_000
}

/// RISC-V-specific timer interrupt handler.
pub unsafe fn timer_interrupt_handler() {
    unsafe {
        // Clear timer interrupt by setting stimecmp to max value
        asm!(
            "csrw stimecmp, {val}",
            val = in(reg) u64::MAX,
            options(nomem, nostack)
        );
        
        // Get scheduler reference and handle preemption
        if let Some(scheduler) = crate::scheduler::get_global_scheduler() {
            scheduler.handle_timer_interrupt();
        }
        
        // Re-setup timer for next preemption (1ms default)
        if setup_preemption_timer(1000).is_err() {
            // Timer setup failed, disable preemption
            return;
        }
    }
}

/// Memory barrier operations for RISC-V.
pub fn memory_barrier_full() {
    unsafe {
        asm!("fence rw,rw", options(nomem, nostack));
    }
}

pub fn memory_barrier_acquire() {
    unsafe {
        asm!("fence r,rw", options(nomem, nostack));
    }
}

pub fn memory_barrier_release() {
    unsafe {
        asm!("fence rw,w", options(nomem, nostack));
    }
}

/// Cache maintenance for RISC-V.
pub unsafe fn flush_dcache_range(start: *const u8, len: usize) {
    // RISC-V doesn't have standard cache maintenance instructions
    // This would be platform-specific, so we use a memory fence
    memory_barrier_full();
    
    // On some RISC-V implementations, we might use custom CSR instructions
    // But this is highly platform-dependent
    let _end = unsafe { start.add(len) };
    // Platform-specific cache flush would go here
}

/// Invalidate instruction cache for RISC-V.
pub unsafe fn flush_icache() {
    unsafe {
        asm!(
            "fence.i",  // Instruction fence - flush instruction cache
            options(nomem, nostack)
        );
    }
}

/// Vector extension support for RISC-V (RVV).
#[cfg(feature = "riscv-vector")]
pub unsafe fn save_vector_state(ctx: &mut RiscvContext) {
    unsafe {
        // Save vector CSRs
        asm!(
            "csrr t0, vstart",
            "sd t0, 528({ctx})",      // vstart offset
            "csrr t0, vxsat", 
            "sd t0, 536({ctx})",      // vxsat offset
            "csrr t0, vxrm",
            "sd t0, 544({ctx})",      // vxrm offset
            "csrr t0, vl",
            "sd t0, 552({ctx})",      // vl offset
            "csrr t0, vtype",
            "sd t0, 560({ctx})",      // vtype offset
            ctx = in(reg) ctx,
            out("t0") _,
            options(nostack)
        );
        
        // Save vector registers - this is simplified
        // Real implementation would save all vector register data
        // based on current vtype and vl settings
        // Note: Using a macro to generate const values for each register
        macro_rules! save_vreg {
            ($reg:literal) => {
                let offset = 272 + $reg * 8; // vector_state offset
                asm!(
                    concat!("vse64.v v", stringify!($reg), ", ({addr})"),
                    addr = in(reg) (ctx as *mut RiscvContext as usize + offset),
                    options(nostack)
                );
            };
        }
        
        // Save first 8 vector registers (simplified for compilation)
        save_vreg!(0); save_vreg!(1); save_vreg!(2); save_vreg!(3);
        save_vreg!(4); save_vreg!(5); save_vreg!(6); save_vreg!(7);
    }
}

#[cfg(feature = "riscv-vector")]
pub unsafe fn restore_vector_state(ctx: &RiscvContext) {
    unsafe {
        // Restore vector CSRs first
        asm!(
            "ld t0, 528({ctx})",      // vstart offset
            "csrw vstart, t0",
            "ld t0, 536({ctx})",      // vxsat offset
            "csrw vxsat, t0",
            "ld t0, 544({ctx})",      // vxrm offset
            "csrw vxrm, t0", 
            "ld t0, 552({ctx})",      // vl offset
            "csrw vl, t0",
            "ld t0, 560({ctx})",      // vtype offset  
            "csrw vtype, t0",
            ctx = in(reg) ctx,
            out("t0") _,
            options(nostack)
        );
        
        // Restore vector registers  
        // Note: Using a macro to generate const values for each register
        macro_rules! restore_vreg {
            ($reg:literal) => {
                let offset = 272 + $reg * 8; // vector_state offset
                asm!(
                    concat!("vle64.v v", stringify!($reg), ", ({addr})"),
                    addr = in(reg) (ctx as *const RiscvContext as usize + offset),
                    options(nostack)
                );
            };
        }
        
        // Restore first 8 vector registers (simplified for compilation)
        restore_vreg!(0); restore_vreg!(1); restore_vreg!(2); restore_vreg!(3);
        restore_vreg!(4); restore_vreg!(5); restore_vreg!(6); restore_vreg!(7);
    }
}
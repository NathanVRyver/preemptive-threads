
/// Full CPU context including FPU/SSE/AVX state
#[repr(C, align(64))]
pub struct FullThreadContext {
    /// General purpose registers
    pub rsp: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rflags: u64,
    
    /// FPU/SSE/AVX state (512 bytes for XSAVE area)
    pub xsave_area: [u8; 512],
    
    /// MXCSR register for SSE state
    pub mxcsr: u32,
    pub mxcsr_mask: u32,
}

impl FullThreadContext {
    pub const fn new() -> Self {
        Self {
            rsp: 0,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: 0x202, // Default RFLAGS with interrupts enabled
            xsave_area: [0; 512],
            mxcsr: 0x1F80, // Default MXCSR
            mxcsr_mask: 0xFFFF,
        }
    }
}

/// Check CPU features at runtime
pub fn check_cpu_features() -> CpuFeatures {
    let mut features = CpuFeatures::default();
    
    unsafe {
        // Check for XSAVE support
        let result = core::arch::x86_64::__cpuid_count(1, 0);
        features.xsave = (result.ecx & (1 << 26)) != 0;
        features.avx = (result.ecx & (1 << 28)) != 0;
        features.fma = (result.ecx & (1 << 12)) != 0;
        
        // Check for AVX2
        let result = core::arch::x86_64::__cpuid_count(7, 0);
        features.avx2 = (result.ebx & (1 << 5)) != 0;
        
        // Check for AVX-512
        features.avx512f = (result.ebx & (1 << 16)) != 0;
        
        // Get XSAVE feature mask
        if features.xsave {
            let result = core::arch::x86_64::__cpuid_count(0xD, 0);
            features.xsave_mask = ((result.edx as u64) << 32) | (result.eax as u64);
        }
    }
    
    features
}

#[derive(Default)]
pub struct CpuFeatures {
    pub xsave: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub fma: bool,
    pub xsave_mask: u64,
}

pub static mut CPU_FEATURES: CpuFeatures = CpuFeatures {
    xsave: false,
    avx: false,
    avx2: false,
    avx512f: false,
    fma: false,
    xsave_mask: 0,
};

/// Initialize CPU feature detection
pub fn init_cpu_features() {
    unsafe {
        CPU_FEATURES = check_cpu_features();
    }
}

/// Full context switch with all CPU state
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn switch_context_full(from: *mut FullThreadContext, to: *const FullThreadContext) {
    core::arch::naked_asm!(
        "
        # Save callee-saved registers
        push rbp
        push rbx
        push r12
        push r13
        push r14
        push r15
        
        # Save RFLAGS
        pushfq
        pop rax
        mov [rdi + 56], rax    # Save RFLAGS to context
        
        # Save stack pointer
        mov [rdi + 0], rsp
        mov [rdi + 8], rbp
        mov [rdi + 16], rbx
        mov [rdi + 24], r12
        mov [rdi + 32], r13
        mov [rdi + 40], r14
        mov [rdi + 48], r15
        
        # For now, just use FXSAVE/FXRSTOR which is always available
        # Save FPU/SSE state
        fxsave [rdi + 64]
        
        # Restore FPU/SSE state
        fxrstor [rsi + 64]
        
        # Restore general registers
        mov rsp, [rsi + 0]
        mov rbp, [rsi + 8]
        mov rbx, [rsi + 16]
        mov r12, [rsi + 24]
        mov r13, [rsi + 32]
        mov r14, [rsi + 40]
        mov r15, [rsi + 48]
        
        # Restore RFLAGS
        mov rax, [rsi + 56]
        push rax
        popfq
        
        # Restore callee-saved registers from stack
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp
        
        ret
        "
    );
}

/// Simple context switch (enhanced version with proper RFLAGS handling)
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn switch_context_simple(from: *mut crate::thread::ThreadContext, to: *const crate::thread::ThreadContext) {
    core::arch::naked_asm!(
        "
        # Save callee-saved registers in correct order
        push rbp
        push rbx
        push r12
        push r13
        push r14
        push r15
        
        # Save RFLAGS
        pushfq
        
        # Save current context
        mov [rdi + 0], rsp
        mov [rdi + 8], rbp
        mov [rdi + 16], rbx
        mov [rdi + 24], r12
        mov [rdi + 32], r13
        mov [rdi + 40], r14
        mov [rdi + 48], r15
        
        # Switch to new context
        mov rsp, [rsi + 0]
        mov rbp, [rsi + 8]
        mov rbx, [rsi + 16]
        mov r12, [rsi + 24]
        mov r13, [rsi + 32]
        mov r14, [rsi + 40]
        mov r15, [rsi + 48]
        
        # Restore RFLAGS
        popfq
        
        # Restore callee-saved registers in reverse order
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp
        
        ret
        "
    );
}
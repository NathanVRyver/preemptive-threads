//! Architecture detection and runtime optimization.
//!
//! This module provides runtime detection of CPU features and capabilities
//! to enable architecture-specific optimizations.

use portable_atomic::{AtomicU32, AtomicBool, Ordering};

/// CPU architecture types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArch {
    X86_64,
    Aarch64,
    RiscV64,
    Unknown,
}

/// CPU feature flags for different architectures.
#[derive(Debug, Clone, Copy)]
pub struct CpuFeatures {
    pub arch: CpuArch,
    pub cache_line_size: u32,
    pub cpu_cores: u32,
    pub supports_fpu: bool,
    pub supports_vector: bool,
    pub supports_atomic_cas: bool,
    pub supports_memory_ordering: bool,
    
    // x86_64-specific features
    #[cfg(feature = "x86_64")]
    pub supports_sse: bool,
    #[cfg(feature = "x86_64")]
    pub supports_avx: bool,
    #[cfg(feature = "x86_64")]
    pub supports_avx512: bool,
    
    // ARM64-specific features
    #[cfg(feature = "arm64")]
    pub supports_neon: bool,
    #[cfg(feature = "arm64")]
    pub supports_sve: bool,
    #[cfg(feature = "arm64")]
    pub supports_sve2: bool,
    
    // RISC-V-specific features
    #[cfg(feature = "riscv64")]
    pub supports_riscv_f: bool,
    #[cfg(feature = "riscv64")]
    pub supports_riscv_d: bool,
    #[cfg(feature = "riscv64")]
    pub supports_riscv_v: bool,
}

static CPU_FEATURES: spin::Mutex<Option<CpuFeatures>> = spin::Mutex::new(None);
static DETECTION_DONE: AtomicBool = AtomicBool::new(false);

/// Detect current CPU architecture and features.
pub fn detect_cpu_features() -> CpuFeatures {
    // Fast path - check if already detected
    if DETECTION_DONE.load(Ordering::Acquire) {
        let guard = CPU_FEATURES.lock();
        if let Some(features) = *guard {
            return features;
        }
    }
    
    // Slow path - perform detection
    let features = perform_detection();
    
    // Store results
    {
        let mut guard = CPU_FEATURES.lock();
        *guard = Some(features);
    }
    DETECTION_DONE.store(true, Ordering::Release);
    
    features
}

/// Get cached CPU features (must call detect_cpu_features first).
pub fn get_cpu_features() -> Option<CpuFeatures> {
    if DETECTION_DONE.load(Ordering::Acquire) {
        let guard = CPU_FEATURES.lock();
        *guard
    } else {
        None
    }
}

/// Internal CPU feature detection.
fn perform_detection() -> CpuFeatures {
    let arch = detect_architecture();
    let cache_line_size = detect_cache_line_size(arch);
    let cpu_cores = detect_cpu_cores();
    
    CpuFeatures {
        arch,
        cache_line_size,
        cpu_cores,
        supports_fpu: detect_fpu_support(arch),
        supports_vector: detect_vector_support(arch),
        supports_atomic_cas: detect_atomic_cas_support(arch),
        supports_memory_ordering: detect_memory_ordering_support(arch),
        
        #[cfg(feature = "x86_64")]
        supports_sse: detect_x86_64_sse(),
        #[cfg(feature = "x86_64")]
        supports_avx: detect_x86_64_avx(),
        #[cfg(feature = "x86_64")]
        supports_avx512: detect_x86_64_avx512(),
        
        #[cfg(feature = "arm64")]
        supports_neon: detect_arm64_neon(),
        #[cfg(feature = "arm64")]
        supports_sve: detect_arm64_sve(),
        #[cfg(feature = "arm64")]
        supports_sve2: detect_arm64_sve2(),
        
        #[cfg(feature = "riscv64")]
        supports_riscv_f: detect_riscv_f_extension(),
        #[cfg(feature = "riscv64")]
        supports_riscv_d: detect_riscv_d_extension(),
        #[cfg(feature = "riscv64")]
        supports_riscv_v: detect_riscv_v_extension(),
    }
}

/// Detect the current CPU architecture.
fn detect_architecture() -> CpuArch {
    #[cfg(target_arch = "x86_64")]
    return CpuArch::X86_64;
    
    #[cfg(target_arch = "aarch64")]
    return CpuArch::Aarch64;
    
    #[cfg(target_arch = "riscv64")]
    return CpuArch::RiscV64;
    
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
    return CpuArch::Unknown;
}

/// Detect cache line size for the current architecture.
fn detect_cache_line_size(arch: CpuArch) -> u32 {
    match arch {
        CpuArch::X86_64 => {
            #[cfg(feature = "x86_64")]
            return detect_x86_64_cache_line_size();
            #[cfg(not(feature = "x86_64"))]
            return 64;
        }
        CpuArch::Aarch64 => {
            #[cfg(feature = "arm64")]
            return detect_arm64_cache_line_size();
            #[cfg(not(feature = "arm64"))]
            return 64;
        }
        CpuArch::RiscV64 => {
            #[cfg(feature = "riscv64")]
            return detect_riscv_cache_line_size();
            #[cfg(not(feature = "riscv64"))]
            return 64;
        }
        CpuArch::Unknown => 64, // Safe default
    }
}

/// Detect number of CPU cores.
fn detect_cpu_cores() -> u32 {
    // This would typically read from system information
    // For now, return a safe default
    1
}

/// Detect FPU support for the given architecture.
fn detect_fpu_support(arch: CpuArch) -> bool {
    match arch {
        CpuArch::X86_64 => true, // x86_64 always has FPU
        CpuArch::Aarch64 => true, // ARM64 always has FPU
        CpuArch::RiscV64 => {
            #[cfg(feature = "riscv64")]
            return detect_riscv_f_extension();
            #[cfg(not(feature = "riscv64"))]
            return false;
        }
        CpuArch::Unknown => false,
    }
}

/// Detect vector instruction support.
fn detect_vector_support(arch: CpuArch) -> bool {
    match arch {
        CpuArch::X86_64 => {
            #[cfg(feature = "x86_64")]
            return detect_x86_64_sse();
            #[cfg(not(feature = "x86_64"))]
            return false;
        }
        CpuArch::Aarch64 => {
            #[cfg(feature = "arm64")]
            return detect_arm64_neon();
            #[cfg(not(feature = "arm64"))]
            return false;
        }
        CpuArch::RiscV64 => {
            #[cfg(feature = "riscv64")]
            return detect_riscv_v_extension();
            #[cfg(not(feature = "riscv64"))]
            return false;
        }
        CpuArch::Unknown => false,
    }
}

/// Detect atomic compare-and-swap support.
fn detect_atomic_cas_support(arch: CpuArch) -> bool {
    match arch {
        CpuArch::X86_64 => true, // x86_64 always supports CAS
        CpuArch::Aarch64 => true, // ARM64 always supports CAS
        CpuArch::RiscV64 => true, // RISC-V with A extension (assumed)
        CpuArch::Unknown => false,
    }
}

/// Detect memory ordering instruction support.
fn detect_memory_ordering_support(arch: CpuArch) -> bool {
    match arch {
        CpuArch::X86_64 => true, // Has mfence, lfence, sfence
        CpuArch::Aarch64 => true, // Has dsb, dmb, isb
        CpuArch::RiscV64 => true, // Has fence
        CpuArch::Unknown => false,
    }
}

// x86_64-specific detection functions
#[cfg(feature = "x86_64")]
fn detect_x86_64_cache_line_size() -> u32 {
    // Could use CPUID to detect, for now return common size
    64
}

#[cfg(feature = "x86_64")]
fn detect_x86_64_sse() -> bool {
    // Would use CPUID instruction to check SSE support
    // For now, assume true on x86_64
    true
}

#[cfg(feature = "x86_64")]
fn detect_x86_64_avx() -> bool {
    // Would use CPUID to check AVX support
    // For now, return false as not all systems have AVX
    false
}

#[cfg(feature = "x86_64")]
fn detect_x86_64_avx512() -> bool {
    // Would use CPUID to check AVX-512 support
    // For now, return false as AVX-512 is less common
    false
}

// ARM64-specific detection functions
#[cfg(feature = "arm64")]
fn detect_arm64_cache_line_size() -> u32 {
    // Could read from system registers
    64
}

#[cfg(not(feature = "arm64"))]
fn detect_arm64_cache_line_size() -> u32 {
    64
}

#[cfg(feature = "arm64")]
fn detect_arm64_neon() -> bool {
    // ARM64 always has NEON
    true
}

#[cfg(feature = "arm64")]
fn detect_arm64_sve() -> bool {
    // Would check ID_AA64PFR0_EL1 register for SVE support
    // For now, return false as not all ARM64 systems have SVE
    false
}

#[cfg(feature = "arm64")]
fn detect_arm64_sve2() -> bool {
    // Would check for SVE2 support in system registers
    false
}

// RISC-V-specific detection functions
#[cfg(feature = "riscv64")]
fn detect_riscv_cache_line_size() -> u32 {
    // RISC-V cache line size varies by implementation
    64 // Common default
}

#[cfg(not(feature = "riscv64"))]
fn detect_riscv_cache_line_size() -> u32 {
    64
}

#[cfg(feature = "riscv64")]
fn detect_riscv_f_extension() -> bool {
    // Would check misa CSR for F extension
    // For now, assume based on feature flags
    cfg!(feature = "riscv-float")
}

#[cfg(not(feature = "riscv64"))]
fn detect_riscv_f_extension() -> bool {
    false
}

#[cfg(feature = "riscv64")]
fn detect_riscv_d_extension() -> bool {
    // Would check misa CSR for D extension
    cfg!(feature = "riscv-float")
}

#[cfg(feature = "riscv64")]
fn detect_riscv_v_extension() -> bool {
    // Would check for V extension support
    cfg!(feature = "riscv-vector")
}

/// Runtime optimization controller.
pub struct RuntimeOptimizer {
    features: CpuFeatures,
}

impl RuntimeOptimizer {
    /// Create a new runtime optimizer with detected CPU features.
    pub fn new() -> Self {
        Self {
            features: detect_cpu_features(),
        }
    }
    
    /// Get the detected CPU features.
    pub fn features(&self) -> &CpuFeatures {
        &self.features
    }
    
    /// Choose optimal memory barrier implementation.
    pub fn optimal_memory_barrier(&self) -> fn() {
        match self.features.arch {
            CpuArch::X86_64 => {
                #[cfg(feature = "x86_64")]
                return crate::arch::x86_64::memory_barrier_full;
                #[cfg(not(feature = "x86_64"))]
                return || core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
            CpuArch::Aarch64 => {
                #[cfg(feature = "arm64")]
                return crate::arch::aarch64::memory_barrier_full;
                #[cfg(not(feature = "arm64"))]
                return || core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
            CpuArch::RiscV64 => {
                #[cfg(feature = "riscv64")]
                return crate::arch::riscv::memory_barrier_full;
                #[cfg(not(feature = "riscv64"))]
                return || core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
            CpuArch::Unknown => || core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst),
        }
    }
    
    /// Get optimal cache line size for alignment.
    pub fn optimal_cache_line_size(&self) -> usize {
        self.features.cache_line_size as usize
    }
    
    /// Determine if lock-free algorithms should be preferred.
    pub fn prefer_lock_free(&self) -> bool {
        self.features.supports_atomic_cas && self.features.supports_memory_ordering
    }
    
    /// Get recommended number of worker threads.
    pub fn recommended_worker_threads(&self) -> usize {
        (self.features.cpu_cores as usize).max(1)
    }
}

impl Default for RuntimeOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Global runtime optimizer instance.
static GLOBAL_OPTIMIZER: spin::Mutex<Option<RuntimeOptimizer>> = spin::Mutex::new(None);

/// Get the global runtime optimizer instance.
pub fn global_optimizer() -> RuntimeOptimizer {
    let mut guard = GLOBAL_OPTIMIZER.lock();
    if let Some(optimizer) = guard.as_ref() {
        RuntimeOptimizer {
            features: optimizer.features,
        }
    } else {
        let optimizer = RuntimeOptimizer::new();
        *guard = Some(RuntimeOptimizer {
            features: optimizer.features,
        });
        optimizer
    }
}
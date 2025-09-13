//! Context switch optimizations with micro-benchmarking support.

use crate::perf::PERF_COUNTERS;
use crate::arch::Arch;
use crate::time::{get_monotonic_time, Duration};
use portable_atomic::{AtomicU64, AtomicU32, Ordering};

/// Context switch optimization techniques.
pub struct ContextSwitchOptimizer<A: Arch> {
    /// Architecture-specific optimizations
    arch_optimizations: A,
    
    /// Performance measurements
    switch_times: SwitchTimingData,
    
    /// Optimization configuration
    config: OptimizationConfig,
}

/// Context switch timing measurements.
#[repr(align(64))] // Cache line aligned
pub struct SwitchTimingData {
    /// Total context switches performed
    pub total_switches: AtomicU64,
    
    /// Cumulative switch time in nanoseconds
    pub total_switch_time_ns: AtomicU64,
    
    /// Fastest recorded switch time
    pub fastest_switch_ns: AtomicU32,
    
    /// Slowest recorded switch time
    pub slowest_switch_ns: AtomicU32,
    
    /// Number of timing measurements
    pub measurement_count: AtomicU64,
    
    /// Recent switch times (circular buffer)
    pub recent_times: [AtomicU32; 64],
    pub recent_index: AtomicU32,
}

impl Default for SwitchTimingData {
    fn default() -> Self {
        Self {
            total_switches: AtomicU64::new(0),
            total_switch_time_ns: AtomicU64::new(0),
            fastest_switch_ns: AtomicU32::new(u32::MAX),
            slowest_switch_ns: AtomicU32::new(0),
            measurement_count: AtomicU64::new(0),
            recent_times: core::array::from_fn(|_| AtomicU32::new(0)),
            recent_index: AtomicU32::new(0),
        }
    }
}

/// Configuration for context switch optimizations.
#[derive(Debug, Clone, Copy)]
pub struct OptimizationConfig {
    /// Whether to measure switch timing
    pub enable_timing: bool,
    
    /// Whether to use optimized assembly sequences
    pub use_optimized_assembly: bool,
    
    /// Whether to prefetch next thread's stack
    pub enable_stack_prefetch: bool,
    
    /// Whether to minimize register saves
    pub minimize_register_saves: bool,
    
    /// Target switch time in nanoseconds
    pub target_switch_time_ns: u32,
    
    /// Number of warmup switches before optimization
    pub warmup_switches: u32,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_timing: true,
            use_optimized_assembly: true,
            enable_stack_prefetch: true,
            minimize_register_saves: true,
            target_switch_time_ns: 1000, // 1 microsecond target
            warmup_switches: 1000,
        }
    }
}

impl<A: Arch> ContextSwitchOptimizer<A> {
    pub fn new(arch: A, config: OptimizationConfig) -> Self {
        Self {
            arch_optimizations: arch,
            switch_times: SwitchTimingData::default(),
            config,
        }
    }
    
    /// Optimized context switch with timing measurement.
    pub unsafe fn optimized_context_switch(
        &self,
        prev_context: *mut A::SavedContext,
        next_context: *const A::SavedContext,
    ) {
        let start_time = if self.config.enable_timing {
            Some(get_monotonic_time())
        } else {
            None
        };
        
        // Prefetch next thread's stack if enabled
        if self.config.enable_stack_prefetch {
            unsafe { self.prefetch_stack(next_context); }
        }
        
        // Record switch
        self.switch_times.total_switches.fetch_add(1, Ordering::Relaxed);
        PERF_COUNTERS.record_context_switch();
        
        // Perform the actual context switch
        unsafe {
            if self.config.use_optimized_assembly {
                self.optimized_arch_switch(prev_context, next_context);
            } else {
                unsafe { A::context_switch(prev_context, next_context); }
            }
        }
        
        // Record timing if enabled
        if let Some(start) = start_time {
            let elapsed = get_monotonic_time().duration_since(start);
            self.record_switch_time(elapsed);
        }
    }
    
    /// Architecture-specific optimized context switch.
    unsafe fn optimized_arch_switch(
        &self,
        prev_context: *mut A::SavedContext,
        next_context: *const A::SavedContext,
    ) {
        // Use unsafe blocks and delegate to generic architecture-specific switch
        unsafe {
            unsafe { A::context_switch(prev_context, next_context); }
        }
    }
    
    /// Optimized x86_64 context switch with minimal register saves.
    #[cfg(feature = "x86_64")]
    unsafe fn optimized_x86_64_switch(
        &self,
        prev_context: *mut A::SavedContext,
        next_context: *const A::SavedContext,
    ) {
        if self.config.minimize_register_saves {
            // Use minimal register set for fast switching
            unsafe { core::arch::asm!(
                // Save only essential registers
                "pushq %rbx",
                "pushq %rbp", 
                "pushq %r12",
                "pushq %r13",
                "pushq %r14",
                "pushq %r15",
                
                // Save stack pointer
                "movq %rsp, ({prev})",
                
                // Load new stack pointer
                "movq ({next}), %rsp",
                
                // Restore registers
                "popq %r15",
                "popq %r14",
                "popq %r13",
                "popq %r12",
                "popq %rbp",
                "popq %rbx",
                
                prev = in(reg) prev_context,
                next = in(reg) next_context,
                clobber_abi("C")
            ); }
        } else {
            // Fallback to full context switch
            unsafe { A::context_switch(prev_context, next_context); }
        }
    }
    
    /// Optimized ARM64 context switch.
    #[cfg(feature = "arm64")]
    unsafe fn optimized_arm64_switch(
        &self,
        prev_context: *mut A::SavedContext,
        next_context: *const A::SavedContext,
    ) {
        if self.config.minimize_register_saves {
            // ARM64-specific optimizations would go here
            // For now, fallback to standard implementation
            unsafe { A::context_switch(prev_context, next_context); }
        } else {
            unsafe { A::context_switch(prev_context, next_context); }
        }
    }
    
    /// Optimized RISC-V context switch.
    #[cfg(feature = "riscv64")]
    unsafe fn optimized_riscv_switch(
        &self,
        prev_context: *mut A::SavedContext,
        next_context: *const A::SavedContext,
    ) {
        if self.config.minimize_register_saves {
            // RISC-V-specific optimizations would go here
            // For now, fallback to standard implementation
            unsafe { A::context_switch(prev_context, next_context); }
        } else {
            unsafe { A::context_switch(prev_context, next_context); }
        }
    }
    
    /// Prefetch stack data for next thread.
    unsafe fn prefetch_stack(&self, next_context: *const A::SavedContext) {
        // Prefetch stack pages to reduce cache misses
        // This is architecture-specific and would read stack pointer from context
        
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        {
            // Generic prefetch - in real implementation would extract stack pointer
            // from context and prefetch cache lines
            let stack_ptr = next_context as *const u8;
            for i in 0..8 {
                unsafe {
                    let prefetch_addr = stack_ptr.offset(i * 64); // 64-byte cache lines
                    unsafe { core::arch::asm!(
                        "prefetcht0 ({addr})",
                        addr = in(reg) prefetch_addr,
                        options(nostack, readonly)
                    ); }
                }
            }
        }
    }
    
    /// Record context switch timing.
    fn record_switch_time(&self, elapsed: Duration) {
        let elapsed_ns = elapsed.as_nanos() as u64;
        let elapsed_u32 = elapsed_ns.min(u32::MAX as u64) as u32;
        
        // Update cumulative statistics
        self.switch_times.total_switch_time_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
        self.switch_times.measurement_count.fetch_add(1, Ordering::Relaxed);
        
        // Update fastest time
        let mut current_fastest = self.switch_times.fastest_switch_ns.load(Ordering::Relaxed);
        while elapsed_u32 < current_fastest {
            match self.switch_times.fastest_switch_ns.compare_exchange_weak(
                current_fastest,
                elapsed_u32,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_fastest = actual,
            }
        }
        
        // Update slowest time
        let mut current_slowest = self.switch_times.slowest_switch_ns.load(Ordering::Relaxed);
        while elapsed_u32 > current_slowest {
            match self.switch_times.slowest_switch_ns.compare_exchange_weak(
                current_slowest,
                elapsed_u32,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_slowest = actual,
            }
        }
        
        // Add to circular buffer
        let index = self.switch_times.recent_index.fetch_add(1, Ordering::Relaxed) as usize % 64;
        self.switch_times.recent_times[index].store(elapsed_u32, Ordering::Relaxed);
    }
    
    /// Get context switch performance statistics.
    pub fn get_switch_stats(&self) -> ContextSwitchStats {
        let total_switches = self.switch_times.total_switches.load(Ordering::Relaxed);
        let total_time = self.switch_times.total_switch_time_ns.load(Ordering::Relaxed);
        let measurement_count = self.switch_times.measurement_count.load(Ordering::Relaxed);
        
        let average_ns = if measurement_count > 0 {
            (total_time / measurement_count) as u32
        } else {
            0
        };
        
        ContextSwitchStats {
            total_switches,
            total_switch_time_ns: total_time,
            average_switch_time_ns: average_ns,
            fastest_switch_ns: self.switch_times.fastest_switch_ns.load(Ordering::Relaxed),
            slowest_switch_ns: self.switch_times.slowest_switch_ns.load(Ordering::Relaxed),
            measurement_count,
            meets_target: average_ns <= self.config.target_switch_time_ns,
            optimization_enabled: self.config.use_optimized_assembly,
        }
    }
    
    /// Run micro-benchmark to optimize context switch parameters.
    pub unsafe fn run_optimization_benchmark(&mut self) -> OptimizationResults {
        let warmup_switches = self.config.warmup_switches;
        let test_iterations = 10000;
        
        // Create dummy contexts for benchmarking
        let mut prev_context = core::mem::MaybeUninit::<A::SavedContext>::uninit();
        let mut next_context = core::mem::MaybeUninit::<A::SavedContext>::uninit();
        
        // Initialize contexts with safe default values
        prev_context.write(unsafe { core::mem::zeroed() });
        next_context.write(unsafe { core::mem::zeroed() });
        
        let prev_ptr = prev_context.as_mut_ptr();
        let next_ptr = next_context.as_ptr();
        
        // Warmup phase
        for _ in 0..warmup_switches {
            unsafe {
                A::context_switch(prev_ptr, next_ptr);
            }
        }
        
        // Benchmark standard implementation
        let start_time = get_monotonic_time();
        for _ in 0..test_iterations {
            unsafe {
                A::context_switch(prev_ptr, next_ptr);
            }
        }
        let standard_time = get_monotonic_time().duration_since(start_time);
        
        // Benchmark optimized implementation
        let optimized_config = OptimizationConfig {
            use_optimized_assembly: true,
            minimize_register_saves: true,
            ..self.config
        };
        
        let old_config = self.config;
        self.config = optimized_config;
        
        let start_time = get_monotonic_time();
        for _ in 0..test_iterations {
            unsafe {
                self.optimized_arch_switch(prev_ptr, next_ptr);
            }
        }
        let optimized_time = get_monotonic_time().duration_since(start_time);
        
        self.config = old_config;
        
        let standard_avg_ns = (standard_time.as_nanos_u128() / test_iterations as u128) as u32;
        let optimized_avg_ns = (optimized_time.as_nanos_u128() / test_iterations as u128) as u32;
        
        OptimizationResults {
            standard_avg_ns,
            optimized_avg_ns,
            improvement_ratio: if optimized_avg_ns > 0 {
                standard_avg_ns as f64 / optimized_avg_ns as f64
            } else {
                1.0
            },
            meets_target: optimized_avg_ns <= self.config.target_switch_time_ns,
            recommended_config: if optimized_avg_ns < standard_avg_ns {
                optimized_config
            } else {
                old_config
            },
        }
    }
    
    /// Apply recommended optimizations based on benchmark results.
    pub fn apply_optimizations(&mut self, results: &OptimizationResults) {
        if results.improvement_ratio > 1.1 { // At least 10% improvement
            self.config = results.recommended_config;
            // Applied context switch optimizations with improvement ratio
        }
    }
}

/// Context switch performance statistics.
#[derive(Debug, Clone)]
pub struct ContextSwitchStats {
    pub total_switches: u64,
    pub total_switch_time_ns: u64,
    pub average_switch_time_ns: u32,
    pub fastest_switch_ns: u32,
    pub slowest_switch_ns: u32,
    pub measurement_count: u64,
    pub meets_target: bool,
    pub optimization_enabled: bool,
}

/// Results from context switch optimization benchmark.
#[derive(Debug, Clone)]
pub struct OptimizationResults {
    pub standard_avg_ns: u32,
    pub optimized_avg_ns: u32,
    pub improvement_ratio: f64,
    pub meets_target: bool,
    pub recommended_config: OptimizationConfig,
}

/// Global context switch optimizer (would be initialized per architecture).
pub static mut CONTEXT_SWITCH_OPTIMIZER: Option<ContextSwitchOptimizer<crate::arch::DefaultArch>> = None;

/// Initialize context switch optimization.
pub fn init_context_switch_optimization() {
    let config = OptimizationConfig::default();
    let arch = crate::arch::DefaultArch;
    
    unsafe {
        CONTEXT_SWITCH_OPTIMIZER = Some(ContextSwitchOptimizer::new(arch, config));
        
        // Run initial benchmark to determine optimal settings
        if let Some(optimizer) = &mut CONTEXT_SWITCH_OPTIMIZER {
            let results = optimizer.run_optimization_benchmark();
            optimizer.apply_optimizations(&results);
            
            // Context switch optimization initialized with optimized average and target time
        }
    }
}

/// Get current context switch statistics.
pub fn get_context_switch_stats() -> Option<ContextSwitchStats> {
    unsafe {
        CONTEXT_SWITCH_OPTIMIZER.as_ref().map(|opt| opt.get_switch_stats())
    }
}
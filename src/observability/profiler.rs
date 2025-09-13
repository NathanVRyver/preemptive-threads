//! Performance profiling and analysis for threading system.
//!
//! This module provides comprehensive performance profiling including
//! CPU usage, memory allocation patterns, context switching costs,
//! and scheduler efficiency analysis.

use portable_atomic::{AtomicU64, AtomicU32, AtomicUsize, AtomicBool, Ordering};
use crate::time::{Duration, Instant};
use crate::thread_new::ThreadId;
extern crate alloc;
use alloc::{vec::Vec, collections::BTreeMap};
use spin::Mutex;

/// Configuration for the profiler.
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    /// Maximum number of samples to keep in memory
    pub max_samples: usize,
    /// Enable statistical sampling
    pub sampling_enabled: bool,
    /// Sampling interval in microseconds
    pub sampling_interval_us: u64,
    /// Enable call stack tracking
    pub stack_tracing_enabled: bool,
    /// Maximum call stack depth to track
    pub max_stack_depth: usize,
    /// Enable memory allocation tracking
    pub memory_tracking_enabled: bool,
    /// Enable scheduler event tracking
    pub scheduler_tracking_enabled: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            max_samples: 10000,
            sampling_enabled: true,
            sampling_interval_us: 1000, // 1ms sampling
            stack_tracing_enabled: cfg!(debug_assertions),
            max_stack_depth: 32,
            memory_tracking_enabled: true,
            scheduler_tracking_enabled: true,
        }
    }
}

/// A single profiling sample.
#[derive(Debug, Clone)]
pub struct ProfileSample {
    /// Thread that generated this sample
    pub thread_id: ThreadId,
    /// Sample timestamp
    pub timestamp: Instant,
    /// Sample type and data
    pub sample_type: SampleType,
    /// CPU usage at time of sample (percentage)
    pub cpu_usage: f32,
    /// Memory usage at time of sample (bytes)
    pub memory_usage: u64,
    /// Call stack if available
    pub call_stack: Option<CallStack>,
}

/// Types of profiling samples.
#[derive(Debug, Clone)]
pub enum SampleType {
    /// CPU execution sample
    CpuSample {
        /// Instruction pointer
        instruction_pointer: u64,
        /// CPU time since last sample (nanoseconds)
        cpu_time_delta: u64,
    },
    /// Memory allocation sample
    MemoryAllocation {
        /// Size of allocation
        size: u64,
        /// Allocation type
        allocation_type: AllocationType,
    },
    /// Memory deallocation sample
    MemoryDeallocation {
        /// Size of deallocation
        size: u64,
    },
    /// Context switch sample
    ContextSwitch {
        /// Previous thread ID
        from_thread: ThreadId,
        /// Next thread ID
        to_thread: ThreadId,
        /// Switch latency (nanoseconds)
        switch_latency_ns: u64,
        /// Switch reason
        switch_reason: ContextSwitchReason,
    },
    /// Scheduler decision sample
    SchedulerDecision {
        /// Number of threads in ready queue
        ready_queue_length: usize,
        /// Selected thread priority
        selected_priority: u8,
        /// Decision latency (nanoseconds)
        decision_latency_ns: u64,
    },
    /// Lock contention sample
    LockContention {
        /// Lock address
        lock_address: u64,
        /// Time spent waiting (nanoseconds)
        wait_time_ns: u64,
        /// Number of waiters
        waiter_count: u32,
    },
}

/// Types of memory allocations for profiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationType {
    Stack,
    Heap,
    StaticData,
    ThreadLocal,
    Other,
}

/// Reasons for context switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextSwitchReason {
    /// Time slice expired
    TimeSliceExpired,
    /// Thread voluntarily yielded
    VoluntaryYield,
    /// Thread blocked on I/O
    IOBlock,
    /// Thread blocked on synchronization primitive
    SyncBlock,
    /// Thread terminated
    ThreadExit,
    /// Priority preemption
    PriorityPreemption,
    /// Load balancing
    LoadBalance,
}

/// Call stack for profiling.
#[derive(Debug, Clone)]
pub struct CallStack {
    /// Stack frames (instruction pointers)
    pub frames: Vec<u64>,
    /// Total stack depth (may be larger than frames if truncated)
    pub total_depth: usize,
}

impl CallStack {
    /// Create a new call stack.
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            total_depth: 0,
        }
    }
    
    /// Capture current call stack (placeholder implementation).
    pub fn capture(max_depth: usize) -> Self {
        // In a real implementation, this would use platform-specific
        // stack walking to capture actual instruction pointers
        Self {
            frames: Vec::new(), // Placeholder
            total_depth: 0,
        }
    }
}

/// Aggregated profiling data for analysis.
#[derive(Debug, Clone)]
pub struct ProfileData {
    /// Total samples collected
    pub total_samples: u64,
    /// Samples by thread
    pub thread_samples: BTreeMap<ThreadId, ThreadProfileData>,
    /// Hot functions/addresses
    pub hot_spots: Vec<HotSpot>,
    /// Memory allocation patterns
    pub memory_patterns: MemoryProfile,
    /// Context switch analysis
    pub context_switch_analysis: ContextSwitchProfile,
    /// Scheduler performance metrics
    pub scheduler_metrics: SchedulerProfile,
    /// Profiling time range
    pub time_range: (Instant, Instant),
}

/// Per-thread profiling data.
#[derive(Debug, Clone)]
pub struct ThreadProfileData {
    /// Thread ID
    pub thread_id: ThreadId,
    /// Number of samples for this thread
    pub sample_count: u64,
    /// Total CPU time observed (nanoseconds)
    pub total_cpu_time: u64,
    /// Peak memory usage
    pub peak_memory_usage: u64,
    /// Average CPU usage percentage
    pub avg_cpu_usage: f32,
    /// Context switches initiated by this thread
    pub context_switches: u64,
    /// Top functions by CPU time
    pub top_functions: Vec<FunctionProfile>,
}

/// Hot spot in CPU usage.
#[derive(Debug, Clone)]
pub struct HotSpot {
    /// Instruction pointer or function address
    pub address: u64,
    /// Number of samples at this address
    pub sample_count: u64,
    /// Percentage of total samples
    pub sample_percentage: f32,
    /// Associated call stack patterns
    pub call_patterns: Vec<CallStack>,
}

/// Function profiling data.
#[derive(Debug, Clone)]
pub struct FunctionProfile {
    /// Function address
    pub address: u64,
    /// Number of samples in this function
    pub sample_count: u64,
    /// Estimated CPU time in this function
    pub cpu_time_ns: u64,
    /// Call frequency
    pub call_count: u64,
}

/// Memory allocation profiling.
#[derive(Debug, Clone)]
pub struct MemoryProfile {
    /// Total allocations observed
    pub total_allocations: u64,
    /// Total deallocations observed
    pub total_deallocations: u64,
    /// Bytes allocated
    pub bytes_allocated: u64,
    /// Bytes deallocated
    pub bytes_deallocated: u64,
    /// Allocation patterns by type
    pub allocation_patterns: BTreeMap<AllocationType, AllocationPattern>,
    /// Peak memory usage observed
    pub peak_memory_usage: u64,
    /// Memory fragmentation estimate
    pub fragmentation_estimate: f32,
}

/// Allocation pattern for a specific type.
#[derive(Debug, Clone)]
pub struct AllocationPattern {
    /// Number of allocations
    pub allocation_count: u64,
    /// Average allocation size
    pub avg_size: u64,
    /// Largest allocation
    pub max_size: u64,
    /// Smallest allocation
    pub min_size: u64,
    /// Standard deviation of sizes
    pub size_stddev: f64,
}

/// Context switching profiling data.
#[derive(Debug, Clone)]
pub struct ContextSwitchProfile {
    /// Total context switches observed
    pub total_switches: u64,
    /// Average switch latency (nanoseconds)
    pub avg_switch_latency: u64,
    /// Maximum switch latency (nanoseconds)
    pub max_switch_latency: u64,
    /// Minimum switch latency (nanoseconds)
    pub min_switch_latency: u64,
    /// Switches by reason
    pub switches_by_reason: BTreeMap<ContextSwitchReason, u64>,
    /// Switches per second
    pub switches_per_second: f64,
}

/// Scheduler profiling data.
#[derive(Debug, Clone)]
pub struct SchedulerProfile {
    /// Total scheduler decisions
    pub total_decisions: u64,
    /// Average decision latency (nanoseconds)
    pub avg_decision_latency: u64,
    /// Maximum decision latency (nanoseconds)
    pub max_decision_latency: u64,
    /// Average ready queue length
    pub avg_ready_queue_length: f32,
    /// Maximum ready queue length observed
    pub max_ready_queue_length: usize,
    /// Load balancing efficiency
    pub load_balance_efficiency: f32,
}

/// Thread profiler implementation.
pub struct ThreadProfiler {
    /// Profiler configuration
    config: Mutex<ProfilerConfig>,
    /// Sample storage
    samples: Mutex<Vec<ProfileSample>>,
    /// Profiler enabled flag
    enabled: AtomicBool,
    /// Total samples collected
    total_samples: AtomicU64,
    /// Sample collection start time
    start_time: Mutex<Option<Instant>>,
    /// Per-thread statistics
    thread_stats: Mutex<BTreeMap<ThreadId, ThreadProfileData>>,
    /// Sampling interval counter
    sample_counter: AtomicU64,
}

impl ThreadProfiler {
    /// Create a new thread profiler.
    pub const fn new() -> Self {
        Self {
            config: Mutex::new(ProfilerConfig {
                max_samples: 10000,
                sampling_enabled: true,
                sampling_interval_us: 1000,
                stack_tracing_enabled: false,
                max_stack_depth: 32,
                memory_tracking_enabled: true,
                scheduler_tracking_enabled: true,
            }),
            samples: Mutex::new(Vec::new()),
            enabled: AtomicBool::new(false),
            total_samples: AtomicU64::new(0),
            start_time: Mutex::new(None),
            thread_stats: Mutex::new(BTreeMap::new()),
            sample_counter: AtomicU64::new(0),
        }
    }
    
    /// Initialize the profiler with configuration.
    pub fn init(&self, config: ProfilerConfig) -> Result<(), &'static str> {
        if let Some(mut profiler_config) = self.config.try_lock() {
            *profiler_config = config;
        } else {
            return Err("Failed to lock profiler config");
        }
        
        if let Some(mut start_time) = self.start_time.try_lock() {
            *start_time = Some(Instant::now());
        }
        
        self.enabled.store(true, Ordering::Release);
        Ok(())
    }
    
    /// Check if profiling is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    
    /// Record a profiling sample.
    pub fn record_sample(&self, sample: ProfileSample) {
        if !self.is_enabled() {
            return;
        }
        
        // Check sampling rate
        let config = if let Some(config) = self.config.try_lock() {
            config.clone()
        } else {
            return;
        };
        
        if config.sampling_enabled {
            let counter = self.sample_counter.fetch_add(1, Ordering::AcqRel);
            // Simple sampling: only collect every Nth sample based on interval
            let sampling_rate = config.sampling_interval_us.max(1);
            if counter % sampling_rate != 0 {
                return;
            }
        }
        
        // Add sample to storage
        if let Some(mut samples) = self.samples.try_lock() {
            samples.push(sample.clone());
            
            // Trim samples if over limit
            let samples_len = samples.len();
            if samples_len > config.max_samples {
                samples.drain(0..samples_len - config.max_samples);
            }
        }
        
        // Update per-thread statistics
        self.update_thread_stats(&sample);
        
        self.total_samples.fetch_add(1, Ordering::AcqRel);
    }
    
    /// Record a CPU sample for a thread.
    pub fn record_cpu_sample(&self, thread_id: ThreadId, instruction_pointer: u64, cpu_time_delta: u64) {
        let sample = ProfileSample {
            thread_id,
            timestamp: Instant::now(),
            sample_type: SampleType::CpuSample {
                instruction_pointer,
                cpu_time_delta,
            },
            cpu_usage: 0.0, // Will be calculated during analysis
            memory_usage: 0, // Not available for CPU samples
            call_stack: None, // TODO: Capture if enabled
        };
        
        self.record_sample(sample);
    }
    
    /// Record a memory allocation.
    pub fn record_allocation(&self, thread_id: ThreadId, size: u64, allocation_type: AllocationType) {
        let sample = ProfileSample {
            thread_id,
            timestamp: Instant::now(),
            sample_type: SampleType::MemoryAllocation {
                size,
                allocation_type,
            },
            cpu_usage: 0.0,
            memory_usage: 0, // TODO: Get current memory usage
            call_stack: None, // TODO: Capture if enabled
        };
        
        self.record_sample(sample);
    }
    
    /// Record a context switch.
    pub fn record_context_switch(
        &self,
        from_thread: ThreadId,
        to_thread: ThreadId,
        switch_latency_ns: u64,
        reason: ContextSwitchReason,
    ) {
        let sample = ProfileSample {
            thread_id: from_thread, // Associate with the thread being switched out
            timestamp: Instant::now(),
            sample_type: SampleType::ContextSwitch {
                from_thread,
                to_thread,
                switch_latency_ns,
                switch_reason: reason,
            },
            cpu_usage: 0.0,
            memory_usage: 0,
            call_stack: None,
        };
        
        self.record_sample(sample);
    }
    
    /// Update per-thread statistics.
    fn update_thread_stats(&self, sample: &ProfileSample) {
        if let Some(mut stats) = self.thread_stats.try_lock() {
            let thread_stats = stats.entry(sample.thread_id)
                .or_insert_with(|| ThreadProfileData {
                    thread_id: sample.thread_id,
                    sample_count: 0,
                    total_cpu_time: 0,
                    peak_memory_usage: 0,
                    avg_cpu_usage: 0.0,
                    context_switches: 0,
                    top_functions: Vec::new(),
                });
            
            thread_stats.sample_count += 1;
            
            match &sample.sample_type {
                SampleType::CpuSample { cpu_time_delta, .. } => {
                    thread_stats.total_cpu_time += cpu_time_delta;
                },
                SampleType::ContextSwitch { .. } => {
                    thread_stats.context_switches += 1;
                },
                _ => {}
            }
            
            if sample.memory_usage > thread_stats.peak_memory_usage {
                thread_stats.peak_memory_usage = sample.memory_usage;
            }
        }
    }
    
    /// Generate comprehensive profile analysis.
    pub fn analyze_profile(&self) -> ProfileData {
        let samples = if let Some(samples) = self.samples.try_lock() {
            samples.clone()
        } else {
            Vec::new()
        };
        
        let thread_stats = if let Some(stats) = self.thread_stats.try_lock() {
            stats.clone()
        } else {
            BTreeMap::new()
        };
        
        let start_time = if let Some(start) = self.start_time.try_lock() {
            start.unwrap_or_else(|| Instant::now())
        } else {
            Instant::now()
        };
        
        let end_time = Instant::now();
        let total_samples = self.total_samples.load(Ordering::Acquire);
        
        // Analyze hot spots
        let hot_spots = self.analyze_hot_spots(&samples);
        
        // Analyze memory patterns
        let memory_patterns = self.analyze_memory_patterns(&samples);
        
        // Analyze context switches
        let context_switch_analysis = self.analyze_context_switches(&samples);
        
        // Analyze scheduler performance
        let scheduler_metrics = self.analyze_scheduler_performance(&samples);
        
        ProfileData {
            total_samples,
            thread_samples: thread_stats,
            hot_spots,
            memory_patterns,
            context_switch_analysis,
            scheduler_metrics,
            time_range: (start_time, end_time),
        }
    }
    
    /// Analyze CPU hot spots.
    fn analyze_hot_spots(&self, samples: &[ProfileSample]) -> Vec<HotSpot> {
        let mut address_counts: BTreeMap<u64, u64> = BTreeMap::new();
        let mut total_cpu_samples = 0u64;
        
        for sample in samples {
            if let SampleType::CpuSample { instruction_pointer, .. } = sample.sample_type {
                *address_counts.entry(instruction_pointer).or_insert(0) += 1;
                total_cpu_samples += 1;
            }
        }
        
        let mut hot_spots: Vec<HotSpot> = address_counts
            .into_iter()
            .map(|(address, count)| {
                let percentage = if total_cpu_samples > 0 {
                    (count as f32 / total_cpu_samples as f32) * 100.0
                } else {
                    0.0
                };
                
                HotSpot {
                    address,
                    sample_count: count,
                    sample_percentage: percentage,
                    call_patterns: Vec::new(), // TODO: Implement call pattern analysis
                }
            })
            .collect();
        
        // Sort by sample count, descending
        hot_spots.sort_by(|a, b| b.sample_count.cmp(&a.sample_count));
        
        // Take top 20 hot spots
        hot_spots.truncate(20);
        hot_spots
    }
    
    /// Analyze memory allocation patterns.
    fn analyze_memory_patterns(&self, samples: &[ProfileSample]) -> MemoryProfile {
        let mut total_allocations = 0u64;
        let mut total_deallocations = 0u64;
        let mut bytes_allocated = 0u64;
        let mut bytes_deallocated = 0u64;
        let mut peak_memory = 0u64;
        let mut current_memory = 0u64;
        let mut allocation_patterns: BTreeMap<AllocationType, AllocationPattern> = BTreeMap::new();
        
        for sample in samples {
            match &sample.sample_type {
                SampleType::MemoryAllocation { size, allocation_type } => {
                    total_allocations += 1;
                    bytes_allocated += size;
                    current_memory += size;
                    
                    if current_memory > peak_memory {
                        peak_memory = current_memory;
                    }
                    
                    // Update allocation pattern
                    let pattern = allocation_patterns.entry(*allocation_type)
                        .or_insert(AllocationPattern {
                            allocation_count: 0,
                            avg_size: 0,
                            max_size: 0,
                            min_size: u64::MAX,
                            size_stddev: 0.0,
                        });
                    
                    pattern.allocation_count += 1;
                    pattern.avg_size = bytes_allocated / total_allocations;
                    if *size > pattern.max_size {
                        pattern.max_size = *size;
                    }
                    if *size < pattern.min_size {
                        pattern.min_size = *size;
                    }
                },
                SampleType::MemoryDeallocation { size } => {
                    total_deallocations += 1;
                    bytes_deallocated += size;
                    if current_memory >= *size {
                        current_memory -= size;
                    }
                },
                _ => {}
            }
        }
        
        // Simple fragmentation estimate
        let fragmentation_estimate = if bytes_allocated > 0 {
            ((bytes_allocated - bytes_deallocated) as f32 / bytes_allocated as f32) * 100.0
        } else {
            0.0
        };
        
        MemoryProfile {
            total_allocations,
            total_deallocations,
            bytes_allocated,
            bytes_deallocated,
            allocation_patterns,
            peak_memory_usage: peak_memory,
            fragmentation_estimate,
        }
    }
    
    /// Analyze context switch performance.
    fn analyze_context_switches(&self, samples: &[ProfileSample]) -> ContextSwitchProfile {
        let mut total_switches = 0u64;
        let mut total_latency = 0u64;
        let mut max_latency = 0u64;
        let mut min_latency = u64::MAX;
        let mut switches_by_reason: BTreeMap<ContextSwitchReason, u64> = BTreeMap::new();
        
        for sample in samples {
            if let SampleType::ContextSwitch { switch_latency_ns, switch_reason, .. } = sample.sample_type {
                total_switches += 1;
                total_latency += switch_latency_ns;
                
                if switch_latency_ns > max_latency {
                    max_latency = switch_latency_ns;
                }
                if switch_latency_ns < min_latency {
                    min_latency = switch_latency_ns;
                }
                
                *switches_by_reason.entry(switch_reason).or_insert(0) += 1;
            }
        }
        
        let avg_switch_latency = if total_switches > 0 {
            total_latency / total_switches
        } else {
            0
        };
        
        if min_latency == u64::MAX {
            min_latency = 0;
        }
        
        ContextSwitchProfile {
            total_switches,
            avg_switch_latency,
            max_switch_latency: max_latency,
            min_switch_latency: min_latency,
            switches_by_reason,
            switches_per_second: 0.0, // TODO: Calculate based on time range
        }
    }
    
    /// Analyze scheduler performance.
    fn analyze_scheduler_performance(&self, samples: &[ProfileSample]) -> SchedulerProfile {
        let mut total_decisions = 0u64;
        let mut total_decision_latency = 0u64;
        let mut max_decision_latency = 0u64;
        let mut total_queue_length = 0u64;
        let mut max_queue_length = 0usize;
        
        for sample in samples {
            if let SampleType::SchedulerDecision { 
                ready_queue_length, 
                decision_latency_ns, 
                .. 
            } = sample.sample_type {
                total_decisions += 1;
                total_decision_latency += decision_latency_ns;
                
                if decision_latency_ns > max_decision_latency {
                    max_decision_latency = decision_latency_ns;
                }
                
                total_queue_length += ready_queue_length as u64;
                
                if ready_queue_length > max_queue_length {
                    max_queue_length = ready_queue_length;
                }
            }
        }
        
        let avg_decision_latency = if total_decisions > 0 {
            total_decision_latency / total_decisions
        } else {
            0
        };
        
        let avg_ready_queue_length = if total_decisions > 0 {
            total_queue_length as f32 / total_decisions as f32
        } else {
            0.0
        };
        
        SchedulerProfile {
            total_decisions,
            avg_decision_latency,
            max_decision_latency,
            avg_ready_queue_length,
            max_ready_queue_length: max_queue_length,
            load_balance_efficiency: 0.0, // TODO: Calculate based on load distribution
        }
    }
    
    /// Clear all profiling data.
    pub fn clear(&self) {
        if let Some(mut samples) = self.samples.try_lock() {
            samples.clear();
        }
        
        if let Some(mut stats) = self.thread_stats.try_lock() {
            stats.clear();
        }
        
        self.total_samples.store(0, Ordering::Release);
        self.sample_counter.store(0, Ordering::Release);
        
        if let Some(mut start_time) = self.start_time.try_lock() {
            *start_time = Some(Instant::now());
        }
    }
    
    /// Get current profiling statistics.
    pub fn get_stats(&self) -> (u64, usize) {
        let total_samples = self.total_samples.load(Ordering::Acquire);
        let sample_count = if let Some(samples) = self.samples.try_lock() {
            samples.len()
        } else {
            0
        };
        
        (total_samples, sample_count)
    }
}

/// Global profiler instance.
pub static GLOBAL_PROFILER: ThreadProfiler = ThreadProfiler::new();

/// Initialize the global profiler.
pub fn init_profiler(config: ProfilerConfig) -> Result<(), &'static str> {
    GLOBAL_PROFILER.init(config)
}

/// Cleanup profiling.
pub fn cleanup_profiler() {
    GLOBAL_PROFILER.enabled.store(false, Ordering::Release);
}
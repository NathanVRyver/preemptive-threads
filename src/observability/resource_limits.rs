//! Resource limit enforcement and tracking.
//!
//! This module provides comprehensive resource limit enforcement including
//! CPU time, memory usage, file descriptors, and thread counts with
//! real-time monitoring and violation detection.

use portable_atomic::{AtomicU64, AtomicUsize, AtomicBool, Ordering};
use crate::time::{Duration, Instant};
use crate::thread_new::ThreadId;
use crate::errors::{ResourceError, ThreadError};
extern crate alloc;
use alloc::{vec::Vec, collections::BTreeMap};
use spin::Mutex;

/// Resource usage tracking for a single thread.
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    /// Thread ID
    pub thread_id: ThreadId,
    /// CPU time consumed (nanoseconds)
    pub cpu_time_ns: u64,
    /// Memory currently allocated (bytes)
    pub memory_usage: u64,
    /// Peak memory usage (bytes)
    pub peak_memory_usage: u64,
    /// Number of file descriptors open
    pub open_files: u32,
    /// Number of child threads created
    pub child_threads: u32,
    /// Network connections opened
    pub network_connections: u32,
    /// Disk I/O operations performed
    pub disk_io_ops: u64,
    /// Network bytes transmitted
    pub network_bytes_tx: u64,
    /// Network bytes received
    pub network_bytes_rx: u64,
    /// Last update timestamp
    pub last_updated: Instant,
}

impl ResourceUsage {
    /// Create new resource usage tracker.
    pub fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            cpu_time_ns: 0,
            memory_usage: 0,
            peak_memory_usage: 0,
            open_files: 0,
            child_threads: 0,
            network_connections: 0,
            disk_io_ops: 0,
            network_bytes_tx: 0,
            network_bytes_rx: 0,
            last_updated: Instant::now(),
        }
    }
    
    /// Update CPU time usage.
    pub fn add_cpu_time(&mut self, duration: Duration) {
        self.cpu_time_ns += duration.as_nanos();
        self.last_updated = Instant::now();
    }
    
    /// Update memory usage.
    pub fn update_memory_usage(&mut self, new_usage: u64) {
        self.memory_usage = new_usage;
        if new_usage > self.peak_memory_usage {
            self.peak_memory_usage = new_usage;
        }
        self.last_updated = Instant::now();
    }
    
    /// Record file descriptor allocation.
    pub fn allocate_file_descriptor(&mut self) {
        self.open_files += 1;
        self.last_updated = Instant::now();
    }
    
    /// Record file descriptor deallocation.
    pub fn deallocate_file_descriptor(&mut self) {
        if self.open_files > 0 {
            self.open_files -= 1;
        }
        self.last_updated = Instant::now();
    }
    
    /// Record child thread creation.
    pub fn add_child_thread(&mut self) {
        self.child_threads += 1;
        self.last_updated = Instant::now();
    }
    
    /// Record child thread termination.
    pub fn remove_child_thread(&mut self) {
        if self.child_threads > 0 {
            self.child_threads -= 1;
        }
        self.last_updated = Instant::now();
    }
}

/// Resource quota limits for a thread or system.
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    /// Maximum CPU time (nanoseconds), 0 = unlimited
    pub max_cpu_time_ns: u64,
    /// Maximum memory usage (bytes), 0 = unlimited
    pub max_memory_bytes: u64,
    /// Maximum number of open file descriptors, 0 = unlimited
    pub max_open_files: u32,
    /// Maximum number of child threads, 0 = unlimited
    pub max_child_threads: u32,
    /// Maximum network connections, 0 = unlimited
    pub max_network_connections: u32,
    /// Maximum disk I/O operations per second, 0 = unlimited
    pub max_disk_iops: u64,
    /// Maximum network bandwidth (bytes/second), 0 = unlimited
    pub max_network_bw: u64,
    /// Hard limits (terminate on violation) vs soft limits (warn on violation)
    pub hard_limits: bool,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_cpu_time_ns: 0, // Unlimited by default
            max_memory_bytes: 0,
            max_open_files: 0,
            max_child_threads: 0,
            max_network_connections: 0,
            max_disk_iops: 0,
            max_network_bw: 0,
            hard_limits: false, // Soft limits by default
        }
    }
}

impl ResourceQuota {
    /// Create a conservative quota for embedded systems.
    pub fn conservative() -> Self {
        Self {
            max_cpu_time_ns: 10_000_000_000, // 10 seconds max CPU time
            max_memory_bytes: 1024 * 1024,   // 1MB memory limit
            max_open_files: 10,              // 10 file descriptors
            max_child_threads: 5,            // 5 child threads
            max_network_connections: 5,      // 5 network connections
            max_disk_iops: 1000,             // 1000 IOPS
            max_network_bw: 1024 * 1024,     // 1MB/s bandwidth
            hard_limits: true,
        }
    }
    
    /// Create a permissive quota for development/testing.
    pub fn permissive() -> Self {
        Self {
            max_cpu_time_ns: 0,     // Unlimited
            max_memory_bytes: 0,    // Unlimited
            max_open_files: 1000,   // 1000 file descriptors
            max_child_threads: 100, // 100 child threads
            max_network_connections: 100,
            max_disk_iops: 0,       // Unlimited
            max_network_bw: 0,      // Unlimited
            hard_limits: false,     // Soft limits
        }
    }
}

/// Resource limit violation information.
#[derive(Debug, Clone)]
pub struct LimitViolation {
    /// Thread that violated the limit
    pub thread_id: ThreadId,
    /// Type of resource that exceeded limit
    pub resource_type: ResourceType,
    /// Current usage value
    pub current_usage: u64,
    /// Limit that was exceeded
    pub limit: u64,
    /// Whether this was a hard or soft limit
    pub hard_limit: bool,
    /// Timestamp of the violation
    pub timestamp: Instant,
    /// Suggested action to take
    pub suggested_action: ViolationAction,
}

/// Types of resources that can be limited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    CpuTime,
    Memory,
    FileDescriptors,
    ChildThreads,
    NetworkConnections,
    DiskIOPS,
    NetworkBandwidth,
}

/// Actions to take on resource limit violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationAction {
    /// Log warning and continue
    Warn,
    /// Throttle the resource usage
    Throttle,
    /// Suspend the thread temporarily
    Suspend,
    /// Terminate the thread
    Terminate,
    /// Notify system administrator
    Alert,
}

/// Resource limiter and enforcement engine.
pub struct ResourceLimiter {
    /// Per-thread resource usage tracking
    thread_usage: Mutex<BTreeMap<ThreadId, ResourceUsage>>,
    /// Per-thread resource quotas
    thread_quotas: Mutex<BTreeMap<ThreadId, ResourceQuota>>,
    /// Default quota for new threads
    default_quota: Mutex<ResourceQuota>,
    /// System-wide resource usage
    system_usage: SystemResourceUsage,
    /// Violation callback
    violation_handler: Mutex<Option<fn(&LimitViolation)>>,
    /// Enforcement enabled flag
    enabled: AtomicBool,
    /// Violation history
    violations: Mutex<Vec<LimitViolation>>,
    /// Maximum violations to keep in history
    max_violation_history: AtomicUsize,
}

/// System-wide resource usage tracking.
pub struct SystemResourceUsage {
    /// Total active threads
    pub total_threads: AtomicU64,
    /// Total memory usage across all threads
    pub total_memory_usage: AtomicU64,
    /// Total CPU time across all threads
    pub total_cpu_time_ns: AtomicU64,
    /// Total file descriptors open
    pub total_open_files: AtomicU64,
    /// Total network connections
    pub total_network_connections: AtomicU64,
    /// System start time for CPU time calculations
    pub system_start_time: Instant,
}

impl SystemResourceUsage {
    pub const fn new() -> Self {
        Self {
            total_threads: AtomicU64::new(0),
            total_memory_usage: AtomicU64::new(0),
            total_cpu_time_ns: AtomicU64::new(0),
            total_open_files: AtomicU64::new(0),
            total_network_connections: AtomicU64::new(0),
            system_start_time: unsafe { core::mem::transmute(0u64) },
        }
    }
}

impl ResourceLimiter {
    /// Create a new resource limiter.
    pub const fn new() -> Self {
        Self {
            thread_usage: Mutex::new(BTreeMap::new()),
            thread_quotas: Mutex::new(BTreeMap::new()),
            default_quota: Mutex::new(ResourceQuota {
                max_cpu_time_ns: 0,
                max_memory_bytes: 0,
                max_open_files: 0,
                max_child_threads: 0,
                max_network_connections: 0,
                max_disk_iops: 0,
                max_network_bw: 0,
                hard_limits: false,
            }),
            system_usage: SystemResourceUsage::new(),
            violation_handler: Mutex::new(None),
            enabled: AtomicBool::new(false),
            violations: Mutex::new(Vec::new()),
            max_violation_history: AtomicUsize::new(1000),
        }
    }
    
    /// Initialize the resource limiter.
    pub fn init(&self) -> Result<(), &'static str> {
        self.enabled.store(true, Ordering::Release);
        Ok(())
    }
    
    /// Check if resource limiting is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    
    /// Set the default quota for new threads.
    pub fn set_default_quota(&self, quota: ResourceQuota) {
        if let Some(mut default_quota) = self.default_quota.try_lock() {
            *default_quota = quota;
        }
    }
    
    /// Set a custom quota for a specific thread.
    pub fn set_thread_quota(&self, thread_id: ThreadId, quota: ResourceQuota) {
        if let Some(mut quotas) = self.thread_quotas.try_lock() {
            quotas.insert(thread_id, quota);
        }
    }
    
    /// Register a new thread for resource tracking.
    pub fn register_thread(&self, thread_id: ThreadId) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut usage) = self.thread_usage.try_lock() {
            usage.insert(thread_id, ResourceUsage::new(thread_id));
        }
        
        self.system_usage.total_threads.fetch_add(1, Ordering::AcqRel);
    }
    
    /// Unregister a thread from resource tracking.
    pub fn unregister_thread(&self, thread_id: ThreadId) {
        if !self.is_enabled() {
            return;
        }
        
        // Remove from tracking
        if let Some(mut usage) = self.thread_usage.try_lock() {
            if let Some(removed_usage) = usage.remove(&thread_id) {
                // Update system totals
                self.system_usage.total_memory_usage.fetch_sub(
                    removed_usage.memory_usage, 
                    Ordering::AcqRel
                );
                self.system_usage.total_open_files.fetch_sub(
                    removed_usage.open_files as u64, 
                    Ordering::AcqRel
                );
            }
        }
        
        // Remove quota
        if let Some(mut quotas) = self.thread_quotas.try_lock() {
            quotas.remove(&thread_id);
        }
        
        self.system_usage.total_threads.fetch_sub(1, Ordering::AcqRel);
    }
    
    /// Check if a resource allocation would violate limits.
    pub fn check_resource_limit(
        &self,
        thread_id: ThreadId,
        resource_type: ResourceType,
        requested_amount: u64,
    ) -> Result<(), ThreadError> {
        if !self.is_enabled() {
            return Ok(());
        }
        
        let quota = self.get_thread_quota(thread_id);
        let current_usage = self.get_thread_usage(thread_id);
        
        let (current_value, limit) = match resource_type {
            ResourceType::CpuTime => (current_usage.cpu_time_ns, quota.max_cpu_time_ns),
            ResourceType::Memory => (current_usage.memory_usage, quota.max_memory_bytes),
            ResourceType::FileDescriptors => (current_usage.open_files as u64, quota.max_open_files as u64),
            ResourceType::ChildThreads => (current_usage.child_threads as u64, quota.max_child_threads as u64),
            ResourceType::NetworkConnections => (current_usage.network_connections as u64, quota.max_network_connections as u64),
            ResourceType::DiskIOPS => (current_usage.disk_io_ops, quota.max_disk_iops),
            ResourceType::NetworkBandwidth => {
                let bw = current_usage.network_bytes_tx + current_usage.network_bytes_rx;
                (bw, quota.max_network_bw)
            },
        };
        
        if limit > 0 && (current_value + requested_amount) > limit {
            let violation = LimitViolation {
                thread_id,
                resource_type,
                current_usage: current_value + requested_amount,
                limit,
                hard_limit: quota.hard_limits,
                timestamp: Instant::now(),
                suggested_action: if quota.hard_limits {
                    ViolationAction::Terminate
                } else {
                    ViolationAction::Warn
                },
            };
            
            self.handle_violation(violation.clone());
            
            if quota.hard_limits {
                match resource_type {
                    ResourceType::CpuTime => Err(ThreadError::Resource(ResourceError::MaxCpuTime)),
                    ResourceType::Memory => Err(ThreadError::Resource(ResourceError::MaxMemoryUsage)),
                    ResourceType::FileDescriptors => Err(ThreadError::Resource(ResourceError::MaxFileDescriptors)),
                    ResourceType::ChildThreads => Err(ThreadError::Resource(ResourceError::MaxThreadsPerProcess)),
                    ResourceType::NetworkConnections => Err(ThreadError::Resource(ResourceError::ResourceUnavailable)),
                    ResourceType::DiskIOPS => Err(ThreadError::Resource(ResourceError::ResourceUnavailable)),
                    ResourceType::NetworkBandwidth => Err(ThreadError::Resource(ResourceError::ResourceUnavailable)),
                }
            } else {
                Ok(()) // Soft limit - allow but warn
            }
        } else {
            Ok(())
        }
    }
    
    /// Update resource usage for a thread.
    pub fn update_resource_usage(&self, thread_id: ThreadId, resource_type: ResourceType, new_value: u64) {
        if !self.is_enabled() {
            return;
        }
        
        if let Some(mut usage) = self.thread_usage.try_lock() {
            if let Some(thread_usage) = usage.get_mut(&thread_id) {
                let old_value = match resource_type {
                    ResourceType::Memory => {
                        let old = thread_usage.memory_usage;
                        thread_usage.update_memory_usage(new_value);
                        old
                    },
                    ResourceType::CpuTime => {
                        let duration = Duration::from_nanos(new_value);
                        thread_usage.add_cpu_time(duration);
                        thread_usage.cpu_time_ns - new_value
                    },
                    ResourceType::FileDescriptors => {
                        let old = thread_usage.open_files as u64;
                        thread_usage.open_files = new_value as u32;
                        old
                    },
                    ResourceType::ChildThreads => {
                        let old = thread_usage.child_threads as u64;
                        thread_usage.child_threads = new_value as u32;
                        old
                    },
                    _ => 0, // Other resources handled separately
                };
                
                // Update system totals
                match resource_type {
                    ResourceType::Memory => {
                        if new_value > old_value {
                            self.system_usage.total_memory_usage.fetch_add(new_value - old_value, Ordering::AcqRel);
                        } else {
                            self.system_usage.total_memory_usage.fetch_sub(old_value - new_value, Ordering::AcqRel);
                        }
                    },
                    ResourceType::CpuTime => {
                        self.system_usage.total_cpu_time_ns.fetch_add(new_value, Ordering::AcqRel);
                    },
                    _ => {}
                }
            }
        }
    }
    
    /// Get resource usage for a thread.
    fn get_thread_usage(&self, thread_id: ThreadId) -> ResourceUsage {
        if let Some(usage) = self.thread_usage.try_lock() {
            usage.get(&thread_id).cloned().unwrap_or_else(|| ResourceUsage::new(thread_id))
        } else {
            ResourceUsage::new(thread_id)
        }
    }
    
    /// Get quota for a thread.
    fn get_thread_quota(&self, thread_id: ThreadId) -> ResourceQuota {
        if let Some(quotas) = self.thread_quotas.try_lock() {
            if let Some(quota) = quotas.get(&thread_id) {
                return quota.clone();
            }
        }
        
        // Use default quota
        if let Some(default_quota) = self.default_quota.try_lock() {
            default_quota.clone()
        } else {
            ResourceQuota::default()
        }
    }
    
    /// Handle a resource limit violation.
    fn handle_violation(&self, violation: LimitViolation) {
        // Add to violation history
        if let Some(mut violations) = self.violations.try_lock() {
            violations.push(violation.clone());
            
            // Trim history if too long
            let max_history = self.max_violation_history.load(Ordering::Acquire);
            let violations_len = violations.len();
            if violations_len > max_history {
                violations.drain(0..violations_len - max_history);
            }
        }
        
        // Call violation handler if set
        if let Some(handler) = self.violation_handler.try_lock() {
            if let Some(callback) = *handler {
                callback(&violation);
            }
        }
    }
    
    /// Set violation handler callback.
    pub fn set_violation_handler(&self, handler: fn(&LimitViolation)) {
        if let Some(mut callback) = self.violation_handler.try_lock() {
            *callback = Some(handler);
        }
    }
    
    /// Get recent violation history.
    pub fn get_violation_history(&self) -> Vec<LimitViolation> {
        if let Some(violations) = self.violations.try_lock() {
            violations.clone()
        } else {
            Vec::new()
        }
    }
    
    /// Get system resource usage summary.
    pub fn get_system_usage(&self) -> SystemResourceUsage {
        // Since SystemResourceUsage contains atomics, we can't clone it directly
        // Return a snapshot instead
        SystemResourceUsage {
            total_threads: AtomicU64::new(self.system_usage.total_threads.load(Ordering::Acquire)),
            total_memory_usage: AtomicU64::new(self.system_usage.total_memory_usage.load(Ordering::Acquire)),
            total_cpu_time_ns: AtomicU64::new(self.system_usage.total_cpu_time_ns.load(Ordering::Acquire)),
            total_open_files: AtomicU64::new(self.system_usage.total_open_files.load(Ordering::Acquire)),
            total_network_connections: AtomicU64::new(self.system_usage.total_network_connections.load(Ordering::Acquire)),
            system_start_time: self.system_usage.system_start_time,
        }
    }
}

/// Global resource limiter instance.
pub static GLOBAL_RESOURCE_LIMITER: ResourceLimiter = ResourceLimiter::new();

/// Initialize the global resource limiter.
pub fn init_resource_limiter() -> Result<(), &'static str> {
    GLOBAL_RESOURCE_LIMITER.init()
}

/// Cleanup resource limiting.
pub fn cleanup_resource_limiter() {
    GLOBAL_RESOURCE_LIMITER.enabled.store(false, Ordering::Release);
}
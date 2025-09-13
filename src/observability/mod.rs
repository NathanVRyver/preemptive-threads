//! Observability and monitoring for the threading system.
//!
//! This module provides comprehensive metrics collection, resource monitoring,
//! and performance profiling capabilities for production threading systems.

pub mod metrics;
pub mod resource_limits;
pub mod profiler;
pub mod health;

pub use metrics::{ThreadMetrics, SystemMetrics, MetricsCollector, GLOBAL_METRICS};
pub use resource_limits::{ResourceLimiter, ResourceUsage, ResourceQuota, LimitViolation};
pub use profiler::{ThreadProfiler, ProfileData, ProfilerConfig, GLOBAL_PROFILER};
pub use health::{HealthMonitor, HealthStatus, SystemHealth, HEALTH_MONITOR};

/// Global observability configuration.
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Enable detailed metrics collection
    pub enable_metrics: bool,
    /// Enable resource limit enforcement
    pub enable_limits: bool,
    /// Enable performance profiling
    pub enable_profiling: bool,
    /// Enable health monitoring
    pub enable_health: bool,
    /// Metrics collection interval in milliseconds
    pub metrics_interval_ms: u32,
    /// Health check interval in milliseconds
    pub health_check_interval_ms: u32,
    /// Maximum number of profiling samples to keep
    pub max_profile_samples: usize,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_metrics: cfg!(debug_assertions),
            enable_limits: true,
            enable_profiling: cfg!(debug_assertions),
            enable_health: true,
            metrics_interval_ms: 1000, // 1 second
            health_check_interval_ms: 5000, // 5 seconds
            max_profile_samples: 1000,
        }
    }
}

/// Initialize observability subsystems with the given configuration.
pub fn init_observability(config: ObservabilityConfig) -> Result<(), &'static str> {
    if config.enable_metrics {
        metrics::init_metrics_collector(config.metrics_interval_ms)?;
    }
    
    if config.enable_limits {
        resource_limits::init_resource_limiter()?;
    }
    
    if config.enable_profiling {
        profiler::init_profiler(profiler::ProfilerConfig {
            max_samples: config.max_profile_samples,
            sampling_enabled: true,
            sampling_interval_us: 1000,
            stack_tracing_enabled: false,
            max_stack_depth: 32,
            memory_tracking_enabled: true,
            scheduler_tracking_enabled: true,
        })?;
    }
    
    if config.enable_health {
        health::init_health_monitor(config.health_check_interval_ms)?;
    }
    
    Ok(())
}

/// Cleanup observability subsystems.
pub fn cleanup_observability() {
    metrics::cleanup_metrics();
    resource_limits::cleanup_resource_limiter();
    profiler::cleanup_profiler();
    health::cleanup_health_monitor();
}
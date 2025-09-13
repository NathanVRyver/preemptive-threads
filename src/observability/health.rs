//! Health monitoring and system diagnostics.
//!
//! This module provides comprehensive health monitoring for the threading
//! system including deadlock detection, resource exhaustion monitoring,
//! and overall system health assessment.

use portable_atomic::{AtomicU64, AtomicU32, AtomicUsize, AtomicBool, Ordering};
use crate::time::{Duration, Instant};
use crate::thread_new::ThreadId;
extern crate alloc;
use alloc::{vec::Vec, collections::BTreeMap, string::{String, ToString}, boxed::Box};
use spin::Mutex;

/// Overall system health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// System is operating normally
    Healthy,
    /// System has minor issues but is functional
    Warning,
    /// System has serious issues that may affect performance
    Critical,
    /// System is in a failed state
    Failed,
}

impl HealthStatus {
    /// Get a numeric score for the health status.
    pub fn score(self) -> u8 {
        match self {
            HealthStatus::Healthy => 100,
            HealthStatus::Warning => 75,
            HealthStatus::Critical => 25,
            HealthStatus::Failed => 0,
        }
    }
    
    /// Combine two health statuses, returning the worse one.
    pub fn combine(self, other: HealthStatus) -> HealthStatus {
        if self.score() < other.score() {
            self
        } else {
            other
        }
    }
}

/// Detailed health information for different system components.
#[derive(Debug, Clone)]
pub struct SystemHealth {
    /// Overall system health
    pub overall_status: HealthStatus,
    /// Individual component health
    pub components: BTreeMap<String, ComponentHealth>,
    /// Active health issues
    pub active_issues: Vec<HealthIssue>,
    /// Health check timestamp
    pub timestamp: Instant,
    /// System uptime
    pub uptime: Duration,
    /// Health trend over time
    pub trend: HealthTrend,
}

/// Health status of a specific system component.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Component health status
    pub status: HealthStatus,
    /// Health metrics for this component
    pub metrics: ComponentMetrics,
    /// Last health check time
    pub last_check: Instant,
    /// Component-specific issues
    pub issues: Vec<HealthIssue>,
}

/// Health metrics for a component.
#[derive(Debug, Clone)]
pub struct ComponentMetrics {
    /// Resource utilization (0-100%)
    pub resource_utilization: f32,
    /// Error rate (errors per second)
    pub error_rate: f32,
    /// Response time (average in microseconds)
    pub avg_response_time_us: u64,
    /// Success rate (0-100%)
    pub success_rate: f32,
    /// Queue depth or backlog
    pub queue_depth: usize,
    /// Component-specific custom metrics
    pub custom_metrics: BTreeMap<String, f64>,
}

impl Default for ComponentMetrics {
    fn default() -> Self {
        Self {
            resource_utilization: 0.0,
            error_rate: 0.0,
            avg_response_time_us: 0,
            success_rate: 100.0,
            queue_depth: 0,
            custom_metrics: BTreeMap::new(),
        }
    }
}

/// A specific health issue detected in the system.
#[derive(Debug, Clone)]
pub struct HealthIssue {
    /// Issue severity
    pub severity: IssueSeverity,
    /// Issue category
    pub category: IssueCategory,
    /// Human-readable description
    pub description: String,
    /// Component that reported the issue
    pub component: String,
    /// When the issue was first detected
    pub detected_at: Instant,
    /// Additional context data
    pub context: BTreeMap<String, String>,
    /// Suggested remediation action
    pub remediation: Option<String>,
}

/// Severity levels for health issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Info,
    Warning,
    Critical,
    Fatal,
}

/// Categories of health issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCategory {
    Performance,
    Resource,
    Concurrency,
    Memory,
    Scheduler,
    Deadlock,
    Configuration,
    Hardware,
    Other,
}

/// Health trend analysis.
#[derive(Debug, Clone)]
pub struct HealthTrend {
    /// Health score trend (improving, stable, degrading)
    pub direction: TrendDirection,
    /// Rate of change
    pub change_rate: f32,
    /// Confidence in the trend analysis (0-1)
    pub confidence: f32,
    /// Historical health scores
    pub history: Vec<HealthHistoryEntry>,
}

/// Direction of health trend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
    Unknown,
}

/// Historical health data point.
#[derive(Debug, Clone)]
pub struct HealthHistoryEntry {
    pub timestamp: Instant,
    pub health_score: u8,
    pub active_issues: usize,
}

/// Health monitoring configuration.
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    /// Health check interval in milliseconds
    pub check_interval_ms: u32,
    /// Enable deadlock detection
    pub enable_deadlock_detection: bool,
    /// Enable resource monitoring
    pub enable_resource_monitoring: bool,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Maximum history entries to keep
    pub max_history_entries: usize,
    /// Thresholds for health status transitions
    pub thresholds: HealthThresholds,
}

/// Thresholds for determining health status.
#[derive(Debug, Clone)]
pub struct HealthThresholds {
    /// CPU utilization threshold for warnings (%)
    pub cpu_warning_threshold: f32,
    /// CPU utilization threshold for critical (%)
    pub cpu_critical_threshold: f32,
    /// Memory utilization threshold for warnings (%)
    pub memory_warning_threshold: f32,
    /// Memory utilization threshold for critical (%)
    pub memory_critical_threshold: f32,
    /// Context switch rate threshold for warnings (switches/sec)
    pub context_switch_warning_threshold: f32,
    /// Context switch rate threshold for critical (switches/sec)
    pub context_switch_critical_threshold: f32,
    /// Maximum acceptable error rate (errors/sec)
    pub max_error_rate: f32,
    /// Maximum acceptable response time (microseconds)
    pub max_response_time_us: u64,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 5000, // 5 seconds
            enable_deadlock_detection: true,
            enable_resource_monitoring: true,
            enable_performance_monitoring: true,
            max_history_entries: 100,
            thresholds: HealthThresholds {
                cpu_warning_threshold: 80.0,
                cpu_critical_threshold: 95.0,
                memory_warning_threshold: 85.0,
                memory_critical_threshold: 95.0,
                context_switch_warning_threshold: 10000.0,
                context_switch_critical_threshold: 50000.0,
                max_error_rate: 10.0,
                max_response_time_us: 100000, // 100ms
            },
        }
    }
}

/// Health monitor implementation.
pub struct HealthMonitor {
    /// Configuration
    config: Mutex<HealthMonitorConfig>,
    /// Current system health
    current_health: Mutex<SystemHealth>,
    /// Component registrations
    components: Mutex<BTreeMap<String, ComponentHealth>>,
    /// Active health checkers
    checkers: Mutex<Vec<Box<dyn HealthChecker + Send + Sync>>>,
    /// Health monitoring enabled flag
    enabled: AtomicBool,
    /// System start time
    system_start_time: Instant,
    /// Last health check time
    last_check_time: Mutex<Instant>,
    /// Health check counter
    check_counter: AtomicU64,
}

/// Trait for health checkers.
pub trait HealthChecker: Send + Sync {
    /// Get the name of this health checker.
    fn name(&self) -> &str;
    
    /// Perform a health check and return component health.
    fn check_health(&self) -> ComponentHealth;
    
    /// Get the check interval for this checker (in milliseconds).
    fn check_interval_ms(&self) -> u32 {
        5000 // Default 5 seconds
    }
}

impl HealthMonitor {
    /// Create a new health monitor (const version for statics).
    pub const fn const_new() -> Self {
        let now = unsafe { core::mem::transmute(0u64) };
        Self {
            config: Mutex::new(HealthMonitorConfig {
                check_interval_ms: 5000,
                enable_deadlock_detection: true,
                enable_resource_monitoring: true,
                enable_performance_monitoring: true,
                max_history_entries: 100,
                thresholds: HealthThresholds {
                    cpu_warning_threshold: 80.0,
                    cpu_critical_threshold: 95.0,
                    memory_warning_threshold: 85.0,
                    memory_critical_threshold: 95.0,
                    context_switch_warning_threshold: 10000.0,
                    context_switch_critical_threshold: 50000.0,
                    max_error_rate: 10.0,
                    max_response_time_us: 100000,
                },
            }),
            current_health: Mutex::new(SystemHealth {
                overall_status: HealthStatus::Healthy,
                components: BTreeMap::new(),
                active_issues: Vec::new(),
                timestamp: now,
                uptime: unsafe { core::mem::transmute(0u64) },
                trend: HealthTrend {
                    direction: TrendDirection::Unknown,
                    change_rate: 0.0,
                    confidence: 0.0,
                    history: Vec::new(),
                },
            }),
            components: Mutex::new(BTreeMap::new()),
            checkers: Mutex::new(Vec::new()),
            enabled: AtomicBool::new(false),
            system_start_time: now,
            last_check_time: Mutex::new(now),
            check_counter: AtomicU64::new(0),
        }
    }
    
    /// Create a new health monitor.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            config: Mutex::new(HealthMonitorConfig::default()),
            current_health: Mutex::new(SystemHealth {
                overall_status: HealthStatus::Healthy,
                components: BTreeMap::new(),
                active_issues: Vec::new(),
                timestamp: now,
                uptime: Duration::from_nanos(0),
                trend: HealthTrend {
                    direction: TrendDirection::Unknown,
                    change_rate: 0.0,
                    confidence: 0.0,
                    history: Vec::new(),
                },
            }),
            components: Mutex::new(BTreeMap::new()),
            checkers: Mutex::new(Vec::new()),
            enabled: AtomicBool::new(false),
            system_start_time: now,
            last_check_time: Mutex::new(now),
            check_counter: AtomicU64::new(0),
        }
    }
    
    /// Initialize the health monitor.
    pub fn init(&self, config: HealthMonitorConfig) -> Result<(), &'static str> {
        if let Some(mut monitor_config) = self.config.try_lock() {
            *monitor_config = config;
        } else {
            return Err("Failed to lock health monitor config");
        }
        
        // Initialize default health checkers
        self.register_default_checkers()?;
        
        self.enabled.store(true, Ordering::Release);
        Ok(())
    }
    
    /// Register default health checkers.
    fn register_default_checkers(&self) -> Result<(), &'static str> {
        let config = if let Some(config) = self.config.try_lock() {
            config.clone()
        } else {
            return Err("Failed to lock config");
        };
        
        if let Some(mut checkers) = self.checkers.try_lock() {
            if config.enable_resource_monitoring {
                checkers.push(Box::new(ResourceHealthChecker::new()));
            }
            
            if config.enable_performance_monitoring {
                checkers.push(Box::new(PerformanceHealthChecker::new()));
            }
            
            if config.enable_deadlock_detection {
                checkers.push(Box::new(DeadlockHealthChecker::new()));
            }
        }
        
        Ok(())
    }
    
    /// Check if health monitoring is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
    
    /// Register a custom health checker.
    pub fn register_checker(&self, checker: Box<dyn HealthChecker + Send + Sync>) {
        if let Some(mut checkers) = self.checkers.try_lock() {
            checkers.push(checker);
        }
    }
    
    /// Perform a comprehensive health check.
    pub fn check_health(&self) -> SystemHealth {
        if !self.is_enabled() {
            return self.get_current_health();
        }
        
        let now = Instant::now();
        let uptime = now.duration_since(self.system_start_time);
        
        // Run all health checkers
        let mut component_healths = BTreeMap::new();
        let mut all_issues = Vec::new();
        
        if let Some(checkers) = self.checkers.try_lock() {
            for checker in checkers.iter() {
                let component_health = checker.check_health();
                all_issues.extend(component_health.issues.clone());
                component_healths.insert(component_health.name.clone(), component_health);
            }
        }
        
        // Determine overall health status
        let overall_status = self.calculate_overall_status(&component_healths, &all_issues);
        
        // Update health trend
        let trend = self.update_health_trend(overall_status);
        
        // Create health report
        let health = SystemHealth {
            overall_status,
            components: component_healths,
            active_issues: all_issues,
            timestamp: now,
            uptime,
            trend,
        };
        
        // Update current health
        if let Some(mut current_health) = self.current_health.try_lock() {
            *current_health = health.clone();
        }
        
        // Update components registry
        if let Some(mut components) = self.components.try_lock() {
            *components = health.components.clone();
        }
        
        if let Some(mut last_check) = self.last_check_time.try_lock() {
            *last_check = now;
        }
        
        self.check_counter.fetch_add(1, Ordering::AcqRel);
        
        health
    }
    
    /// Calculate overall system health status.
    fn calculate_overall_status(
        &self,
        components: &BTreeMap<String, ComponentHealth>,
        issues: &[HealthIssue],
    ) -> HealthStatus {
        // Start with healthy status
        let mut overall = HealthStatus::Healthy;
        
        // Check component statuses
        for component in components.values() {
            overall = overall.combine(component.status);
        }
        
        // Check issue severities
        for issue in issues {
            let issue_status = match issue.severity {
                IssueSeverity::Info => HealthStatus::Healthy,
                IssueSeverity::Warning => HealthStatus::Warning,
                IssueSeverity::Critical => HealthStatus::Critical,
                IssueSeverity::Fatal => HealthStatus::Failed,
            };
            
            overall = overall.combine(issue_status);
        }
        
        overall
    }
    
    /// Update health trend analysis.
    fn update_health_trend(&self, current_status: HealthStatus) -> HealthTrend {
        let mut trend = if let Some(health) = self.current_health.try_lock() {
            health.trend.clone()
        } else {
            HealthTrend {
                direction: TrendDirection::Unknown,
                change_rate: 0.0,
                confidence: 0.0,
                history: Vec::new(),
            }
        };
        
        // Add current health score to history
        let history_entry = HealthHistoryEntry {
            timestamp: Instant::now(),
            health_score: current_status.score(),
            active_issues: 0, // TODO: Count active issues
        };
        
        trend.history.push(history_entry);
        
        // Keep only recent history
        let max_entries = if let Some(config) = self.config.try_lock() {
            config.max_history_entries
        } else {
            100
        };
        
        if trend.history.len() > max_entries {
            trend.history.drain(0..trend.history.len() - max_entries);
        }
        
        // Calculate trend direction
        if trend.history.len() >= 2 {
            let recent_avg = trend.history.iter()
                .rev()
                .take(5)
                .map(|entry| entry.health_score as f32)
                .sum::<f32>() / 5.0_f32.min(trend.history.len() as f32);
            
            let older_avg = trend.history.iter()
                .rev()
                .skip(5)
                .take(5)
                .map(|entry| entry.health_score as f32)
                .sum::<f32>() / 5.0_f32.min((trend.history.len() - 5) as f32);
            
            let change = recent_avg - older_avg;
            trend.change_rate = change;
            
            trend.direction = if change > 2.0 {
                TrendDirection::Improving
            } else if change < -2.0 {
                TrendDirection::Degrading
            } else {
                TrendDirection::Stable
            };
            
            trend.confidence = (trend.history.len() as f32 / max_entries as f32).min(1.0);
        }
        
        trend
    }
    
    /// Get current health status.
    pub fn get_current_health(&self) -> SystemHealth {
        if let Some(health) = self.current_health.try_lock() {
            health.clone()
        } else {
            SystemHealth {
                overall_status: HealthStatus::Warning,
                components: BTreeMap::new(),
                active_issues: Vec::new(),
                timestamp: Instant::now(),
                uptime: Duration::from_nanos(0),
                trend: HealthTrend {
                    direction: TrendDirection::Unknown,
                    change_rate: 0.0,
                    confidence: 0.0,
                    history: Vec::new(),
                },
            }
        }
    }
    
    /// Report a health issue.
    pub fn report_issue(&self, issue: HealthIssue) {
        if let Some(mut health) = self.current_health.try_lock() {
            health.active_issues.push(issue);
        }
    }
    
    /// Get health statistics.
    pub fn get_stats(&self) -> (u64, usize, HealthStatus) {
        let check_count = self.check_counter.load(Ordering::Acquire);
        let component_count = if let Some(components) = self.components.try_lock() {
            components.len()
        } else {
            0
        };
        let current_status = self.get_current_health().overall_status;
        
        (check_count, component_count, current_status)
    }
}

/// Resource health checker implementation.
pub struct ResourceHealthChecker {
    name: String,
}

impl ResourceHealthChecker {
    pub fn new() -> Self {
        Self {
            name: "resource".to_string(),
        }
    }
}

impl HealthChecker for ResourceHealthChecker {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn check_health(&self) -> ComponentHealth {
        // TODO: Implement actual resource health checks
        ComponentHealth {
            name: self.name.clone(),
            status: HealthStatus::Healthy,
            metrics: ComponentMetrics::default(),
            last_check: Instant::now(),
            issues: Vec::new(),
        }
    }
}

/// Performance health checker implementation.
pub struct PerformanceHealthChecker {
    name: String,
}

impl PerformanceHealthChecker {
    pub fn new() -> Self {
        Self {
            name: "performance".to_string(),
        }
    }
}

impl HealthChecker for PerformanceHealthChecker {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn check_health(&self) -> ComponentHealth {
        // TODO: Implement actual performance health checks
        ComponentHealth {
            name: self.name.clone(),
            status: HealthStatus::Healthy,
            metrics: ComponentMetrics::default(),
            last_check: Instant::now(),
            issues: Vec::new(),
        }
    }
}

/// Deadlock health checker implementation.
pub struct DeadlockHealthChecker {
    name: String,
}

impl DeadlockHealthChecker {
    pub fn new() -> Self {
        Self {
            name: "deadlock".to_string(),
        }
    }
}

impl HealthChecker for DeadlockHealthChecker {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn check_health(&self) -> ComponentHealth {
        // TODO: Implement actual deadlock detection
        ComponentHealth {
            name: self.name.clone(),
            status: HealthStatus::Healthy,
            metrics: ComponentMetrics::default(),
            last_check: Instant::now(),
            issues: Vec::new(),
        }
    }
}

/// Global health monitor instance.
pub static HEALTH_MONITOR: HealthMonitor = HealthMonitor::const_new();

/// Initialize the global health monitor.
pub fn init_health_monitor(interval_ms: u32) -> Result<(), &'static str> {
    let config = HealthMonitorConfig {
        check_interval_ms: interval_ms,
        ..HealthMonitorConfig::default()
    };
    
    HEALTH_MONITOR.init(config)
}

/// Cleanup health monitoring.
pub fn cleanup_health_monitor() {
    HEALTH_MONITOR.enabled.store(false, Ordering::Release);
}
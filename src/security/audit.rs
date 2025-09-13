//! Comprehensive audit logging for security events and thread operations.

use crate::errors::ThreadError;
use crate::security::{SecurityConfig, SecurityViolation};
use crate::thread_new::ThreadId;
use portable_atomic::{AtomicU64, AtomicUsize, Ordering};
use alloc::{collections::VecDeque, string::String, vec::Vec, format};
use core::fmt::Write;

/// Audit logging system for security and operational events.
pub struct AuditLogger {
    /// Event buffer for recent events
    event_buffer: VecDeque<AuditEvent>,
    /// Maximum events to keep in buffer
    max_buffer_size: usize,
    /// Total events logged
    events_logged: AtomicU64,
    /// Security violations logged
    security_violations: AtomicUsize,
    /// Log level filter
    log_level: AuditLevel,
    /// Enabled event categories
    enabled_categories: AuditCategories,
}

impl AuditLogger {
    pub fn new(config: AuditConfig) -> Self {
        Self {
            event_buffer: VecDeque::with_capacity(config.max_buffer_size),
            max_buffer_size: config.max_buffer_size,
            events_logged: AtomicU64::new(0),
            security_violations: AtomicUsize::new(0),
            log_level: config.log_level,
            enabled_categories: config.enabled_categories,
        }
    }
    
    /// Log a security violation event.
    pub fn log_security_violation(
        &mut self,
        violation: SecurityViolation,
        thread_id: Option<ThreadId>,
        details: &str,
    ) {
        if !self.enabled_categories.security {
            return;
        }
        
        let event = AuditEvent::new(
            AuditEventType::SecurityViolation {
                violation_type: violation,
                thread_id,
                details: String::from(details),
            },
            AuditLevel::Critical,
            self.get_current_context(),
        );
        
        self.log_event(event);
        self.security_violations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Log thread lifecycle event.
    pub fn log_thread_event(
        &mut self,
        thread_id: ThreadId,
        event_type: ThreadEventType,
        details: &str,
    ) {
        if !self.enabled_categories.thread_lifecycle {
            return;
        }
        
        let event = AuditEvent::new(
            AuditEventType::ThreadLifecycle {
                thread_id,
                event_type,
                details: String::from(details),
            },
            AuditLevel::Info,
            self.get_current_context(),
        );
        
        self.log_event(event);
    }
    
    /// Log memory operation event.
    pub fn log_memory_event(
        &mut self,
        operation: MemoryOperation,
        address: usize,
        size: usize,
        thread_id: Option<ThreadId>,
    ) {
        if !self.enabled_categories.memory_operations {
            return;
        }
        
        let event = AuditEvent::new(
            AuditEventType::MemoryOperation {
                operation,
                address,
                size,
                thread_id,
            },
            AuditLevel::Debug,
            self.get_current_context(),
        );
        
        self.log_event(event);
    }
    
    /// Log scheduler event.
    pub fn log_scheduler_event(
        &mut self,
        event_type: SchedulerEventType,
        thread_id: Option<ThreadId>,
        details: &str,
    ) {
        if !self.enabled_categories.scheduler_events {
            return;
        }
        
        let level = match event_type {
            SchedulerEventType::PreemptionOverride => AuditLevel::Warning,
            _ => AuditLevel::Debug,
        };
        
        let event = AuditEvent::new(
            AuditEventType::SchedulerEvent {
                event_type,
                thread_id,
                details: String::from(details),
            },
            level,
            self.get_current_context(),
        );
        
        self.log_event(event);
    }
    
    /// Log performance event.
    pub fn log_performance_event(
        &mut self,
        metric: PerformanceMetric,
        value: u64,
        threshold: Option<u64>,
    ) {
        if !self.enabled_categories.performance {
            return;
        }
        
        let level = if let Some(thresh) = threshold {
            if value > thresh {
                AuditLevel::Warning
            } else {
                AuditLevel::Debug
            }
        } else {
            AuditLevel::Debug
        };
        
        let event = AuditEvent::new(
            AuditEventType::Performance {
                metric,
                value,
                threshold,
            },
            level,
            self.get_current_context(),
        );
        
        self.log_event(event);
    }
    
    /// Log system event.
    pub fn log_system_event(&mut self, event_type: SystemEventType, details: &str) {
        if !self.enabled_categories.system_events {
            return;
        }
        
        let level = match event_type {
            SystemEventType::Initialization | SystemEventType::Shutdown => AuditLevel::Info,
            SystemEventType::ConfigurationChange => AuditLevel::Warning,
            SystemEventType::ResourceExhaustion => AuditLevel::Critical,
            _ => AuditLevel::Debug,
        };
        
        let event = AuditEvent::new(
            AuditEventType::System {
                event_type,
                details: String::from(details),
            },
            level,
            self.get_current_context(),
        );
        
        self.log_event(event);
    }
    
    /// Internal event logging.
    fn log_event(&mut self, event: AuditEvent) {
        // Check log level filter
        if event.level < self.log_level {
            return;
        }
        
        // Add to buffer
        if self.event_buffer.len() >= self.max_buffer_size {
            self.event_buffer.pop_front();
        }
        self.event_buffer.push_back(event);
        
        self.events_logged.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get current execution context for audit events.
    fn get_current_context(&self) -> AuditContext {
        AuditContext {
            timestamp: crate::time::get_monotonic_time().as_nanos() as u64,
            current_thread: crate::thread_new::current_thread_id(),
            cpu_id: 0, // Would be determined from current CPU
            interrupt_context: false, // Would check if in interrupt handler
        }
    }
    
    /// Get recent audit events.
    pub fn get_recent_events(&self, count: usize) -> Vec<AuditEvent> {
        self.event_buffer
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }
    
    /// Search events by criteria.
    pub fn search_events(&self, criteria: &SearchCriteria) -> Vec<AuditEvent> {
        self.event_buffer
            .iter()
            .filter(|event| criteria.matches(event))
            .cloned()
            .collect()
    }
    
    /// Export events in structured format.
    pub fn export_events(&self, format: ExportFormat) -> Result<String, ThreadError> {
        let mut output = String::new();
        
        match format {
            ExportFormat::Json => {
                writeln!(output, "[").map_err(|_| ThreadError::Other("Format error".into()))?;
                for (i, event) in self.event_buffer.iter().enumerate() {
                    if i > 0 {
                        writeln!(output, ",").map_err(|_| ThreadError::Other("Format error".into()))?;
                    }
                    writeln!(output, "  {}", event.to_json()).map_err(|_| ThreadError::Other("Format error".into()))?;
                }
                writeln!(output, "]").map_err(|_| ThreadError::Other("Format error".into()))?;
            },
            ExportFormat::Csv => {
                writeln!(output, "timestamp,level,category,thread_id,details").map_err(|_| ThreadError::Other("Format error".into()))?;
                for event in &self.event_buffer {
                    writeln!(output, "{}", event.to_csv()).map_err(|_| ThreadError::Other("Format error".into()))?;
                }
            },
            ExportFormat::Plain => {
                for event in &self.event_buffer {
                    writeln!(output, "{}", event).map_err(|_| ThreadError::Other("Format error".into()))?;
                }
            },
        }
        
        Ok(output)
    }
}

/// Individual audit event.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub event_type: AuditEventType,
    pub level: AuditLevel,
    pub context: AuditContext,
}

impl AuditEvent {
    fn new(event_type: AuditEventType, level: AuditLevel, context: AuditContext) -> Self {
        Self {
            event_type,
            level,
            context,
        }
    }
    
    /// Convert event to JSON format.
    fn to_json(&self) -> String {
        format!(
            "{{\"timestamp\":{},\"level\":\"{:?}\",\"thread_id\":{},\"event\":\"{}\"}}",
            self.context.timestamp,
            self.level,
            self.context.current_thread,
            self.event_type.description()
        )
    }
    
    /// Convert event to CSV format.
    fn to_csv(&self) -> String {
        format!(
            "{},{:?},{},{},\"{}\"",
            self.context.timestamp,
            self.level,
            self.event_type.category(),
            self.context.current_thread,
            self.event_type.description().replace("\"", "\"\"")
        )
    }
}

impl core::fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[{}] {:?} [T:{}] {}",
            self.context.timestamp,
            self.level,
            self.context.current_thread,
            self.event_type.description()
        )
    }
}

/// Types of audit events.
#[derive(Debug, Clone)]
pub enum AuditEventType {
    SecurityViolation {
        violation_type: SecurityViolation,
        thread_id: Option<ThreadId>,
        details: String,
    },
    ThreadLifecycle {
        thread_id: ThreadId,
        event_type: ThreadEventType,
        details: String,
    },
    MemoryOperation {
        operation: MemoryOperation,
        address: usize,
        size: usize,
        thread_id: Option<ThreadId>,
    },
    SchedulerEvent {
        event_type: SchedulerEventType,
        thread_id: Option<ThreadId>,
        details: String,
    },
    Performance {
        metric: PerformanceMetric,
        value: u64,
        threshold: Option<u64>,
    },
    System {
        event_type: SystemEventType,
        details: String,
    },
}

impl AuditEventType {
    fn category(&self) -> &'static str {
        match self {
            AuditEventType::SecurityViolation { .. } => "security",
            AuditEventType::ThreadLifecycle { .. } => "thread",
            AuditEventType::MemoryOperation { .. } => "memory",
            AuditEventType::SchedulerEvent { .. } => "scheduler",
            AuditEventType::Performance { .. } => "performance",
            AuditEventType::System { .. } => "system",
        }
    }
    
    fn description(&self) -> String {
        match self {
            AuditEventType::SecurityViolation { violation_type, details, .. } => {
                format!("Security violation: {:?} - {}", violation_type, details)
            },
            AuditEventType::ThreadLifecycle { event_type, details, .. } => {
                format!("Thread {:?}: {}", event_type, details)
            },
            AuditEventType::MemoryOperation { operation, address, size, .. } => {
                format!("Memory {:?}: addr=0x{:x} size={}", operation, address, size)
            },
            AuditEventType::SchedulerEvent { event_type, details, .. } => {
                format!("Scheduler {:?}: {}", event_type, details)
            },
            AuditEventType::Performance { metric, value, threshold } => {
                if let Some(thresh) = threshold {
                    format!("Performance {:?}: {} (threshold: {})", metric, value, thresh)
                } else {
                    format!("Performance {:?}: {}", metric, value)
                }
            },
            AuditEventType::System { event_type, details } => {
                format!("System {:?}: {}", event_type, details)
            },
        }
    }
}

/// Audit event severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditLevel {
    Debug,
    Info,
    Warning,
    Critical,
}

/// Execution context for audit events.
#[derive(Debug, Clone)]
pub struct AuditContext {
    pub timestamp: u64,
    pub current_thread: ThreadId,
    pub cpu_id: u32,
    pub interrupt_context: bool,
}

/// Thread lifecycle event types.
#[derive(Debug, Clone, Copy)]
pub enum ThreadEventType {
    Created,
    Started,
    Suspended,
    Resumed,
    Terminated,
    Joined,
    Detached,
}

/// Memory operation types for auditing.
#[derive(Debug, Clone, Copy)]
pub enum MemoryOperation {
    Allocate,
    Deallocate,
    Protect,
    Unprotect,
    Map,
    Unmap,
    Access,
}

/// Scheduler event types.
#[derive(Debug, Clone, Copy)]
pub enum SchedulerEventType {
    ContextSwitch,
    Preemption,
    PreemptionOverride,
    ThreadMigration,
    PriorityChange,
    AffinityChange,
}

/// Performance metrics for auditing.
#[derive(Debug, Clone, Copy)]
pub enum PerformanceMetric {
    ContextSwitchTime,
    SchedulingLatency,
    MemoryUsage,
    CpuUtilization,
    ThreadCount,
    LockContention,
}

/// System event types.
#[derive(Debug, Clone, Copy)]
pub enum SystemEventType {
    Initialization,
    Shutdown,
    ConfigurationChange,
    ResourceExhaustion,
    HardwareEvent,
    ErrorRecovery,
}

/// Audit configuration.
#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub log_level: AuditLevel,
    pub max_buffer_size: usize,
    pub enabled_categories: AuditCategories,
    pub export_format: ExportFormat,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_level: AuditLevel::Info,
            max_buffer_size: 10000,
            enabled_categories: AuditCategories::default(),
            export_format: ExportFormat::Plain,
        }
    }
}

/// Enabled audit categories.
#[derive(Debug, Clone)]
pub struct AuditCategories {
    pub security: bool,
    pub thread_lifecycle: bool,
    pub memory_operations: bool,
    pub scheduler_events: bool,
    pub performance: bool,
    pub system_events: bool,
}

impl Default for AuditCategories {
    fn default() -> Self {
        Self {
            security: true,
            thread_lifecycle: true,
            memory_operations: false, // Too verbose by default
            scheduler_events: false,  // Too verbose by default
            performance: true,
            system_events: true,
        }
    }
}

/// Event search criteria.
#[derive(Debug)]
pub struct SearchCriteria {
    pub level_filter: Option<AuditLevel>,
    pub category_filter: Option<String>,
    pub thread_filter: Option<ThreadId>,
    pub time_range: Option<(u64, u64)>,
}

impl SearchCriteria {
    fn matches(&self, event: &AuditEvent) -> bool {
        if let Some(level) = self.level_filter {
            if event.level < level {
                return false;
            }
        }
        
        if let Some(ref category) = self.category_filter {
            if event.event_type.category() != category {
                return false;
            }
        }
        
        if let Some(thread_id) = self.thread_filter {
            if event.context.current_thread != thread_id {
                return false;
            }
        }
        
        if let Some((start, end)) = self.time_range {
            if event.context.timestamp < start || event.context.timestamp > end {
                return false;
            }
        }
        
        true
    }
}

/// Export formats for audit events.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Csv,
    Plain,
}

/// Audit statistics.
#[derive(Debug, Clone)]
pub struct AuditStats {
    pub events_logged: u64,
    pub security_violations: usize,
    pub buffer_size: usize,
    pub max_buffer_size: usize,
    pub audit_enabled: bool,
}

/// Global audit logger instance.
static mut AUDIT_LOGGER: Option<AuditLogger> = None;

/// Initialize audit logging subsystem.
pub fn init_audit_logging(config: SecurityConfig) -> Result<(), ThreadError> {
    let audit_config = AuditConfig {
        log_level: if cfg!(debug_assertions) {
            AuditLevel::Debug
        } else {
            AuditLevel::Info
        },
        max_buffer_size: 10000,
        enabled_categories: AuditCategories::default(),
        export_format: ExportFormat::Plain,
    };
    
    unsafe {
        AUDIT_LOGGER = Some(AuditLogger::new(audit_config));
    }
    
    log_system_event(SystemEventType::Initialization, "Audit logging system initialized");
    // Initialization completed
    
    Ok(())
}

/// Log security violation (called from security violation handler).
pub fn log_security_violation(violation: SecurityViolation) {
    let thread_id = Some(crate::thread_new::current_thread_id());
    let details = format!("Security violation detected in thread {}", thread_id.unwrap());
    
    unsafe {
        if let Some(logger) = &mut AUDIT_LOGGER {
            logger.log_security_violation(violation, thread_id, &details);
        }
    }
}

/// Log thread lifecycle event.
pub fn log_thread_event(thread_id: ThreadId, event_type: ThreadEventType, details: &str) {
    unsafe {
        if let Some(logger) = &mut AUDIT_LOGGER {
            logger.log_thread_event(thread_id, event_type, details);
        }
    }
}

/// Log memory operation.
pub fn log_memory_event(operation: MemoryOperation, address: usize, size: usize) {
    let thread_id = Some(crate::thread_new::current_thread_id());
    
    unsafe {
        if let Some(logger) = &mut AUDIT_LOGGER {
            logger.log_memory_event(operation, address, size, thread_id);
        }
    }
}

/// Log scheduler event.
pub fn log_scheduler_event(event_type: SchedulerEventType, thread_id: Option<ThreadId>, details: &str) {
    unsafe {
        if let Some(logger) = &mut AUDIT_LOGGER {
            logger.log_scheduler_event(event_type, thread_id, details);
        }
    }
}

/// Log performance event.
pub fn log_performance_event(metric: PerformanceMetric, value: u64, threshold: Option<u64>) {
    unsafe {
        if let Some(logger) = &mut AUDIT_LOGGER {
            logger.log_performance_event(metric, value, threshold);
        }
    }
}

/// Log system event.
pub fn log_system_event(event_type: SystemEventType, details: &str) {
    unsafe {
        if let Some(logger) = &mut AUDIT_LOGGER {
            logger.log_system_event(event_type, details);
        }
    }
}

/// Get recent audit events.
pub fn get_recent_audit_events(count: usize) -> Vec<AuditEvent> {
    unsafe {
        match &AUDIT_LOGGER {
            Some(logger) => logger.get_recent_events(count),
            None => Vec::new(),
        }
    }
}

/// Search audit events.
pub fn search_audit_events(criteria: &SearchCriteria) -> Vec<AuditEvent> {
    unsafe {
        match &AUDIT_LOGGER {
            Some(logger) => logger.search_events(criteria),
            None => Vec::new(),
        }
    }
}

/// Export audit events.
pub fn export_audit_events(format: ExportFormat) -> Result<String, ThreadError> {
    unsafe {
        match &AUDIT_LOGGER {
            Some(logger) => logger.export_events(format),
            None => Ok(String::new()),
        }
    }
}

/// Get audit statistics.
pub fn get_audit_stats() -> AuditStats {
    unsafe {
        match &AUDIT_LOGGER {
            Some(logger) => AuditStats {
                events_logged: logger.events_logged.load(Ordering::Relaxed),
                security_violations: logger.security_violations.load(Ordering::Relaxed),
                buffer_size: logger.event_buffer.len(),
                max_buffer_size: logger.max_buffer_size,
                audit_enabled: true,
            },
            None => AuditStats {
                events_logged: 0,
                security_violations: 0,
                buffer_size: 0,
                max_buffer_size: 0,
                audit_enabled: false,
            },
        }
    }
}
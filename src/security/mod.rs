//! Security and hardening features for the threading system.
//!
//! This module provides comprehensive security mechanisms to protect against
//! various attack vectors and ensure system integrity in hostile environments.

pub mod stack_protection;
pub mod cfi;
pub mod isolation;
pub mod crypto_rng;
pub mod aslr;
pub mod audit;

use portable_atomic::{AtomicBool, AtomicU64, Ordering};
use crate::errors::ThreadError;

/// Global security configuration.
#[derive(Debug, Clone, Copy)]
pub struct SecurityConfig {
    /// Enable stack canaries for overflow detection
    pub enable_stack_canaries: bool,
    /// Enable stack guard pages
    pub enable_guard_pages: bool,
    /// Enable control flow integrity
    pub enable_cfi: bool,
    /// Enable thread isolation/sandboxing
    pub enable_thread_isolation: bool,
    /// Enable ASLR for thread stacks
    pub enable_aslr: bool,
    /// Enable comprehensive audit logging
    pub enable_audit_logging: bool,
    /// Cryptographically secure RNG for security features
    pub use_secure_rng: bool,
    /// Panic on security violations
    pub panic_on_violation: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_stack_canaries: cfg!(feature = "hardened"),
            enable_guard_pages: cfg!(feature = "mmu"),
            enable_cfi: cfg!(feature = "hardened"),
            enable_thread_isolation: false, // Expensive, opt-in only
            enable_aslr: cfg!(feature = "hardened"),
            enable_audit_logging: cfg!(debug_assertions),
            use_secure_rng: true,
            panic_on_violation: cfg!(debug_assertions),
        }
    }
}

/// Security violation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityViolation {
    /// Stack overflow detected via canary
    StackCanaryViolation,
    /// Stack guard page accessed
    GuardPageViolation,
    /// Control flow integrity violation
    CfiViolation,
    /// Thread isolation boundary crossed
    IsolationViolation,
    /// Invalid memory access detected
    MemoryViolation,
    /// Resource limit exceeded
    ResourceViolation,
    /// Cryptographic operation failed
    CryptoViolation,
}

/// Security violation handler result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationResponse {
    /// Continue execution (violation was handled)
    Continue,
    /// Terminate the violating thread
    TerminateThread,
    /// Panic the entire system
    Panic,
}

/// Global security state and statistics.
#[repr(align(64))] // Cache line aligned
pub struct SecurityState {
    /// Total security violations detected
    pub total_violations: AtomicU64,
    /// Violations by type
    pub stack_violations: AtomicU64,
    pub guard_violations: AtomicU64,
    pub cfi_violations: AtomicU64,
    pub isolation_violations: AtomicU64,
    pub memory_violations: AtomicU64,
    pub resource_violations: AtomicU64,
    pub crypto_violations: AtomicU64,
    
    /// Security features enabled
    pub canaries_enabled: AtomicBool,
    pub guards_enabled: AtomicBool,
    pub cfi_enabled: AtomicBool,
    pub isolation_enabled: AtomicBool,
    pub aslr_enabled: AtomicBool,
    pub audit_enabled: AtomicBool,
    
    /// Configuration
    config: SecurityConfig,
}

impl SecurityState {
    pub const fn new(config: SecurityConfig) -> Self {
        Self {
            total_violations: AtomicU64::new(0),
            stack_violations: AtomicU64::new(0),
            guard_violations: AtomicU64::new(0),
            cfi_violations: AtomicU64::new(0),
            isolation_violations: AtomicU64::new(0),
            memory_violations: AtomicU64::new(0),
            resource_violations: AtomicU64::new(0),
            crypto_violations: AtomicU64::new(0),
            canaries_enabled: AtomicBool::new(config.enable_stack_canaries),
            guards_enabled: AtomicBool::new(config.enable_guard_pages),
            cfi_enabled: AtomicBool::new(config.enable_cfi),
            isolation_enabled: AtomicBool::new(config.enable_thread_isolation),
            aslr_enabled: AtomicBool::new(config.enable_aslr),
            audit_enabled: AtomicBool::new(config.enable_audit_logging),
            config,
        }
    }
    
    /// Record a security violation.
    pub fn record_violation(&self, violation: SecurityViolation) -> ViolationResponse {
        // Update statistics
        self.total_violations.fetch_add(1, Ordering::Relaxed);
        
        match violation {
            SecurityViolation::StackCanaryViolation => {
                self.stack_violations.fetch_add(1, Ordering::Relaxed);
            }
            SecurityViolation::GuardPageViolation => {
                self.guard_violations.fetch_add(1, Ordering::Relaxed);
            }
            SecurityViolation::CfiViolation => {
                self.cfi_violations.fetch_add(1, Ordering::Relaxed);
            }
            SecurityViolation::IsolationViolation => {
                self.isolation_violations.fetch_add(1, Ordering::Relaxed);
            }
            SecurityViolation::MemoryViolation => {
                self.memory_violations.fetch_add(1, Ordering::Relaxed);
            }
            SecurityViolation::ResourceViolation => {
                self.resource_violations.fetch_add(1, Ordering::Relaxed);
            }
            SecurityViolation::CryptoViolation => {
                self.crypto_violations.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        // Log violation if audit is enabled
        if self.audit_enabled.load(Ordering::Relaxed) {
            audit::log_security_violation(violation);
        }
        
        // Determine response
        if self.config.panic_on_violation {
            ViolationResponse::Panic
        } else {
            match violation {
                SecurityViolation::StackCanaryViolation |
                SecurityViolation::GuardPageViolation |
                SecurityViolation::CfiViolation => ViolationResponse::TerminateThread,
                _ => ViolationResponse::Continue,
            }
        }
    }
    
    /// Get security statistics.
    pub fn get_stats(&self) -> SecurityStats {
        SecurityStats {
            total_violations: self.total_violations.load(Ordering::Relaxed),
            stack_violations: self.stack_violations.load(Ordering::Relaxed),
            guard_violations: self.guard_violations.load(Ordering::Relaxed),
            cfi_violations: self.cfi_violations.load(Ordering::Relaxed),
            isolation_violations: self.isolation_violations.load(Ordering::Relaxed),
            memory_violations: self.memory_violations.load(Ordering::Relaxed),
            resource_violations: self.resource_violations.load(Ordering::Relaxed),
            crypto_violations: self.crypto_violations.load(Ordering::Relaxed),
            features_enabled: FeaturesEnabled {
                canaries: self.canaries_enabled.load(Ordering::Relaxed),
                guard_pages: self.guards_enabled.load(Ordering::Relaxed),
                cfi: self.cfi_enabled.load(Ordering::Relaxed),
                isolation: self.isolation_enabled.load(Ordering::Relaxed),
                aslr: self.aslr_enabled.load(Ordering::Relaxed),
                audit: self.audit_enabled.load(Ordering::Relaxed),
            },
        }
    }
}

/// Security statistics.
#[derive(Debug, Clone)]
pub struct SecurityStats {
    pub total_violations: u64,
    pub stack_violations: u64,
    pub guard_violations: u64,
    pub cfi_violations: u64,
    pub isolation_violations: u64,
    pub memory_violations: u64,
    pub resource_violations: u64,
    pub crypto_violations: u64,
    pub features_enabled: FeaturesEnabled,
}

#[derive(Debug, Clone)]
pub struct FeaturesEnabled {
    pub canaries: bool,
    pub guard_pages: bool,
    pub cfi: bool,
    pub isolation: bool,
    pub aslr: bool,
    pub audit: bool,
}

/// Global security state instance.
pub static SECURITY_STATE: SecurityState = SecurityState::new(SecurityConfig {
    enable_stack_canaries: cfg!(feature = "hardened"),
    enable_guard_pages: cfg!(feature = "mmu"),
    enable_cfi: cfg!(feature = "hardened"),
    enable_thread_isolation: false,
    enable_aslr: cfg!(feature = "hardened"),
    enable_audit_logging: cfg!(debug_assertions),
    use_secure_rng: true,
    panic_on_violation: cfg!(debug_assertions),
});

/// Initialize security subsystem.
pub fn init_security(config: SecurityConfig) -> Result<(), ThreadError> {
    // Initialize stack protection
    if config.enable_stack_canaries || config.enable_guard_pages {
        stack_protection::init_stack_protection(config)?;
    }
    
    // Initialize CFI
    if config.enable_cfi {
        cfi::init_cfi_protection(config)?;
    }
    
    // Initialize thread isolation
    if config.enable_thread_isolation {
        isolation::init_thread_isolation(config)?;
    }
    
    // Initialize secure RNG
    if config.use_secure_rng {
        crypto_rng::init_secure_rng()?;
    }
    
    // Initialize ASLR
    if config.enable_aslr {
        aslr::init_aslr(config)?;
    }
    
    // Initialize audit logging
    if config.enable_audit_logging {
        audit::init_audit_logging(config)?;
    }
    
    // Security subsystem initialized with feature count based on config
    
    Ok(())
}

/// Count enabled security features.
fn count_enabled_features(config: &SecurityConfig) -> usize {
    let mut count = 0;
    if config.enable_stack_canaries { count += 1; }
    if config.enable_guard_pages { count += 1; }
    if config.enable_cfi { count += 1; }
    if config.enable_thread_isolation { count += 1; }
    if config.enable_aslr { count += 1; }
    if config.enable_audit_logging { count += 1; }
    if config.use_secure_rng { count += 1; }
    count
}

/// Security violation handler (called from various subsystems).
pub fn handle_security_violation(violation: SecurityViolation) -> ! {
    let response = SECURITY_STATE.record_violation(violation);
    
    match response {
        ViolationResponse::Continue => {
            // This shouldn't happen for serious violations
            panic!("Security violation handler requested continue for serious violation: {:?}", violation);
        }
        ViolationResponse::TerminateThread => {
            // Terminate current thread
            crate::exit_thread();
            unreachable!("Thread should have been terminated");
        }
        ViolationResponse::Panic => {
            panic!("Security violation detected: {:?}", violation);
        }
    }
}

/// Get current security statistics.
pub fn get_security_stats() -> SecurityStats {
    SECURITY_STATE.get_stats()
}

/// Enable/disable security features at runtime.
pub fn configure_security_feature(feature: SecurityFeature, enabled: bool) {
    match feature {
        SecurityFeature::StackCanaries => {
            SECURITY_STATE.canaries_enabled.store(enabled, Ordering::Relaxed);
        }
        SecurityFeature::GuardPages => {
            SECURITY_STATE.guards_enabled.store(enabled, Ordering::Relaxed);
        }
        SecurityFeature::Cfi => {
            SECURITY_STATE.cfi_enabled.store(enabled, Ordering::Relaxed);
        }
        SecurityFeature::Isolation => {
            SECURITY_STATE.isolation_enabled.store(enabled, Ordering::Relaxed);
        }
        SecurityFeature::Aslr => {
            SECURITY_STATE.aslr_enabled.store(enabled, Ordering::Relaxed);
        }
        SecurityFeature::Audit => {
            SECURITY_STATE.audit_enabled.store(enabled, Ordering::Relaxed);
        }
    }
}

/// Security features that can be configured at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityFeature {
    StackCanaries,
    GuardPages,
    Cfi,
    Isolation,
    Aslr,
    Audit,
}
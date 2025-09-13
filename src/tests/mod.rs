//! Comprehensive testing battery for the preemptive threading system.
//!
//! This module provides extensive testing infrastructure including unit tests,
//! integration tests, stress tests, property-based tests, and fuzzing support.

#[cfg(test)]
pub mod unit;

#[cfg(test)]
pub mod integration;

#[cfg(test)]
pub mod stress;

#[cfg(test)]
pub mod property;

#[cfg(test)]
pub mod performance;

#[cfg(test)]
pub mod helpers;

/// Test configuration for controlling test behavior.
#[cfg(test)]
pub struct TestConfig {
    /// Number of threads to use in stress tests
    pub stress_thread_count: usize,
    /// Duration for stress tests in seconds
    pub stress_duration_secs: u64,
    /// Number of iterations for performance tests
    pub perf_iterations: usize,
    /// Enable verbose output
    pub verbose: bool,
    /// Random seed for reproducible tests
    pub seed: u64,
}

#[cfg(test)]
impl Default for TestConfig {
    fn default() -> Self {
        Self {
            stress_thread_count: 100,
            stress_duration_secs: 10,
            perf_iterations: 10000,
            verbose: false,
            seed: 0x12345678,
        }
    }
}

#[cfg(test)]
impl TestConfig {
    /// Create a config for quick tests during development.
    pub fn quick() -> Self {
        Self {
            stress_thread_count: 10,
            stress_duration_secs: 1,
            perf_iterations: 100,
            verbose: true,
            seed: 0x12345678,
        }
    }
    
    /// Create a config for thorough CI testing.
    pub fn ci() -> Self {
        Self {
            stress_thread_count: 50,
            stress_duration_secs: 30,
            perf_iterations: 5000,
            verbose: false,
            seed: 0x87654321,
        }
    }
}

/// Global test configuration.
#[cfg(test)]
pub static TEST_CONFIG: spin::Mutex<TestConfig> = spin::Mutex::new(TestConfig {
    stress_thread_count: 100,
    stress_duration_secs: 10,
    perf_iterations: 10000,
    verbose: false,
    seed: 0x12345678,
});
//! x86_64-specific timer implementation using APIC timer.

use super::{Duration, Instant, Timer, TimerConfig};
use super::timer::TimerError;
use core::arch::asm;
use portable_atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// x86_64 APIC timer implementation.
///
/// This uses the Local APIC timer for high-precision, per-CPU timer interrupts.
/// The APIC timer is preferred over PIT or HPET because it's per-CPU and has
/// better precision characteristics.
pub struct ApicTimer {
    /// Whether the timer is initialized
    initialized: AtomicBool,
    /// Whether the timer is currently running
    running: AtomicBool,
    /// Timer frequency in Hz
    frequency: AtomicU32,
    /// Timer vector number
    vector: AtomicU32,
    /// CPU frequency for TSC calculations
    tsc_frequency: AtomicU64,
}

impl ApicTimer {
    /// Create a new APIC timer instance.
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            running: AtomicBool::new(false),
            frequency: AtomicU32::new(0),
            vector: AtomicU32::new(0),
            tsc_frequency: AtomicU64::new(0),
        }
    }
    
    /// Read the Time Stamp Counter (TSC).
    ///
    /// The TSC provides a high-resolution monotonic counter that can be
    /// used for precise timing measurements.
    fn read_tsc() -> u64 {
        let low: u32;
        let high: u32;
        unsafe {
            asm!(
                "rdtsc",
                out("eax") low,
                out("edx") high,
                options(nomem, nostack, preserves_flags)
            );
        }
        ((high as u64) << 32) | (low as u64)
    }
    
    /// Calibrate the TSC frequency by measuring against the PIT.
    ///
    /// This uses the PIT (Programmable Interval Timer) as a reference
    /// to measure the actual TSC frequency.
    fn calibrate_tsc_frequency(&self) -> u64 {
        const PIT_FREQUENCY: u64 = 1193182; // PIT frequency in Hz
        const CALIBRATION_MS: u64 = 50; // Calibrate for 50ms
        
        unsafe {
            // Set up PIT for one-shot mode
            let pit_count = (PIT_FREQUENCY * CALIBRATION_MS) / 1000;
            
            // Program PIT channel 2 for one-shot
            core::arch::asm!("out dx, al", in("dx") 0x43u16, in("al") 0xB0u8); // Mode 0, binary
            core::arch::asm!("out dx, al", in("dx") 0x42u16, in("al") (pit_count & 0xFF) as u8);
            core::arch::asm!("out dx, al", in("dx") 0x42u16, in("al") ((pit_count >> 8) & 0xFF) as u8);
            
            // Read TSC before
            let tsc_start = Self::read_tsc();
            
            // Start PIT and wait for completion
            core::arch::asm!("out dx, al", in("dx") 0x61u16, in("al") 0x01u8); // Enable PIT gate
            
            // Wait for PIT to count down (simplified busy wait)
            let mut pit_status: u8;
            loop {
                core::arch::asm!("in al, dx", out("al") pit_status, in("dx") 0x42u16);
                if pit_status == 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            
            // Read TSC after
            let tsc_end = Self::read_tsc();
            
            // Calculate frequency: TSC_counts / time_in_seconds
            let tsc_counts = tsc_end - tsc_start;
            let frequency = (tsc_counts * 1000) / CALIBRATION_MS;
            
            // Sanity check: frequency should be reasonable (1GHz to 5GHz)
            if frequency < 1_000_000_000 || frequency > 5_000_000_000 {
                // Fall back to CPUID if available, or reasonable default
                return 3_000_000_000;
            }
            
            frequency
        }
    }
    
    /// Get the APIC base address from IA32_APIC_BASE MSR.
    ///
    /// # Safety
    ///
    /// Must be called on a CPU that supports APIC.
    unsafe fn get_apic_base() -> u64 {
        let mut low: u32;
        let mut high: u32;
        unsafe {
            asm!(
                "rdmsr",
                in("ecx") 0x1B, // IA32_APIC_BASE MSR
                out("eax") low,
                out("edx") high,
                options(nomem, nostack, preserves_flags)
            );
        }
        ((high as u64) << 32) | (low as u64) & !0xFFF // Mask off lower 12 bits
    }

    /// Read from a Local APIC register.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the APIC base address is valid
    /// and that the register offset is correct.
    unsafe fn read_apic_register(offset: u32) -> u32 {
        unsafe {
            let apic_base = Self::get_apic_base();
            let reg_addr = (apic_base + offset as u64) as *const u32;
            core::ptr::read_volatile(reg_addr)
        }
    }
    
    /// Write to a Local APIC register.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the APIC base address is valid,
    /// that the register offset is correct, and that the value is appropriate.
    unsafe fn write_apic_register(offset: u32, value: u32) {
        unsafe {
            let apic_base = Self::get_apic_base();
            let reg_addr = (apic_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(reg_addr, value);
        }
    }
    
    /// Set up the APIC timer with the given configuration.
    ///
    /// # Safety
    ///
    /// This function modifies APIC timer registers and sets up interrupt vectors.
    /// It must be called with interrupts disabled during system initialization.
    unsafe fn setup_apic_timer(&self, config: &TimerConfig) -> Result<(), TimerError> {
        unsafe {
            // Set up Local Vector Table (LVT) Timer Entry
            let lvt_timer = (config.vector as u32) | (0 << 16); // Periodic mode
            Self::write_apic_register(0x320, lvt_timer);
            
            // Calculate timer initial count for desired frequency
            let apic_frequency = self.get_apic_bus_frequency();
            let initial_count = apic_frequency / config.frequency;
            
            // Set divide configuration register (divide by 1)
            Self::write_apic_register(0x3E0, 0x0B);
            
            // Set initial count
            Self::write_apic_register(0x380, initial_count);
        }
        
        Ok(())
    }
    
    /// Get the APIC bus frequency by calibration.
    ///
    /// This measures the APIC timer frequency against the TSC.
    fn get_apic_bus_frequency(&self) -> u32 {
        unsafe {
            // Save current timer settings
            let saved_divide = Self::read_apic_register(0x3E0);
            let saved_lvt = Self::read_apic_register(0x320);
            
            // Set divide by 1 and mask interrupts
            Self::write_apic_register(0x3E0, 0x0B); // Divide by 1
            Self::write_apic_register(0x320, 0x10000); // Masked
            
            // Set initial count to maximum
            Self::write_apic_register(0x380, 0xFFFFFFFF);
            
            // Wait a short period using TSC
            let tsc_start = Self::read_tsc();
            let tsc_freq = self.tsc_frequency.load(Ordering::Acquire);
            let wait_tsc_counts = tsc_freq / 1000; // Wait ~1ms
            
            while (Self::read_tsc() - tsc_start) < wait_tsc_counts {
                core::hint::spin_loop();
            }
            
            // Read how much the APIC timer counted down
            let apic_end = Self::read_apic_register(0x390);
            let apic_counts = 0xFFFFFFFF - apic_end;
            
            // APIC frequency = counts_per_millisecond * 1000
            let apic_frequency = apic_counts * 1000;
            
            // Restore timer settings
            Self::write_apic_register(0x380, 0); // Stop timer
            Self::write_apic_register(0x3E0, saved_divide);
            Self::write_apic_register(0x320, saved_lvt);
            
            apic_frequency
        }
    }
}

impl Timer for ApicTimer {
    unsafe fn init(&mut self, config: &TimerConfig) -> Result<(), TimerError> {
        if self.initialized.load(Ordering::Acquire) {
            return Err(TimerError::AlreadyRunning);
        }
        
        // Validate configuration
        if config.frequency == 0 || config.frequency > 10000 {
            return Err(TimerError::UnsupportedFrequency);
        }
        
        // Calibrate TSC frequency
        let tsc_freq = self.calibrate_tsc_frequency();
        self.tsc_frequency.store(tsc_freq, Ordering::Release);
        
        // Set up APIC timer
        unsafe {
            self.setup_apic_timer(config)?;
        }
        
        // Store configuration
        self.frequency.store(config.frequency, Ordering::Release);
        self.vector.store(config.vector as u32, Ordering::Release);
        self.initialized.store(true, Ordering::Release);
        
        Ok(())
    }
    
    fn start(&mut self) -> Result<(), TimerError> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(TimerError::NotInitialized);
        }
        
        if self.running.load(Ordering::Acquire) {
            return Err(TimerError::AlreadyRunning);
        }
        
        // The APIC timer starts automatically when initial count is set
        // We just need to mark it as running
        self.running.store(true, Ordering::Release);
        
        Ok(())
    }
    
    fn stop(&mut self) -> Result<(), TimerError> {
        if !self.running.load(Ordering::Acquire) {
            return Err(TimerError::NotRunning);
        }
        
        // Stop the timer by setting initial count to 0
        unsafe {
            Self::write_apic_register(0x380, 0);
        }
        
        self.running.store(false, Ordering::Release);
        Ok(())
    }
    
    fn current_count(&self) -> u64 {
        if !self.initialized.load(Ordering::Acquire) {
            return 0;
        }
        
        // Read current count from APIC timer
        unsafe { Self::read_apic_register(0x390) as u64 }
    }
    
    fn counts_to_nanos(&self, counts: u64) -> u64 {
        let freq = self.frequency.load(Ordering::Acquire);
        if freq == 0 {
            return 0;
        }
        
        // Convert APIC timer counts to nanoseconds
        let apic_freq = self.get_apic_bus_frequency() as u64;
        (counts * 1_000_000_000) / apic_freq
    }
    
    fn nanos_to_counts(&self, nanos: u64) -> u64 {
        let apic_freq = self.get_apic_bus_frequency() as u64;
        (nanos * apic_freq) / 1_000_000_000
    }
    
    fn set_oneshot(&mut self, duration: Duration) -> Result<(), TimerError> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(TimerError::NotInitialized);
        }
        
        let counts = self.nanos_to_counts(duration.as_nanos());
        
        unsafe {
            // Set to one-shot mode
            let vector = self.vector.load(Ordering::Acquire);
            let lvt_timer = vector | (0b00 << 17); // One-shot mode
            Self::write_apic_register(0x320, lvt_timer);
            
            // Set initial count for one-shot
            Self::write_apic_register(0x380, counts as u32);
        }
        
        Ok(())
    }
}

/// Read the current TSC value and convert to an Instant.
///
/// This provides high-resolution timing for the scheduler and other
/// time-sensitive operations.
pub fn read_tsc() -> Instant {
    let tsc = ApicTimer::read_tsc();
    
    // Convert TSC to nanoseconds using calibrated frequency
    unsafe {
        let tsc_freq = APIC_TIMER.tsc_frequency.load(Ordering::Acquire);
        if tsc_freq > 0 {
            // Convert: (tsc_counts * 1_000_000_000) / tsc_frequency_hz
            let nanos = (tsc * 1_000_000_000) / tsc_freq;
            Instant::from_nanos(nanos)
        } else {
            // Fallback if not calibrated yet
            Instant::from_nanos(tsc / 3_000_000) // Assume ~3GHz
        }
    }
}

/// Global APIC timer instance.
pub static mut APIC_TIMER: ApicTimer = ApicTimer::new();

/// Initialize the x86_64 timer subsystem.
///
/// # Safety
///
/// Must be called once during system initialization with interrupts disabled.
pub unsafe fn init() -> Result<(), TimerError> {
    let config = TimerConfig::default();
    unsafe {
        APIC_TIMER.init(&config)?;
        APIC_TIMER.start()?;
    }
    Ok(())
}

/// x86_64 timer interrupt handler.
///
/// This should be called from the interrupt service routine for the
/// timer vector. It handles the low-level timer interrupt and calls
/// the generic timer handling code.
///
/// # Safety
///
/// Must be called from the timer interrupt context with a valid
/// interrupt frame. The caller must preserve all register state.
pub unsafe extern "C" fn timer_interrupt_handler() {
    // Call the Rust portion of the handler
    timer_interrupt_rust_handler();
    
    // Send EOI (End of Interrupt) to APIC
    unsafe {
        ApicTimer::write_apic_register(0xB0, 0); // EOI register
    }
}

/// Rust portion of the timer interrupt handler.
extern "C" fn timer_interrupt_rust_handler() {
    unsafe {
        super::timer::handle_timer_interrupt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_apic_timer_creation() {
        let timer = ApicTimer::new();
        assert!(!timer.initialized.load(Ordering::Acquire));
        assert!(!timer.running.load(Ordering::Acquire));
    }
    
    #[test]
    fn test_tsc_reading() {
        // TSC should be monotonic
        let tsc1 = ApicTimer::read_tsc();
        let tsc2 = ApicTimer::read_tsc();
        assert!(tsc2 >= tsc1);
    }
    
    #[cfg(feature = "std-shim")]
    #[test] 
    fn test_time_conversion() {
        let timer = ApicTimer::new();
        let nanos = 1_000_000_000; // 1 second
        let counts = timer.nanos_to_counts(nanos);
        let back_to_nanos = timer.counts_to_nanos(counts);
        
        // Should be approximately equal (allowing for rounding)
        let diff = if back_to_nanos > nanos {
            back_to_nanos - nanos
        } else {
            nanos - back_to_nanos
        };
        assert!(diff < 1000); // Within 1 microsecond
    }
}
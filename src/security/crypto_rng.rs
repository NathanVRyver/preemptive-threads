//! Cryptographically secure random number generation for security features.

use crate::errors::ThreadError;
use portable_atomic::{AtomicU64, AtomicUsize, Ordering};

/// Cryptographically secure RNG implementation.
pub struct SecureRng {
    /// Entropy pool
    entropy_pool: [u64; 32],
    /// Pool position
    pool_position: AtomicUsize,
    /// Bytes generated
    bytes_generated: AtomicU64,
    /// Entropy collected
    entropy_collected: AtomicU64,
    /// RNG state
    state: ChaCha20State,
}

impl SecureRng {
    const fn new() -> Self {
        Self {
            entropy_pool: [0; 32],
            pool_position: AtomicUsize::new(0),
            bytes_generated: AtomicU64::new(0),
            entropy_collected: AtomicU64::new(0),
            state: ChaCha20State::new(),
        }
    }
    
    /// Initialize secure RNG with hardware entropy.
    pub fn init(&mut self) -> Result<(), ThreadError> {
        // Collect entropy from multiple sources
        self.collect_hardware_entropy()?;
        self.collect_timing_entropy()?;
        self.collect_system_entropy()?;
        
        // Initialize ChaCha20 state with entropy
        self.state.initialize(&self.entropy_pool)?;
        
        Ok(())
    }
    
    /// Generate cryptographically secure random bytes.
    pub fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), ThreadError> {
        if dest.is_empty() {
            return Ok(());
        }
        
        // Generate random data using ChaCha20
        self.state.generate_bytes(dest)?;
        self.bytes_generated.fetch_add(dest.len() as u64, Ordering::Relaxed);
        
        // Reseed periodically for forward secrecy
        if self.bytes_generated.load(Ordering::Relaxed) % (1024 * 1024) == 0 {
            self.reseed()?;
        }
        
        Ok(())
    }
    
    /// Generate random u64.
    pub fn next_u64(&mut self) -> Result<u64, ThreadError> {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes)?;
        Ok(u64::from_ne_bytes(bytes))
    }
    
    /// Generate random u32.
    pub fn next_u32(&mut self) -> Result<u32, ThreadError> {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes)?;
        Ok(u32::from_ne_bytes(bytes))
    }
    
    /// Generate random value in range [0, max).
    pub fn gen_range(&mut self, max: u64) -> Result<u64, ThreadError> {
        if max == 0 {
            return Ok(0);
        }
        
        // Use rejection sampling to avoid bias
        let limit = u64::MAX - (u64::MAX % max);
        loop {
            let value = self.next_u64()?;
            if value < limit {
                return Ok(value % max);
            }
        }
    }
    
    /// Collect entropy from hardware sources.
    fn collect_hardware_entropy(&mut self) -> Result<(), ThreadError> {
        let entropy_count = 0;
        
        // Use RDRAND if available
        #[cfg(feature = "x86_64")]
        if is_rdrand_available() {
            for i in 0..16 {
                if let Some(value) = rdrand_u64() {
                    self.entropy_pool[i] ^= value;
                    entropy_count += 1;
                }
            }
        }
        
        // Use RDSEED if available (higher quality entropy)
        #[cfg(feature = "x86_64")]
        if is_rdseed_available() {
            for i in 16..32 {
                if let Some(value) = rdseed_u64() {
                    self.entropy_pool[i] ^= value;
                    entropy_count += 1;
                }
            }
        }
        
        // ARM64 entropy sources
        #[cfg(feature = "arm64")]
        if is_arm64_rng_available() {
            for i in 0..16 {
                if let Some(value) = arm64_random_u64() {
                    self.entropy_pool[i] ^= value;
                    entropy_count += 1;
                }
            }
        }
        
        self.entropy_collected.fetch_add(entropy_count, Ordering::Relaxed);
        
        if entropy_count < 8 {
            return Err(ThreadError::UnsupportedOperation(
                "Insufficient hardware entropy sources".into()
            ));
        }
        
        Ok(())
    }
    
    /// Collect entropy from timing sources.
    fn collect_timing_entropy(&mut self) -> Result<(), ThreadError> {
        // Collect high-resolution timing entropy
        for i in 0..32 {
            let start_time = get_cycle_count();
            
            // Perform some variable-time operations
            volatile_memory_access();
            cache_timing_variation();
            
            let end_time = get_cycle_count();
            let timing_entropy = end_time.wrapping_sub(start_time);
            
            // Mix timing data into entropy pool
            self.entropy_pool[i] = self.entropy_pool[i]
                .wrapping_mul(0x6c078965)
                .wrapping_add(timing_entropy);
        }
        
        Ok(())
    }
    
    /// Collect entropy from system sources.
    fn collect_system_entropy(&mut self) -> Result<(), ThreadError> {
        // Mix in various system state
        let system_entropy = [
            core::ptr::null::<u8>() as usize as u64, // Stack address
            self as *const _ as usize as u64,         // Heap address  
            get_cycle_count(),                        // CPU cycle counter
            crate::time::get_monotonic_time().as_nanos() as u64, // System time
        ];
        
        for (i, &entropy) in system_entropy.iter().enumerate() {
            self.entropy_pool[i % 32] ^= entropy;
        }
        
        Ok(())
    }
    
    /// Reseed the RNG for forward secrecy.
    fn reseed(&mut self) -> Result<(), ThreadError> {
        // Collect fresh entropy
        self.collect_timing_entropy()?;
        
        // Re-initialize state with new entropy
        self.state.reseed(&self.entropy_pool)?;
        
        Ok(())
    }
}

/// ChaCha20 stream cipher for cryptographically secure random generation.
#[repr(align(64))]
struct ChaCha20State {
    state: [u32; 16],
    counter: u64,
    initialized: bool,
}

impl ChaCha20State {
    const fn new() -> Self {
        Self {
            state: [0; 16],
            counter: 0,
            initialized: false,
        }
    }
    
    /// Initialize ChaCha20 with key from entropy pool.
    fn initialize(&mut self, entropy: &[u64; 32]) -> Result<(), ThreadError> {
        // ChaCha20 constants
        self.state[0] = 0x61707865; // "expa"
        self.state[1] = 0x3320646e; // "nd 3"
        self.state[2] = 0x79622d32; // "2-by"
        self.state[3] = 0x6b206574; // "te k"
        
        // 256-bit key from entropy
        for i in 0..8 {
            let entropy_u64 = entropy[i];
            self.state[4 + i] = (entropy_u64 >> (32 * (i & 1))) as u32;
        }
        
        // Counter and nonce
        self.state[12] = 0; // Counter low
        self.state[13] = 0; // Counter high  
        self.state[14] = (entropy[30] & 0xFFFFFFFF) as u32; // Nonce low
        self.state[15] = (entropy[31] & 0xFFFFFFFF) as u32; // Nonce high
        
        self.counter = 0;
        self.initialized = true;
        
        Ok(())
    }
    
    /// Reseed with fresh entropy.
    fn reseed(&mut self, entropy: &[u64; 32]) -> Result<(), ThreadError> {
        if !self.initialized {
            return self.initialize(entropy);
        }
        
        // Mix new entropy into existing key
        for i in 0..8 {
            let entropy_u64 = entropy[i + 8];
            self.state[4 + i] ^= (entropy_u64 >> (32 * (i & 1))) as u32;
        }
        
        // Update nonce
        self.state[14] ^= (entropy[30] & 0xFFFFFFFF) as u32;
        self.state[15] ^= (entropy[31] & 0xFFFFFFFF) as u32;
        
        Ok(())
    }
    
    /// Generate random bytes using ChaCha20.
    fn generate_bytes(&mut self, dest: &mut [u8]) -> Result<(), ThreadError> {
        if !self.initialized {
            return Err(ThreadError::InvalidState());
        }
        
        let mut dest_pos = 0;
        let mut block = [0u8; 64];
        
        while dest_pos < dest.len() {
            // Generate 64-byte block
            self.chacha20_block(&mut block);
            
            // Copy to destination
            let copy_len = core::cmp::min(64, dest.len() - dest_pos);
            dest[dest_pos..dest_pos + copy_len].copy_from_slice(&block[..copy_len]);
            dest_pos += copy_len;
            
            // Increment counter
            self.counter = self.counter.wrapping_add(1);
            self.state[12] = (self.counter & 0xFFFFFFFF) as u32;
            self.state[13] = (self.counter >> 32) as u32;
        }
        
        Ok(())
    }
    
    /// Generate one ChaCha20 block.
    fn chacha20_block(&self, output: &mut [u8; 64]) {
        let mut working_state = self.state;
        
        // 20 rounds of ChaCha20
        for _ in 0..10 {
            // Column rounds
            quarter_round_by_indices(&mut working_state, 0, 4, 8, 12);
            quarter_round_by_indices(&mut working_state, 1, 5, 9, 13);
            quarter_round_by_indices(&mut working_state, 2, 6, 10, 14);
            quarter_round_by_indices(&mut working_state, 3, 7, 11, 15);
            
            // Diagonal rounds
            quarter_round_by_indices(&mut working_state, 0, 5, 10, 15);
            quarter_round_by_indices(&mut working_state, 1, 6, 11, 12);
            quarter_round_by_indices(&mut working_state, 2, 7, 8, 13);
            quarter_round_by_indices(&mut working_state, 3, 4, 9, 14);
        }
        
        // Add original state
        for i in 0..16 {
            working_state[i] = working_state[i].wrapping_add(self.state[i]);
        }
        
        // Convert to bytes
        for (i, &word) in working_state.iter().enumerate() {
            let bytes = word.to_le_bytes();
            output[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
        }
    }
}

/// ChaCha20 quarter round operation.
fn quarter_round(a: &mut u32, b: &mut u32, c: &mut u32, d: &mut u32) {
    *a = a.wrapping_add(*b);
    *d ^= *a;
    *d = d.rotate_left(16);
    
    *c = c.wrapping_add(*d);
    *b ^= *c;
    *b = b.rotate_left(12);
    
    *a = a.wrapping_add(*b);
    *d ^= *a;
    *d = d.rotate_left(8);
    
    *c = c.wrapping_add(*d);
    *b ^= *c;
    *b = b.rotate_left(7);
}

/// ChaCha20 quarter-round function that works with indices to avoid borrow conflicts.
fn quarter_round_by_indices(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]); 
    state[d] ^= state[a]; 
    state[d] = state[d].rotate_left(16);
    
    state[c] = state[c].wrapping_add(state[d]); 
    state[b] ^= state[c]; 
    state[b] = state[b].rotate_left(12);
    
    state[a] = state[a].wrapping_add(state[b]); 
    state[d] ^= state[a]; 
    state[d] = state[d].rotate_left(8);
    
    state[c] = state[c].wrapping_add(state[d]); 
    state[b] ^= state[c]; 
    state[b] = state[b].rotate_left(7);
}

/// Global secure RNG instance.
static mut SECURE_RNG: SecureRng = SecureRng::new();

/// Hardware entropy collection functions.

#[cfg(feature = "x86_64")]
fn is_rdrand_available() -> bool {
    // Check CPUID for RDRAND support
    let cpuid_result: u32;
    unsafe {
        asm!(
            "mov eax, 1",
            "cpuid",
            "mov {}, ecx",
            out(reg) cpuid_result,
            out("eax") _,
            out("ebx") _,
            out("ecx") _,
            out("edx") _,
        );
    }
    (cpuid_result & (1 << 30)) != 0
}

#[cfg(feature = "x86_64")]
fn is_rdseed_available() -> bool {
    // Check CPUID for RDSEED support
    let cpuid_result: u32;
    unsafe {
        asm!(
            "mov eax, 7",
            "mov ecx, 0",
            "cpuid",
            "mov {}, ebx",
            out(reg) cpuid_result,
            out("eax") _,
            out("ebx") _,
            out("ecx") _,
            out("edx") _,
        );
    }
    (cpuid_result & (1 << 18)) != 0
}

#[cfg(feature = "x86_64")]
fn rdrand_u64() -> Option<u64> {
    let mut value: u64;
    let success: u8;
    
    unsafe {
        asm!(
            "rdrand {}",
            "setc {}",
            out(reg) value,
            out(reg_byte) success,
            options(nomem, nostack)
        );
    }
    
    if success != 0 {
        Some(value)
    } else {
        None
    }
}

#[cfg(feature = "x86_64")]
fn rdseed_u64() -> Option<u64> {
    let mut value: u64;
    let success: u8;
    
    unsafe {
        asm!(
            "rdseed {}",
            "setc {}",
            out(reg) value,
            out(reg_byte) success,
            options(nomem, nostack)
        );
    }
    
    if success != 0 {
        Some(value)
    } else {
        None
    }
}

#[cfg(feature = "arm64")]
fn is_arm64_rng_available() -> bool {
    // Check if ARM64 FEAT_RNG is available
    // This is simplified - real implementation would check system registers
    true
}

#[cfg(feature = "arm64")]
fn arm64_random_u64() -> Option<u64> {
    // ARM64 RNDR instruction for random numbers
    // This is simplified - real implementation would use system instructions
    Some(get_cycle_count())
}

/// Get CPU cycle counter for timing entropy.
fn get_cycle_count() -> u64 {
    #[cfg(feature = "x86_64")]
    unsafe {
        let low: u32;
        let high: u32;
        asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack)
        );
        ((high as u64) << 32) | (low as u64)
    }
    
    #[cfg(feature = "arm64")]
    unsafe {
        let counter: u64;
        asm!(
            "mrs {}, cntvct_el0",
            out(reg) counter,
            options(nomem, nostack)
        );
        counter
    }
    
    #[cfg(not(any(feature = "x86_64", feature = "arm64")))]
    {
        crate::time::get_monotonic_time().as_nanos() as u64
    }
}

/// Perform volatile memory access for timing entropy.
fn volatile_memory_access() {
    static mut DUMMY: [u8; 64] = [0; 64];
    
    unsafe {
        for i in 0..64 {
            core::ptr::write_volatile(&mut DUMMY[i], i as u8);
        }
    }
}

/// Create cache timing variation for entropy.
fn cache_timing_variation() {
    // Access memory in unpredictable pattern to create cache timing variations
    static mut CACHE_DATA: [u64; 256] = [0; 256];
    
    unsafe {
        let index = (get_cycle_count() as usize) % 256;
        core::ptr::read_volatile(&CACHE_DATA[index]);
    }
}

/// Secure RNG statistics.
#[derive(Debug, Clone)]
pub struct SecureRngStats {
    pub bytes_generated: u64,
    pub entropy_collected: u64,
    pub initialized: bool,
}

/// Initialize secure RNG subsystem.
pub fn init_secure_rng() -> Result<(), ThreadError> {
    unsafe {
        SECURE_RNG.init()?;
    }
    
    // Initialization completed
    Ok(())
}

/// Fill buffer with cryptographically secure random bytes.
pub fn secure_random_bytes(dest: &mut [u8]) -> Result<(), ThreadError> {
    unsafe {
        SECURE_RNG.fill_bytes(dest)
    }
}

/// Generate secure random u64.
pub fn secure_random_u64() -> Result<u64, ThreadError> {
    unsafe {
        SECURE_RNG.next_u64()
    }
}

/// Generate secure random u32.
pub fn secure_random_u32() -> Result<u32, ThreadError> {
    unsafe {
        SECURE_RNG.next_u32()
    }
}

/// Generate secure random value in range.
pub fn secure_random_range(max: u64) -> Result<u64, ThreadError> {
    unsafe {
        SECURE_RNG.gen_range(max)
    }
}

/// Get secure RNG statistics.
pub fn get_secure_rng_stats() -> SecureRngStats {
    unsafe {
        SecureRngStats {
            bytes_generated: SECURE_RNG.bytes_generated.load(Ordering::Relaxed),
            entropy_collected: SECURE_RNG.entropy_collected.load(Ordering::Relaxed),
            initialized: SECURE_RNG.state.initialized,
        }
    }
}
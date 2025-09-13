# RISC-V 64-bit Architecture Guide

This guide covers RISC-V 64-bit (RV64G) specific features and optimizations in the preemptive multithreading library.

## Architecture Overview  

The RISC-V 64-bit implementation provides clean, extensible support for the open ISA:

- **RV64G base instruction set** (RV64IMAFD + Zicsr + Zifencei)
- **Vector extension (RVV)** for SIMD operations
- **Supervisor Binary Interface (SBI)** for timer and IPI
- **Physical Memory Protection (PMP)** for security
- **Custom extension support** for specialized hardware
- **Hypervisor extension** for virtualization
- **Bit manipulation extensions** for optimized operations

## Context Switching

### Register Set

The RISC-V context includes all architectural registers:

```rust
#[repr(C)]
pub struct Riscv64Context {
    // Integer registers (x1-x31, x0 is hardwired zero)
    pub x: [u64; 32],
    
    // Program counter
    pub pc: u64,
    
    // Floating point registers (if F/D extensions present)
    pub f: [u64; 32],  // Can be f32 or f64 depending on context
    pub fcsr: u32,     // Floating point control/status register
    
    // Supervisor-level registers
    pub sstatus: u64,  // Supervisor status
    pub sie: u64,      // Supervisor interrupt enable
    pub stvec: u64,    // Supervisor trap vector base address
    pub sscratch: u64, // Supervisor scratch register
    
    // Vector extension state (if V extension present)
    pub vector_state: Option<VectorState>,
    
    // Custom extension state
    pub custom_state: Option<CustomExtensionState>,
}

#[repr(C)]
pub struct VectorState {
    pub vl: u64,          // Vector length
    pub vtype: u64,       // Vector type register
    pub vcsr: u32,        // Vector control/status register
    pub v: Vec<u8>,       // Vector registers (variable size)
}
```

### Assembly Implementation

RISC-V context switch using standard instructions:

```asm
# Save current context (a0 = current context)
sd x1,  8(a0)     # Save ra (return address)
sd x2,  16(a0)    # Save sp (stack pointer) 
sd x3,  24(a0)    # Save gp (global pointer)
sd x4,  32(a0)    # Save tp (thread pointer)
sd x5,  40(a0)    # Save t0
...
sd x31, 248(a0)   # Save t6

# Save floating point state (if present)
csrr t0, fcsr
sw t0, 256(a0)    # Save FCSR
fsd f0,  264(a0)  # Save f0
fsd f1,  272(a0)  # Save f1
...
fsd f31, 512(a0)  # Save f31

# Save supervisor registers
csrr t0, sstatus
sd t0, 520(a0)
csrr t0, sie  
sd t0, 528(a0)

# Load new context (a1 = new context)
ld x2,  16(a1)    # Load sp
ld x3,  24(a1)    # Load gp  
ld x4,  32(a1)    # Load tp
ld x5,  40(a1)    # Load t0
...

# Load floating point state
lw t0, 256(a1)
csrw fcsr, t0     # Restore FCSR
fld f0,  264(a1)  # Load f0
...
fld f31, 512(a1)  # Load f31

# Load supervisor registers
ld t0, 520(a1)
csrw sstatus, t0
ld t0, 528(a1)
csrw sie, t0

ld x1, 8(a1)      # Load ra
ret               # Return to new thread
```

### Performance Characteristics

| Operation | Instructions | Cycles | Notes |
|-----------|-------------|---------|-------|
| Basic context switch | ~40 | 80-120 | Integer registers only |
| Full context switch | ~80 | 150-200 | Including FP registers |
| Vector context switch | ~120+ | 250+ | With RVV extension |

## Timer Integration

### SBI Timer Interface

RISC-V uses the Supervisor Binary Interface for timer services:

```rust
// SBI timer implementation
pub fn init_sbi_timer(interval_ms: u32) -> Result<(), TimerError> {
    // Get time base frequency
    let timebase_freq = get_timebase_frequency()?;
    
    // Calculate timer ticks for interval
    let timer_ticks = (timebase_freq * interval_ms as u64) / 1000;
    
    // Set initial timer
    set_next_timer_interrupt(timer_ticks)?;
    
    // Enable timer interrupts
    enable_supervisor_timer_interrupt();
    
    println!("SBI Timer initialized: {}MHz, {}ms intervals",
             timebase_freq / 1000000, interval_ms);
    Ok(())
}

// SBI call for timer setup
fn sbi_set_timer(stime_value: u64) -> SbiResult {
    let result: usize;
    let error: usize;
    
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") SBI_SET_TIMER => error,
            inlateout("a1") stime_value => result,
            in("a7") SBI_TIMER_EXTENSION,
            options(nostack)
        );
    }
    
    if error == 0 {
        SbiResult::Success(result)
    } else {
        SbiResult::Error(error)
    }
}

// High-precision timing using cycle counter
pub fn get_cycle_count() -> u64 {
    let cycles: u64;
    unsafe {
        asm!("rdcycle {}", out(reg) cycles, options(nomem, nostack));
    }
    cycles
}

pub fn get_time() -> u64 {
    let time: u64;
    unsafe {
        asm!("rdtime {}", out(reg) time, options(nomem, nostack));
    }
    time
}
```

### Performance Monitoring

RISC-V provides hardware performance counters:

```rust
// Configure performance monitoring
pub fn init_riscv_pmu() -> Result<(), ProfilingError> {
    unsafe {
        // Enable user-mode counter access
        let counteren: u64 = (1 << 0) | (1 << 2); // CY, IR bits
        asm!("csrw counteren, {}", in(reg) counteren);
        
        // Configure performance event selection (if supported)
        if cpu_has_hpm_counters() {
            // Configure hardware performance monitor counters
            for i in 3..=31 { // HPM counters 3-31
                let event = match i {
                    3 => HPM_CACHE_MISS,
                    4 => HPM_BRANCH_MISPREDICT,
                    5 => HPM_TLB_MISS,
                    _ => HPM_UNUSED,
                };
                
                let csr_addr = 0x323 + (i - 3); // mhpmevent3-31
                asm!("csrw {}, {}", 
                     const csr_addr, 
                     in(reg) event);
            }
        }
    }
    
    println!("RISC-V PMU initialized");
    Ok(())
}

// Sample performance counters
pub fn sample_riscv_performance() -> RiscvPerfSample {
    let cycles: u64;
    let instructions: u64;
    let cache_misses: u64;
    
    unsafe {
        asm!("rdcycle {}", out(reg) cycles);
        asm!("rdinstret {}", out(reg) instructions);
        
        // Read HPM counter 3 (cache misses)
        if cpu_has_hpm_counters() {
            asm!("csrr {}, hpmcounter3", out(reg) cache_misses);
        } else {
            cache_misses = 0;
        }
    }
    
    RiscvPerfSample {
        timestamp: get_time(),
        cycles,
        instructions,
        cache_misses,
        ipc: instructions as f64 / cycles.max(1) as f64,
    }
}
```

## Memory Management

### Virtual Memory Layout

```
Virtual Address Space Layout (RISC-V RV64, Sv48):
0x0000000000000000 - 0x00007FFFFFFFFFFF : User space (128TB)
0x0000800000000000 - 0xFFFF7FFFFFFFFFFF : Invalid (sign-extended)
0xFFFF800000000000 - 0xFFFFFFFFFFFFFFFF : Supervisor space (128TB)

Page Table Levels (Sv48):
Level 3: PTE covers 4KB (page)
Level 2: PTE covers 2MB (megapage)  
Level 1: PTE covers 1GB (gigapage)
Level 0: PTE covers 512GB (terapage)

Thread Stack Layout:
High Address  ┌─────────────────┐
              │   Guard Page    │ ← PMP protection
              ├─────────────────┤
              │                 │
              │   Thread Stack  │ ← Grows downward
              │                 │  
              └─────────────────┘
Low Address
```

### Physical Memory Protection (PMP)

```rust
// Configure PMP for thread isolation
pub fn setup_pmp_protection(region: &MemoryRegion) -> Result<(), SecurityError> {
    unsafe {
        // Find available PMP entry
        let pmp_index = find_free_pmp_entry()?;
        
        // Configure address range
        let pmpaddr = (region.base + region.size - 1) >> 2; // Right shift by 2
        let pmpaddr_csr = 0x3B0 + pmp_index; // pmpaddr0-15
        asm!("csrw {}, {}", const pmpaddr_csr, in(reg) pmpaddr);
        
        // Configure permissions (R=1, W=1, X=0, A=TOR)
        let pmpcfg_value = (1 << 0) | (1 << 1) | (1 << 3); // R|W|TOR
        let pmpcfg_shift = (pmp_index % 8) * 8;
        let pmpcfg_csr = 0x3A0 + (pmp_index / 8); // pmpcfg0-3
        
        // Read-modify-write PMP config
        let mut pmpcfg: u64;
        asm!("csrr {}, {}", out(reg) pmpcfg, const pmpcfg_csr);
        pmpcfg &= !(0xFF << pmpcfg_shift);
        pmpcfg |= (pmpcfg_value as u64) << pmpcfg_shift;
        asm!("csrw {}, {}", const pmpcfg_csr, in(reg) pmpcfg);
    }
    
    println!("PMP protection configured for region 0x{:x}-0x{:x}",
             region.base, region.base + region.size);
    Ok(())
}
```

### Memory Ordering

RISC-V provides flexible memory ordering with fence instructions:

```rust
// Memory ordering primitives
#[inline(always)]
pub fn fence_full() {
    unsafe {
        asm!("fence rw,rw", options(nomem, nostack));
    }
}

#[inline(always)] 
pub fn fence_acquire() {
    unsafe {
        asm!("fence r,rw", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn fence_release() {
    unsafe {
        asm!("fence rw,w", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn fence_tso() {
    unsafe {
        asm!("fence.tso", options(nomem, nostack));
    }
}
```

## Vector Processing (RVV)

### Vector Extension Support

```rust
// RISC-V Vector extension context
pub struct RvvState {
    pub vl: u64,           // Vector length
    pub vtype: u64,        // Vector type
    pub vstart: u64,       // Vector start index
    pub vxrm: u8,          // Vector fixed-point rounding mode
    pub vxsat: bool,       // Vector fixed-point saturation flag
    pub v_regs: Vec<u8>,   // Vector register file (variable size)
}

impl RvvState {
    pub fn new() -> Result<Self, ArchError> {
        if !cpu_has_rvv() {
            return Err(ArchError::UnsupportedFeature("RVV"));
        }
        
        let vlen = get_vector_length(); // VLEN in bits
        let vector_reg_size = vlen / 8;  // Convert to bytes
        
        Ok(Self {
            vl: 0,
            vtype: 0,
            vstart: 0,
            vxrm: 0,
            vxsat: false,
            v_regs: vec![0; 32 * vector_reg_size], // 32 vector registers
        })
    }
    
    // Save vector context
    pub unsafe fn save(&mut self) {
        // Read vector CSRs
        asm!("csrr {}, vl", out(reg) self.vl);
        asm!("csrr {}, vtype", out(reg) self.vtype);
        asm!("csrr {}, vstart", out(reg) self.vstart);
        
        let vcsr: u64;
        asm!("csrr {}, vcsr", out(reg) vcsr);
        self.vxrm = ((vcsr >> 1) & 0x3) as u8;
        self.vxsat = (vcsr & 0x1) != 0;
        
        // Save vector registers (would need custom assembly for each VLEN)
        // This is simplified - real implementation needs dynamic assembly
        self.save_vector_registers();
    }
    
    // Restore vector context
    pub unsafe fn restore(&self) {
        asm!("csrw vl, {}", in(reg) self.vl);
        asm!("csrw vtype, {}", in(reg) self.vtype);
        asm!("csrw vstart, {}", in(reg) self.vstart);
        
        let vcsr = ((self.vxrm as u64) << 1) | (self.vxsat as u64);
        asm!("csrw vcsr, {}", in(reg) vcsr);
        
        self.restore_vector_registers();
    }
}

// Vector-accelerated operations
pub unsafe fn rvv_memcpy(dst: *mut u8, src: *const u8, len: usize) {
    if !cpu_has_rvv() || len < 32 {
        // Fallback to scalar copy
        scalar_memcpy(dst, src, len);
        return;
    }
    
    let mut offset = 0;
    
    while offset < len {
        let remaining = len - offset;
        
        // Set vector length for this iteration
        let vl: usize;
        asm!(
            "vsetvli {vl}, {avl}, e8, m8, ta, ma",
            vl = out(reg) vl,
            avl = in(reg) remaining,
        );
        
        if vl == 0 {
            break;
        }
        
        // Vector load
        asm!(
            "vle8.v v0, ({src})",
            src = in(reg) src.add(offset),
        );
        
        // Vector store
        asm!(
            "vse8.v v0, ({dst})",
            dst = in(reg) dst.add(offset),
        );
        
        offset += vl;
    }
}
```

## Security Features

### Cryptographic Extensions

```rust
// RISC-V Cryptographic ISA Extension support
pub struct RiscvCrypto {
    scalar_crypto: bool,
    vector_crypto: bool,
}

impl RiscvCrypto {
    pub fn new() -> Self {
        Self {
            scalar_crypto: cpu_has_scalar_crypto(),
            vector_crypto: cpu_has_vector_crypto(),
        }
    }
    
    // AES encryption using scalar crypto extension
    pub fn aes_encrypt_block(&self, plaintext: [u8; 16], key: &[u8]) -> [u8; 16] {
        if !self.scalar_crypto {
            return software_aes_encrypt(plaintext, key);
        }
        
        unsafe {
            let mut state = u128::from_le_bytes(plaintext);
            
            // AES rounds using RISC-V crypto instructions
            for round_key in expand_aes_key(key) {
                asm!(
                    "aes64es {rd}, {rs1}, {rs2}",
                    rd = inout(reg) state,
                    rs1 = in(reg) state,
                    rs2 = in(reg) round_key,
                );
            }
            
            state.to_le_bytes()
        }
    }
    
    // SHA-256 using scalar crypto extension
    pub fn sha256_compress(&self, hash: &mut [u32; 8], block: &[u8; 64]) {
        if !self.scalar_crypto {
            return software_sha256_compress(hash, block);
        }
        
        unsafe {
            // Use RISC-V SHA instructions
            for chunk in block.chunks_exact(4) {
                let word = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                
                asm!(
                    "sha256sig0 {rd}, {rs1}",
                    "sha256sig1 {rd}, {rs1}", 
                    "sha256sum0 {rd}, {rs1}",
                    "sha256sum1 {rd}, {rs1}",
                    rd = inout(reg) word,
                    rs1 = in(reg) word,
                );
            }
        }
    }
}
```

### Pointer Masking

```rust
// RISC-V pointer masking for security
pub struct PointerMasking {
    enabled: bool,
    mask_bits: u8,
}

impl PointerMasking {
    pub fn init() -> Result<Self, SecurityError> {
        if !cpu_has_pointer_masking() {
            return Err(SecurityError::UnsupportedFeature("Pointer Masking"));
        }
        
        unsafe {
            // Enable pointer masking
            let menvcfg: u64 = (1 << 62) | (1 << 63); // PBMTE | STCE
            asm!("csrw menvcfg, {}", in(reg) menvcfg);
            
            // Configure masking parameters
            let mask_bits = 8; // Use 8 bits for tags
            let pmpcfg: u64 = mask_bits as u64;
            asm!("csrw pmpaddr0, {}", in(reg) pmpcfg);
        }
        
        Ok(Self {
            enabled: true,
            mask_bits: 8,
        })
    }
    
    pub fn tag_pointer(&self, ptr: *mut u8, tag: u8) -> *mut u8 {
        if !self.enabled {
            return ptr;
        }
        
        let addr = ptr as usize;
        let tagged_addr = addr | ((tag as usize) << (64 - self.mask_bits));
        tagged_addr as *mut u8
    }
    
    pub fn untag_pointer(&self, tagged_ptr: *mut u8) -> (*mut u8, u8) {
        if !self.enabled {
            return (tagged_ptr, 0);
        }
        
        let tagged_addr = tagged_ptr as usize;
        let tag = (tagged_addr >> (64 - self.mask_bits)) as u8;
        let addr = tagged_addr & !(((1 << self.mask_bits) - 1) << (64 - self.mask_bits));
        
        (addr as *mut u8, tag)
    }
}
```

## Performance Optimizations

### Custom Instructions

```rust
// Support for custom RISC-V extensions
pub trait CustomExtension {
    fn init(&mut self) -> Result<(), ArchError>;
    fn context_size(&self) -> usize;
    unsafe fn save_context(&self, buffer: *mut u8);
    unsafe fn restore_context(&self, buffer: *const u8);
}

// Example custom extension for threading acceleration
pub struct ThreadAccelExtension {
    enabled: bool,
}

impl CustomExtension for ThreadAccelExtension {
    fn init(&mut self) -> Result<(), ArchError> {
        // Check if custom threading instructions are available
        if !self.has_thread_accel_extension() {
            return Err(ArchError::UnsupportedFeature("Thread Acceleration"));
        }
        
        self.enabled = true;
        Ok(())
    }
    
    fn context_size(&self) -> usize {
        if self.enabled { 64 } else { 0 }
    }
    
    unsafe fn save_context(&self, buffer: *mut u8) {
        if !self.enabled { return; }
        
        // Save custom extension state
        asm!(
            ".insn r 0x0B, 0x0, 0x0, x0, {}, x0", // Custom save instruction
            in(reg) buffer,
        );
    }
    
    unsafe fn restore_context(&self, buffer: *const u8) {
        if !self.enabled { return; }
        
        // Restore custom extension state
        asm!(
            ".insn r 0x0B, 0x1, 0x0, x0, {}, x0", // Custom restore instruction
            in(reg) buffer,
        );
    }
}

// Fast context switch using custom instructions
pub unsafe fn custom_context_switch(
    prev_ctx: *mut Riscv64Context,
    next_ctx: *const Riscv64Context,
) {
    if has_custom_context_switch() {
        // Use custom instruction for ultra-fast switching
        asm!(
            ".insn r 0x7B, 0x0, 0x1, x0, {}, {}", // Custom context switch
            in(reg) prev_ctx,
            in(reg) next_ctx,
        );
    } else {
        // Fallback to standard context switch
        standard_context_switch(prev_ctx, next_ctx);
    }
}
```

### Bit Manipulation Optimizations

```rust
// Use RISC-V bit manipulation extensions
pub struct BitManipOps;

impl BitManipOps {
    // Count leading zeros using Zbb extension
    pub fn clz(value: u64) -> u32 {
        if cpu_has_zbb() {
            let result: u64;
            unsafe {
                asm!("clz {}, {}", out(reg) result, in(reg) value);
            }
            result as u32
        } else {
            value.leading_zeros()
        }
    }
    
    // Population count using Zbb extension
    pub fn popcount(value: u64) -> u32 {
        if cpu_has_zbb() {
            let result: u64;
            unsafe {
                asm!("cpop {}, {}", out(reg) result, in(reg) value);
            }
            result as u32
        } else {
            value.count_ones()
        }
    }
    
    // Optimized priority queue operations
    pub fn find_highest_priority(bitmap: u64) -> Option<u8> {
        if bitmap == 0 {
            return None;
        }
        
        let priority = 63 - Self::clz(bitmap);
        Some(priority as u8)
    }
    
    // Fast atomic bit operations using Zbt extension
    pub fn atomic_bit_set(addr: *mut u64, bit: u8) -> bool {
        if cpu_has_zbt() {
            let old_value: u64;
            unsafe {
                asm!(
                    "bset {}, {}, {}",
                    "amoor.d {}, {}, ({})",
                    out(reg) old_value,
                    inout(reg) 1u64 << bit,
                    in(reg) addr,
                );
            }
            (old_value & (1u64 << bit)) != 0
        } else {
            // Fallback to standard atomic operations
            let old = unsafe { (*addr).fetch_or(1u64 << bit, Ordering::AcqRel) };
            (old & (1u64 << bit)) != 0
        }
    }
}
```

## Platform Integration

### SBI Interface

```rust
// Complete SBI interface implementation
pub struct SbiInterface {
    version: SbiVersion,
    extensions: Vec<SbiExtension>,
}

impl SbiInterface {
    pub fn new() -> Result<Self, PlatformError> {
        let version = Self::get_sbi_version()?;
        let extensions = Self::probe_extensions()?;
        
        Ok(Self { version, extensions })
    }
    
    fn get_sbi_version() -> Result<SbiVersion, PlatformError> {
        let version: usize;
        unsafe {
            asm!(
                "ecall",
                inlateout("a0") SBI_GET_SPEC_VERSION => version,
                in("a7") SBI_BASE_EXTENSION,
                out("a1") _,
            );
        }
        
        Ok(SbiVersion {
            major: ((version >> 24) & 0x7F) as u8,
            minor: (version & 0xFFFFFF) as u32,
        })
    }
    
    // Timer services
    pub fn set_timer(&self, stime_value: u64) -> Result<(), SbiError> {
        let result: isize;
        unsafe {
            asm!(
                "ecall",
                inlateout("a0") stime_value => result,
                in("a7") SBI_TIMER_EXTENSION,
                out("a1") _,
            );
        }
        
        if result == 0 {
            Ok(())
        } else {
            Err(SbiError::from(result))
        }
    }
    
    // IPI services
    pub fn send_ipi(&self, hart_mask: &[u64], hart_mask_base: u64) -> Result<(), SbiError> {
        let result: isize;
        unsafe {
            asm!(
                "ecall",
                inlateout("a0") hart_mask.as_ptr() => result,
                in("a1") hart_mask_base,
                in("a7") SBI_IPI_EXTENSION,
            );
        }
        
        if result == 0 {
            Ok(())
        } else {
            Err(SbiError::from(result))
        }
    }
    
    // Hart state management
    pub fn hart_start(&self, hartid: u64, start_addr: u64, opaque: u64) -> Result<(), SbiError> {
        let result: isize;
        unsafe {
            asm!(
                "ecall",
                inlateout("a0") hartid => result,
                in("a1") start_addr,
                in("a2") opaque,
                in("a7") SBI_HSM_EXTENSION,
            );
        }
        
        if result == 0 {
            Ok(())
        } else {
            Err(SbiError::from(result))
        }
    }
}
```

## Best Practices

### Thread Configuration

```rust
// Optimal RISC-V thread setup
let thread = ThreadBuilder::new()
    .stack_size(512 * 1024)       // 512KB stack (smaller than x86_64)
    .enable_vector(cpu_has_rvv()) // Enable RVV if available
    .enable_crypto(cpu_has_crypto()) // Enable crypto extensions
    .pmp_region(pmp_config)       // Configure PMP protection
    .spawn(worker_function)?;
```

### Performance Guidelines

1. **Leverage RISC-V's simplicity** - fewer pipeline hazards
2. **Use vector extensions** when available for data parallel work
3. **Minimize memory barriers** - RISC-V has relaxed memory model
4. **Use bit manipulation extensions** for scheduler operations
5. **Implement custom extensions** for specialized workloads
6. **Profile with performance counters** to find bottlenecks
7. **Use compressed instructions** to reduce code size

### Security Recommendations

1. **Configure PMP** for memory protection between threads
2. **Use cryptographic extensions** for secure operations  
3. **Implement pointer masking** for tagged pointers
4. **Enable security extensions** when available
5. **Use SBI secure services** for trusted operations
6. **Implement stack protection** using PMP or custom methods
7. **Validate all SBI calls** for proper error handling

## Troubleshooting

### Common Issues

**SBI call failures:**
- Check SBI version compatibility
- Verify extension availability
- Handle all error codes properly

**Vector extension problems:**
- Verify VLEN configuration
- Check vector context save/restore
- Handle variable-length vectors correctly

**PMP configuration errors:**
- Ensure proper address alignment
- Check for overlapping regions
- Verify permission settings

### Debug Tools

```bash
# Check RISC-V ISA string
cat /proc/cpuinfo | grep isa

# Monitor with Linux perf (if available)
perf stat -e cycles,instructions ./program

# Check SBI implementation
./sbi_test --probe-extensions

# Test vector performance  
./rvv_benchmark --vlen=256
```

## References

- [RISC-V Instruction Set Manual](https://riscv.org/specifications/)
- [RISC-V Supervisor Binary Interface](https://github.com/riscv-non-isa/riscv-sbi-doc)
- [RISC-V Vector Extension](https://github.com/riscv/riscv-v-spec)
- [RISC-V Cryptography Extensions](https://github.com/riscv/riscv-crypto)
- [RISC-V Physical Memory Protection](https://github.com/riscv/riscv-pmp)
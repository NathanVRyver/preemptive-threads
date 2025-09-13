//! NUMA-aware thread placement and memory allocation optimizations.

use crate::perf::{PerfConfig, PERF_COUNTERS};
use crate::thread_new::Thread;
use crate::sched::CpuId;
use crate::arch::barriers::CacheLinePadded;
use portable_atomic::{AtomicUsize, AtomicU64, Ordering};
use alloc::{vec, vec::Vec};

/// NUMA node identifier.
pub type NumaNodeId = u16;

/// NUMA topology information.
#[derive(Debug, Clone)]
pub struct NumaTopology {
    /// Number of NUMA nodes in the system
    pub node_count: usize,
    /// CPU to NUMA node mapping
    pub cpu_to_node: Vec<NumaNodeId>,
    /// NUMA node to CPU list mapping
    pub node_to_cpus: Vec<Vec<CpuId>>,
    /// Inter-node distance matrix (relative costs)
    pub distance_matrix: Vec<Vec<u32>>,
    /// Memory capacity per NUMA node (in bytes)
    pub node_memory_capacity: Vec<u64>,
}

impl NumaTopology {
    /// Create a default single-node topology.
    pub fn single_node(cpu_count: usize) -> Self {
        Self {
            node_count: 1,
            cpu_to_node: vec![0; cpu_count],
            node_to_cpus: vec![(0..cpu_count as CpuId).collect()],
            distance_matrix: vec![vec![10]], // Standard local distance
            node_memory_capacity: vec![u64::MAX], // Assume unlimited for single node
        }
    }
    
    /// Detect system NUMA topology (simplified detection).
    pub fn detect() -> Self {
        // In a real implementation, this would query the OS for NUMA information
        // For now, assume single node
        Self::single_node(4) // Default to 4 CPUs
    }
    
    /// Get NUMA node for a given CPU.
    pub fn get_cpu_node(&self, cpu_id: CpuId) -> NumaNodeId {
        self.cpu_to_node.get(cpu_id as usize).copied().unwrap_or(0)
    }
    
    /// Get CPUs in a given NUMA node.
    pub fn get_node_cpus(&self, node_id: NumaNodeId) -> &[CpuId] {
        self.node_to_cpus.get(node_id as usize).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    /// Get distance between two NUMA nodes.
    pub fn get_distance(&self, node1: NumaNodeId, node2: NumaNodeId) -> u32 {
        self.distance_matrix
            .get(node1 as usize)
            .and_then(|row| row.get(node2 as usize))
            .copied()
            .unwrap_or(u32::MAX)
    }
    
    /// Find closest NUMA node to a given node.
    pub fn find_closest_node(&self, from_node: NumaNodeId, exclude: &[NumaNodeId]) -> Option<NumaNodeId> {
        let mut best_node = None;
        let mut best_distance = u32::MAX;
        
        for node_id in 0..self.node_count {
            let node_id = node_id as NumaNodeId;
            if node_id != from_node && !exclude.contains(&node_id) {
                let distance = self.get_distance(from_node, node_id);
                if distance < best_distance {
                    best_distance = distance;
                    best_node = Some(node_id);
                }
            }
        }
        
        best_node
    }
}

/// NUMA-aware thread placement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumaPlacementPolicy {
    /// Prefer local NUMA node
    Local,
    /// Spread threads across NUMA nodes
    Spread,
    /// Interleave threads across NUMA nodes
    Interleave,
    /// Follow memory allocation
    FollowMemory,
    /// Custom policy with specific node preference
    Preferred(NumaNodeId),
}

/// Per-NUMA node statistics and state.
#[repr(align(64))] // Cache line aligned
pub struct NumaNodeState {
    /// NUMA node ID
    pub node_id: NumaNodeId,
    
    /// Number of threads currently running on this node
    pub active_threads: AtomicUsize,
    
    /// Number of threads with affinity to this node
    pub affine_threads: AtomicUsize,
    
    /// Memory allocation statistics
    pub local_allocations: AtomicU64,
    pub remote_allocations: AtomicU64,
    
    /// Load balancing statistics
    pub migrations_in: AtomicU64,
    pub migrations_out: AtomicU64,
    
    /// Performance counters
    pub perf_stats: CacheLinePadded<NumaNodePerfStats>,
}

#[derive(Default)]
pub struct NumaNodePerfStats {
    pub scheduler_runs: AtomicU64,
    pub work_steals: AtomicU64,
    pub load_imbalances: AtomicU64,
    pub memory_pressure_events: AtomicU64,
}

impl NumaNodeState {
    pub fn new(node_id: NumaNodeId) -> Self {
        Self {
            node_id,
            active_threads: AtomicUsize::new(0),
            affine_threads: AtomicUsize::new(0),
            local_allocations: AtomicU64::new(0),
            remote_allocations: AtomicU64::new(0),
            migrations_in: AtomicU64::new(0),
            migrations_out: AtomicU64::new(0),
            perf_stats: CacheLinePadded::new(NumaNodePerfStats::default()),
        }
    }
    
    /// Get current load on this NUMA node.
    pub fn get_load(&self) -> f64 {
        let active = self.active_threads.load(Ordering::Relaxed) as f64;
        let affine = self.affine_threads.load(Ordering::Relaxed) as f64;
        
        // Load is a combination of active threads and thread affinity
        active + (affine * 0.5)
    }
    
    /// Get memory locality ratio.
    pub fn get_memory_locality(&self) -> f64 {
        let local = self.local_allocations.load(Ordering::Relaxed) as f64;
        let remote = self.remote_allocations.load(Ordering::Relaxed) as f64;
        
        if local + remote > 0.0 {
            local / (local + remote)
        } else {
            1.0 // Assume perfect locality if no data
        }
    }
    
    /// Record a thread migration into this node.
    pub fn record_migration_in(&self) {
        self.migrations_in.fetch_add(1, Ordering::Relaxed);
        self.active_threads.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a thread migration out of this node.
    pub fn record_migration_out(&self) {
        self.migrations_out.fetch_add(1, Ordering::Relaxed);
        self.active_threads.fetch_sub(1, Ordering::Relaxed);
    }
    
    /// Record local memory allocation.
    pub fn record_local_allocation(&self, size: usize) {
        self.local_allocations.fetch_add(size as u64, Ordering::Relaxed);
        PERF_COUNTERS.record_numa_local();
    }
    
    /// Record remote memory allocation.
    pub fn record_remote_allocation(&self, size: usize) {
        self.remote_allocations.fetch_add(size as u64, Ordering::Relaxed);
        PERF_COUNTERS.record_numa_remote();
    }
}

/// NUMA-aware scheduler with optimized thread placement.
pub struct NumaAwareScheduler {
    /// NUMA topology information
    topology: NumaTopology,
    
    /// Per-node state (cache line padded)
    node_states: Vec<CacheLinePadded<NumaNodeState>>,
    
    /// Global placement policy
    placement_policy: NumaPlacementPolicy,
    
    /// Load balancing configuration
    load_balance_threshold: f64,
    migration_cost_threshold: u32,
    
    /// Performance configuration
    config: PerfConfig,
}

impl NumaAwareScheduler {
    pub fn new(config: PerfConfig, topology: NumaTopology) -> Self {
        let mut node_states = Vec::with_capacity(topology.node_count);
        for node_id in 0..topology.node_count {
            node_states.push(CacheLinePadded::new(
                NumaNodeState::new(node_id as NumaNodeId)
            ));
        }
        
        Self {
            topology,
            node_states,
            placement_policy: NumaPlacementPolicy::Local,
            load_balance_threshold: 1.5, // 50% imbalance triggers balancing
            migration_cost_threshold: 20, // Maximum distance for migration
            config,
        }
    }
    
    /// Find optimal NUMA node for thread placement.
    pub fn find_optimal_placement(&self, thread: &Thread, current_cpu: CpuId) -> NumaNodeId {
        let current_node = self.topology.get_cpu_node(current_cpu);
        
        match self.placement_policy {
            NumaPlacementPolicy::Local => current_node,
            
            NumaPlacementPolicy::Spread => {
                self.find_least_loaded_node().unwrap_or(current_node)
            }
            
            NumaPlacementPolicy::Interleave => {
                (thread.id().as_u64() % self.topology.node_count as u64) as NumaNodeId
            }
            
            NumaPlacementPolicy::FollowMemory => {
                // In a real implementation, this would analyze thread memory usage
                current_node
            }
            
            NumaPlacementPolicy::Preferred(node_id) => {
                if node_id < self.topology.node_count as NumaNodeId {
                    node_id
                } else {
                    current_node
                }
            }
        }
    }
    
    /// Find the NUMA node with the lowest load.
    fn find_least_loaded_node(&self) -> Option<NumaNodeId> {
        let mut best_node = None;
        let mut best_load = f64::INFINITY;
        
        for (node_id, node_state) in self.node_states.iter().enumerate() {
            let load = node_state.get().get_load();
            if load < best_load {
                best_load = load;
                best_node = Some(node_id as NumaNodeId);
            }
        }
        
        best_node
    }
    
    /// Check if load balancing is needed between NUMA nodes.
    pub fn should_balance_load(&self) -> Option<(NumaNodeId, NumaNodeId)> {
        if self.topology.node_count < 2 {
            return None;
        }
        
        let mut max_load = 0.0;
        let mut min_load = f64::INFINITY;
        let mut max_node = 0;
        let mut min_node = 0;
        
        for (node_id, node_state) in self.node_states.iter().enumerate() {
            let load = node_state.get().get_load();
            
            if load > max_load {
                max_load = load;
                max_node = node_id as NumaNodeId;
            }
            
            if load < min_load {
                min_load = load;
                min_node = node_id as NumaNodeId;
            }
        }
        
        // Check if imbalance exceeds threshold
        if min_load > 0.0 && (max_load / min_load) > self.load_balance_threshold {
            // Check if migration cost is acceptable
            let distance = self.topology.get_distance(max_node, min_node);
            if distance <= self.migration_cost_threshold {
                return Some((max_node, min_node));
            }
        }
        
        None
    }
    
    /// Suggest thread migration for load balancing.
    pub fn suggest_migration(&self, from_node: NumaNodeId, to_node: NumaNodeId) -> Option<ThreadMigrationSuggestion> {
        // Validate migration makes sense
        let distance = self.topology.get_distance(from_node, to_node);
        if distance > self.migration_cost_threshold {
            return None;
        }
        
        let from_state = self.node_states.get(from_node as usize)?;
        let to_state = self.node_states.get(to_node as usize)?;
        
        let from_load = from_state.get().get_load();
        let to_load = to_state.get().get_load();
        
        if from_load <= to_load {
            return None; // Migration wouldn't improve balance
        }
        
        Some(ThreadMigrationSuggestion {
            from_node,
            to_node,
            migration_cost: distance,
            expected_benefit: from_load - to_load,
            target_cpu: self.find_best_cpu_in_node(to_node),
        })
    }
    
    /// Find the best CPU in a NUMA node for thread placement.
    fn find_best_cpu_in_node(&self, node_id: NumaNodeId) -> Option<CpuId> {
        let cpus = self.topology.get_node_cpus(node_id);
        if cpus.is_empty() {
            return None;
        }
        
        // For now, just return the first CPU
        // In a real implementation, this would consider CPU load
        Some(cpus[0])
    }
    
    /// Set the NUMA placement policy.
    pub fn set_placement_policy(&mut self, policy: NumaPlacementPolicy) {
        self.placement_policy = policy;
    }
    
    /// Get NUMA statistics for all nodes.
    pub fn get_numa_stats(&self) -> Vec<NumaNodeStats> {
        self.node_states.iter().enumerate().map(|(node_id, node_state)| {
            let state = node_state.get();
            NumaNodeStats {
                node_id: node_id as NumaNodeId,
                active_threads: state.active_threads.load(Ordering::Relaxed),
                affine_threads: state.affine_threads.load(Ordering::Relaxed),
                load: state.get_load(),
                memory_locality: state.get_memory_locality(),
                migrations_in: state.migrations_in.load(Ordering::Relaxed),
                migrations_out: state.migrations_out.load(Ordering::Relaxed),
                local_allocations: state.local_allocations.load(Ordering::Relaxed),
                remote_allocations: state.remote_allocations.load(Ordering::Relaxed),
            }
        }).collect()
    }
    
    /// Get system-wide NUMA efficiency metrics.
    pub fn get_numa_efficiency(&self) -> NumaEfficiencyMetrics {
        let stats = self.get_numa_stats();
        
        let total_local: u64 = stats.iter().map(|s| s.local_allocations).sum();
        let total_remote: u64 = stats.iter().map(|s| s.remote_allocations).sum();
        
        let locality_ratio = if total_local + total_remote > 0 {
            total_local as f64 / (total_local + total_remote) as f64
        } else {
            1.0
        };
        
        // Calculate load imbalance
        let loads: Vec<f64> = stats.iter().map(|s| s.load).collect();
        let max_load = loads.iter().fold(0.0_f64, |a, &b| a.max(b));
        let min_load = loads.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let load_imbalance = if min_load > 0.0 { max_load / min_load } else { 1.0 };
        
        NumaEfficiencyMetrics {
            locality_ratio,
            load_imbalance,
            total_migrations: stats.iter().map(|s| s.migrations_in + s.migrations_out).sum(),
            active_nodes: stats.iter().filter(|s| s.active_threads > 0).count(),
            memory_efficiency: locality_ratio,
        }
    }
}

/// Thread migration suggestion.
#[derive(Debug, Clone)]
pub struct ThreadMigrationSuggestion {
    pub from_node: NumaNodeId,
    pub to_node: NumaNodeId,
    pub migration_cost: u32,
    pub expected_benefit: f64,
    pub target_cpu: Option<CpuId>,
}

/// Per-NUMA node statistics.
#[derive(Debug, Clone)]
pub struct NumaNodeStats {
    pub node_id: NumaNodeId,
    pub active_threads: usize,
    pub affine_threads: usize,
    pub load: f64,
    pub memory_locality: f64,
    pub migrations_in: u64,
    pub migrations_out: u64,
    pub local_allocations: u64,
    pub remote_allocations: u64,
}

/// NUMA system efficiency metrics.
#[derive(Debug, Clone)]
pub struct NumaEfficiencyMetrics {
    pub locality_ratio: f64,
    pub load_imbalance: f64,
    pub total_migrations: u64,
    pub active_nodes: usize,
    pub memory_efficiency: f64,
}

/// Initialize NUMA optimization subsystem.
pub fn init_numa_optimization(config: &PerfConfig) {
    let topology = if config.numa_nodes > 1 {
        NumaTopology::detect()
    } else {
        NumaTopology::single_node(config.cpu_count)
    };
    
    // In a real implementation, this would set up global NUMA scheduler
    // For now, just record that NUMA optimization is available
    // NUMA optimization initialized for topology with node count and CPU count
}

// Helper to get number of CPUs (would be replaced with actual detection)
mod num_cpus {
    fn get_physical() -> usize {
        // Simplified CPU detection - in reality would query OS
        4
    }
}
use core::sync::atomic::{AtomicU32, AtomicUsize, AtomicBool, Ordering};
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use crate::error::{ThreadError, ThreadResult};
use crate::thread::{Thread, ThreadId, ThreadState};

const MAX_THREADS: usize = 32;
const PRIORITY_LEVELS: usize = 8;

/// Lock-free priority queue for thread scheduling
pub struct PriorityQueue {
    /// Per-priority circular buffers
    queues: [CircularBuffer; PRIORITY_LEVELS],
    /// Bitmap of non-empty priority levels
    priority_bitmap: AtomicU32,
}

struct CircularBuffer {
    buffer: [AtomicUsize; MAX_THREADS],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl CircularBuffer {
    const fn new() -> Self {
        const ATOMIC_NONE: AtomicUsize = AtomicUsize::new(usize::MAX);
        Self {
            buffer: [ATOMIC_NONE; MAX_THREADS],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn enqueue(&self, thread_id: ThreadId) -> bool {
        let mut tail = self.tail.load(Ordering::Acquire);
        
        loop {
            let next_tail = (tail + 1) % MAX_THREADS;
            let head = self.head.load(Ordering::Acquire);
            
            if next_tail == head {
                return false; // Queue full
            }
            
            match self.tail.compare_exchange_weak(
                tail,
                next_tail,
                Ordering::Release,
                Ordering::Acquire
            ) {
                Ok(_) => {
                    self.buffer[tail].store(thread_id, Ordering::Release);
                    return true;
                }
                Err(actual) => tail = actual,
            }
        }
    }

    fn dequeue(&self) -> Option<ThreadId> {
        let mut head = self.head.load(Ordering::Acquire);
        
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            
            if head == tail {
                return None; // Queue empty
            }
            
            let thread_id = self.buffer[head].load(Ordering::Acquire);
            if thread_id == usize::MAX {
                // Spurious empty slot, try next
                head = (head + 1) % MAX_THREADS;
                continue;
            }
            
            let next_head = (head + 1) % MAX_THREADS;
            
            match self.head.compare_exchange_weak(
                head,
                next_head,
                Ordering::Release,
                Ordering::Acquire
            ) {
                Ok(_) => {
                    self.buffer[head].store(usize::MAX, Ordering::Release);
                    return Some(thread_id);
                }
                Err(actual) => head = actual,
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }
}

impl PriorityQueue {
    const fn new() -> Self {
        const CIRCULAR_BUFFER: CircularBuffer = CircularBuffer::new();
        Self {
            queues: [CIRCULAR_BUFFER; PRIORITY_LEVELS],
            priority_bitmap: AtomicU32::new(0),
        }
    }

    fn enqueue(&self, thread_id: ThreadId, priority: u8) -> bool {
        let priority_level = (priority as usize).min(PRIORITY_LEVELS - 1);
        
        if self.queues[priority_level].enqueue(thread_id) {
            // Set bit in bitmap to indicate non-empty queue
            self.priority_bitmap.fetch_or(1 << priority_level, Ordering::Release);
            true
        } else {
            false
        }
    }

    fn dequeue(&self) -> Option<ThreadId> {
        let mut bitmap = self.priority_bitmap.load(Ordering::Acquire);
        
        while bitmap != 0 {
            // Find highest priority non-empty queue (MSB)
            let priority_level = 31 - bitmap.leading_zeros() as usize;
            
            if let Some(thread_id) = self.queues[priority_level].dequeue() {
                // Check if queue is now empty and clear bit
                if self.queues[priority_level].is_empty() {
                    self.priority_bitmap.fetch_and(!(1 << priority_level), Ordering::Release);
                }
                return Some(thread_id);
            }
            
            // Queue was empty, clear bit and retry
            bitmap &= !(1 << priority_level);
        }
        
        None
    }
}

/// Per-CPU scheduler state
pub struct CpuScheduler {
    /// Current running thread on this CPU
    current_thread: AtomicUsize,
    /// CPU-local run queue for better cache locality
    local_queue: UnsafeCell<CircularBuffer>,
    /// Is this CPU idle?
    idle: AtomicBool,
}

unsafe impl Sync for CpuScheduler {}

impl CpuScheduler {
    const fn new() -> Self {
        Self {
            current_thread: AtomicUsize::new(usize::MAX),
            local_queue: UnsafeCell::new(CircularBuffer::new()),
            idle: AtomicBool::new(true),
        }
    }
}

/// Thread-safe atomic scheduler
pub struct AtomicScheduler {
    /// Thread pool
    threads: [UnsafeCell<MaybeUninit<Thread>>; MAX_THREADS],
    /// Thread allocation bitmap
    thread_bitmap: AtomicU32,
    /// Next thread ID counter
    next_thread_id: AtomicUsize,
    /// Global priority queue
    global_queue: PriorityQueue,
    /// Per-CPU schedulers (we'll use just one for now)
    cpu_schedulers: [CpuScheduler; 1],
    /// Scheduler lock for critical sections
    scheduler_lock: AtomicBool,
}

unsafe impl Sync for AtomicScheduler {}

impl AtomicScheduler {
    pub const fn new() -> Self {
        const UNINIT_THREAD: UnsafeCell<MaybeUninit<Thread>> = UnsafeCell::new(MaybeUninit::uninit());
        
        Self {
            threads: [UNINIT_THREAD; MAX_THREADS],
            thread_bitmap: AtomicU32::new(0),
            next_thread_id: AtomicUsize::new(0),
            global_queue: PriorityQueue::new(),
            cpu_schedulers: [CpuScheduler::new(); 1],
            scheduler_lock: AtomicBool::new(false),
        }
    }

    /// Acquire scheduler lock with exponential backoff
    fn acquire_lock(&self) {
        let mut backoff = 1;
        while self.scheduler_lock.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed
        ).is_err() {
            for _ in 0..backoff {
                core::hint::spin_loop();
            }
            backoff = (backoff * 2).min(1024);
        }
    }

    /// Release scheduler lock
    fn release_lock(&self) {
        self.scheduler_lock.store(false, Ordering::Release);
    }

    pub fn spawn_thread(
        &self,
        stack: &'static mut [u8],
        entry_point: fn(),
        priority: u8,
    ) -> ThreadResult<ThreadId> {
        // Find free thread slot
        let mut bitmap = self.thread_bitmap.load(Ordering::Acquire);
        let mut slot = None;
        
        for i in 0..MAX_THREADS {
            if bitmap & (1 << i) == 0 {
                // Try to claim this slot
                if self.thread_bitmap.compare_exchange_weak(
                    bitmap,
                    bitmap | (1 << i),
                    Ordering::AcqRel,
                    Ordering::Acquire
                ).is_ok() {
                    slot = Some(i);
                    break;
                }
                // Reload bitmap and retry
                bitmap = self.thread_bitmap.load(Ordering::Acquire);
            }
        }
        
        let slot = slot.ok_or(ThreadError::MaxThreadsReached)?;
        let thread_id = slot;
        
        // Initialize thread
        let thread = Thread::new(thread_id, stack, entry_point, priority);
        
        unsafe {
            (*self.threads[slot].get()).write(thread);
        }
        
        // Add to run queue
        if !self.global_queue.enqueue(thread_id, priority) {
            // Failed to enqueue, free the slot
            self.thread_bitmap.fetch_and(!(1 << slot), Ordering::Release);
            return Err(ThreadError::SchedulerFull);
        }
        
        Ok(thread_id)
    }

    pub fn schedule(&self) -> Option<ThreadId> {
        let cpu = &self.cpu_schedulers[0];
        
        // Try local queue first for better cache locality
        let local_queue = unsafe { &*cpu.local_queue.get() };
        
        if let Some(thread_id) = local_queue.dequeue() {
            return Some(thread_id);
        }
        
        // Fall back to global queue
        self.global_queue.dequeue()
    }

    pub fn get_current_thread(&self) -> Option<ThreadId> {
        let cpu = &self.cpu_schedulers[0];
        let current = cpu.current_thread.load(Ordering::Acquire);
        
        if current == usize::MAX {
            None
        } else {
            Some(current)
        }
    }

    pub fn set_current_thread(&self, thread_id: Option<ThreadId>) {
        let cpu = &self.cpu_schedulers[0];
        
        match thread_id {
            Some(id) => {
                cpu.current_thread.store(id, Ordering::Release);
                cpu.idle.store(false, Ordering::Release);
            }
            None => {
                cpu.current_thread.store(usize::MAX, Ordering::Release);
                cpu.idle.store(true, Ordering::Release);
            }
        }
    }

    pub fn get_thread(&self, thread_id: ThreadId) -> Option<&Thread> {
        if thread_id >= MAX_THREADS {
            return None;
        }
        
        let bitmap = self.thread_bitmap.load(Ordering::Acquire);
        if bitmap & (1 << thread_id) == 0 {
            return None;
        }
        
        unsafe {
            Some((*self.threads[thread_id].get()).assume_init_ref())
        }
    }

    pub fn get_thread_mut(&self, thread_id: ThreadId) -> Option<&mut Thread> {
        if thread_id >= MAX_THREADS {
            return None;
        }
        
        let bitmap = self.thread_bitmap.load(Ordering::Acquire);
        if bitmap & (1 << thread_id) == 0 {
            return None;
        }
        
        unsafe {
            Some((*self.threads[thread_id].get()).assume_init_mut())
        }
    }

    pub fn exit_current_thread(&self) {
        if let Some(thread_id) = self.get_current_thread() {
            self.acquire_lock();
            
            if let Some(thread) = self.get_thread_mut(thread_id) {
                thread.state = ThreadState::Finished;
                
                // Wake up joiners
                for waiter_id in thread.join_waiters.iter().flatten() {
                    if let Some(waiter) = self.get_thread_mut(*waiter_id) {
                        if waiter.state == ThreadState::Blocked {
                            waiter.state = ThreadState::Ready;
                            let _ = self.global_queue.enqueue(*waiter_id, waiter.priority);
                        }
                    }
                }
            }
            
            self.release_lock();
            
            // Clear current thread
            self.set_current_thread(None);
        }
    }

    pub fn switch_context(&self, from_id: ThreadId, to_id: ThreadId) -> ThreadResult<()> {
        // Validate thread IDs
        if from_id >= MAX_THREADS || to_id >= MAX_THREADS {
            return Err(ThreadError::InvalidThreadId);
        }
        
        let bitmap = self.thread_bitmap.load(Ordering::Acquire);
        if bitmap & (1 << from_id) == 0 || bitmap & (1 << to_id) == 0 {
            return Err(ThreadError::InvalidThreadId);
        }
        
        // Get thread pointers
        let from_thread = self.get_thread_mut(from_id).ok_or(ThreadError::InvalidThreadId)?;
        let from_context = &mut from_thread.context as *mut _;
        
        let to_thread = self.get_thread(to_id).ok_or(ThreadError::InvalidThreadId)?;
        let to_context = &to_thread.context as *const _;
        
        unsafe {
            crate::context::switch_context(from_context, to_context);
        }
        
        Ok(())
    }
}

pub static ATOMIC_SCHEDULER: AtomicScheduler = AtomicScheduler::new();
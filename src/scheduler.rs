use crate::error::{ThreadError, ThreadResult};
use crate::thread::{Thread, ThreadId, ThreadState};
use core::cell::UnsafeCell;

const MAX_THREADS: usize = 32;

pub struct Scheduler {
    threads: [Option<Thread>; MAX_THREADS],
    current_thread: Option<ThreadId>,
    next_thread_id: ThreadId,
    run_queue: [Option<ThreadId>; MAX_THREADS],
    run_queue_head: usize,
    run_queue_tail: usize,
    run_queue_count: usize,
}

pub struct SchedulerCell(UnsafeCell<Scheduler>);

unsafe impl Sync for SchedulerCell {}

impl Default for SchedulerCell {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulerCell {
    pub const fn new() -> Self {
        SchedulerCell(UnsafeCell::new(Scheduler::new()))
    }

    /// # Safety
    /// Returns mutable reference to scheduler. Caller must ensure thread safety.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get(&self) -> &mut Scheduler {
        &mut *self.0.get()
    }
}

pub static SCHEDULER: SchedulerCell = SchedulerCell::new();

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            threads: [const { None }; MAX_THREADS],
            current_thread: None,
            next_thread_id: 0,
            run_queue: [None; MAX_THREADS],
            run_queue_head: 0,
            run_queue_tail: 0,
            run_queue_count: 0,
        }
    }

    pub fn spawn_thread(
        &mut self,
        stack: &'static mut [u8],
        entry_point: fn(),
        priority: u8,
    ) -> ThreadResult<ThreadId> {
        let thread_id = self.next_thread_id;

        if thread_id >= MAX_THREADS {
            return Err(ThreadError::MaxThreadsReached);
        }

        let thread = Thread::new(thread_id, stack, entry_point, priority);
        self.threads[thread_id] = Some(thread);
        self.next_thread_id += 1;

        self.enqueue_thread(thread_id)?;
        Ok(thread_id)
    }

    fn enqueue_thread(&mut self, thread_id: ThreadId) -> ThreadResult<()> {
        if self.run_queue_count >= MAX_THREADS {
            return Err(ThreadError::SchedulerFull);
        }

        self.run_queue[self.run_queue_tail] = Some(thread_id);
        self.run_queue_tail = (self.run_queue_tail + 1) % MAX_THREADS;
        self.run_queue_count += 1;
        Ok(())
    }

    pub fn schedule(&mut self) -> Option<ThreadId> {
        if let Some(current) = self.current_thread {
            if let Some(thread) = &mut self.threads[current] {
                if thread.state == ThreadState::Running {
                    // Set back to Ready when yielding
                    thread.state = ThreadState::Ready;
                    let _ = self.enqueue_thread(current);
                }
            }
        }

        self.schedule_with_priority()
    }

    fn schedule_with_priority(&mut self) -> Option<ThreadId> {
        if self.run_queue_count == 0 {
            return None;
        }

        let mut best_thread = None;
        let mut highest_priority = 0u8;
        let mut best_index = None;

        // First pass: find the highest priority
        for i in 0..self.run_queue_count {
            let queue_index = (self.run_queue_head + i) % MAX_THREADS;

            if let Some(thread_id) = self.run_queue[queue_index] {
                if let Some(thread) = &self.threads[thread_id] {
                    if thread.is_runnable() && thread.priority > highest_priority {
                        highest_priority = thread.priority;
                    }
                }
            }
        }

        // Second pass: find the first thread with highest priority (round-robin for equal priorities)
        for i in 0..self.run_queue_count {
            let queue_index = (self.run_queue_head + i) % MAX_THREADS;

            if let Some(thread_id) = self.run_queue[queue_index] {
                if let Some(thread) = &self.threads[thread_id] {
                    if thread.is_runnable() && thread.priority == highest_priority {
                        best_thread = Some(thread_id);
                        best_index = Some(queue_index);
                        break; // Take the first one we find for round-robin
                    }
                }
            }
        }

        if let (Some(thread_id), Some(index)) = (best_thread, best_index) {
            // Remove from queue and compact
            self.run_queue[index] = None;

            let mut read_pos = (index + 1) % MAX_THREADS;
            let mut write_pos = index;

            while read_pos != self.run_queue_tail {
                self.run_queue[write_pos] = self.run_queue[read_pos];
                self.run_queue[read_pos] = None;
                write_pos = (write_pos + 1) % MAX_THREADS;
                read_pos = (read_pos + 1) % MAX_THREADS;
            }

            self.run_queue_tail = write_pos;
            self.run_queue_count -= 1;

            return Some(thread_id);
        }

        None
    }

    pub fn get_current_thread(&self) -> Option<ThreadId> {
        self.current_thread
    }

    pub fn set_current_thread(&mut self, thread_id: Option<ThreadId>) {
        if let Some(old_id) = self.current_thread {
            if let Some(thread) = &mut self.threads[old_id] {
                if thread.state == ThreadState::Running {
                    thread.state = ThreadState::Ready;
                }
            }
        }

        self.current_thread = thread_id;

        if let Some(new_id) = thread_id {
            if let Some(thread) = &mut self.threads[new_id] {
                thread.state = ThreadState::Running;
            }
        }
    }

    pub fn exit_current_thread(&mut self) {
        if let Some(current) = self.current_thread {
            let mut waiters_to_wake = [None; 4];

            if let Some(thread) = &mut self.threads[current] {
                thread.state = ThreadState::Finished;
                waiters_to_wake = thread.join_waiters;
            }

            // Wake up any threads waiting to join this one
            for waiter in waiters_to_wake.iter().flatten() {
                if let Some(waiter_thread) = &mut self.threads[*waiter] {
                    if waiter_thread.state == ThreadState::Blocked {
                        waiter_thread.state = ThreadState::Ready;
                        let _ = self.enqueue_thread(*waiter);
                    }
                }
            }
        }
    }

    pub fn join_thread(&mut self, target_id: ThreadId, current_id: ThreadId) -> ThreadResult<()> {
        if target_id >= MAX_THREADS {
            return Err(ThreadError::InvalidThreadId);
        }

        if let Some(target_thread) = &mut self.threads[target_id] {
            if target_thread.state == ThreadState::Finished {
                return Ok(()); // Already finished
            }

            // Add current thread to join waiters
            for slot in &mut target_thread.join_waiters {
                if slot.is_none() {
                    *slot = Some(current_id);

                    // Block current thread
                    if let Some(current_thread) = &mut self.threads[current_id] {
                        current_thread.state = ThreadState::Blocked;
                    }

                    return Ok(());
                }
            }

            Err(ThreadError::SchedulerFull)
        } else {
            Err(ThreadError::InvalidThreadId)
        }
    }

    pub fn get_thread(&self, thread_id: ThreadId) -> Option<&Thread> {
        self.threads[thread_id].as_ref()
    }

    pub fn get_thread_mut(&mut self, thread_id: ThreadId) -> Option<&mut Thread> {
        self.threads[thread_id].as_mut()
    }

    pub fn switch_context(&mut self, from_id: ThreadId, to_id: ThreadId) -> ThreadResult<()> {
        if let Some(from_thread) = self.get_thread(from_id) {
            if from_thread.check_stack_overflow() {
                return Err(ThreadError::StackOverflow);
            }
        }

        let from_thread = self.get_thread_mut(from_id);
        let from_context = if let Some(thread) = from_thread {
            &mut thread.context as *mut _
        } else {
            return Err(ThreadError::InvalidThreadId);
        };

        let to_thread = self.get_thread_mut(to_id);
        let to_context = if let Some(thread) = to_thread {
            &thread.context as *const _
        } else {
            return Err(ThreadError::InvalidThreadId);
        };

        unsafe {
            crate::context::switch_context(from_context, to_context);
        }

        Ok(())
    }
}

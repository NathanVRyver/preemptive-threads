
pub type ThreadId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Finished,
}

#[repr(C)]
pub struct ThreadContext {
    pub rsp: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rflags: u64,
    pub rip: u64,
}

pub struct Thread {
    pub id: ThreadId,
    pub state: ThreadState,
    pub context: ThreadContext,
    pub stack: &'static mut [u8],
    pub stack_top: *mut u8,
    pub stack_bottom: *mut u8,
    pub entry_point: fn(),
    pub priority: u8,
    pub stack_guard: u64,
    pub join_waiters: [Option<ThreadId>; 4],
}

impl Thread {
    pub const STACK_SIZE: usize = 64 * 1024;

    pub fn new(
        id: ThreadId,
        stack: &'static mut [u8],
        entry_point: fn(),
        priority: u8,
    ) -> Self {
        let stack_top = unsafe { stack.as_mut_ptr().add(stack.len()) };
        let stack_bottom = stack.as_mut_ptr();
        let stack_guard = 0xDEADBEEFCAFEBABE;
        
        unsafe {
            core::ptr::write(stack_bottom as *mut u64, stack_guard);
        }
        
        let mut thread = Thread {
            id,
            state: ThreadState::Ready,
            context: ThreadContext {
                rsp: 0,
                rbp: 0,
                rbx: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rflags: 0x202,
                rip: 0,
            },
            stack,
            stack_top,
            stack_bottom,
            entry_point,
            priority,
            stack_guard,
            join_waiters: [None; 4],
        };

        thread.initialize_stack();
        thread
    }

    fn initialize_stack(&mut self) {
        unsafe {
            let stack_ptr = self.stack_top as *mut u64;
            
            let stack_ptr = stack_ptr.sub(1);
            *stack_ptr = thread_wrapper as usize as u64;
            
            let stack_ptr = stack_ptr.sub(1);
            *stack_ptr = 0;
            
            self.context.rsp = stack_ptr as u64;
            self.context.rip = thread_entry as usize as u64;
        }
    }

    pub fn is_runnable(&self) -> bool {
        self.state == ThreadState::Ready || self.state == ThreadState::Running
    }

    pub fn check_stack_overflow(&self) -> bool {
        unsafe {
            let guard_value = core::ptr::read(self.stack_bottom as *const u64);
            guard_value != self.stack_guard
        }
    }
}

extern "C" fn thread_entry() {
    crate::sync::exit_thread();
}

extern "C" fn thread_wrapper() -> ! {
    crate::sync::exit_thread();
}
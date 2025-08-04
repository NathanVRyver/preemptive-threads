#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadError {
    MaxThreadsReached,
    InvalidThreadId,
    ThreadNotRunnable,
    StackOverflow,
    SchedulerFull,
    NotRunning,
    NotImplemented,
    MemoryAllocationFailed,
    InvalidArgument,
    DeadlockDetected,
    ResourceExhausted,
}

impl ThreadError {
    pub fn as_str(self) -> &'static str {
        match self {
            ThreadError::MaxThreadsReached => "Maximum number of threads reached",
            ThreadError::InvalidThreadId => "Invalid thread ID provided",
            ThreadError::ThreadNotRunnable => "Thread is not in a runnable state",
            ThreadError::StackOverflow => "Stack overflow detected",
            ThreadError::SchedulerFull => "Scheduler queue is full",
            ThreadError::NotRunning => "Thread is not running",
            ThreadError::NotImplemented => "Feature not implemented",
            ThreadError::MemoryAllocationFailed => "Memory allocation failed",
            ThreadError::InvalidArgument => "Invalid argument provided",
            ThreadError::DeadlockDetected => "Deadlock detected",
            ThreadError::ResourceExhausted => "System resources exhausted",
        }
    }
}

pub type ThreadResult<T> = Result<T, ThreadError>;

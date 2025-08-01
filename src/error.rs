#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadError {
    MaxThreadsReached,
    InvalidThreadId,
    ThreadNotRunnable,
    StackOverflow,
    SchedulerFull,
}

impl ThreadError {
    pub fn as_str(self) -> &'static str {
        match self {
            ThreadError::MaxThreadsReached => "Maximum number of threads reached",
            ThreadError::InvalidThreadId => "Invalid thread ID provided",
            ThreadError::ThreadNotRunnable => "Thread is not in a runnable state",
            ThreadError::StackOverflow => "Stack overflow detected",
            ThreadError::SchedulerFull => "Scheduler queue is full",
        }
    }
}

pub type ThreadResult<T> = Result<T, ThreadError>;
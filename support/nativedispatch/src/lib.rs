//! This crate provides a cross-platform interface to each target platform's
//! thread pool facility.

// --------------------------------------------------------------------------
// Backend implementations

#[cfg(target_os = "macos")]
mod dispatch;
#[cfg(target_os = "macos")]
use self::dispatch::QueueImpl;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use self::windows::QueueImpl;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod glib;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use self::glib::QueueImpl;

// --------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Queue {
    imp: QueueImpl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QueuePriority {
    High = 0,
    Medium = 1,
    Low = 2,
    Background = 3,
}

impl Queue {
    /// Get a global queue with a specified priority.
    pub fn global(pri: QueuePriority) -> Self {
        Self {
            imp: QueueImpl::global(pri),
        }
    }

    /// Get a global queue with `QueuePriority::High`.
    pub fn global_high() -> Self {
        Self::global(QueuePriority::High)
    }

    /// Get a global queue with `QueuePriority::Medium`.
    pub fn global_med() -> Self {
        Self::global(QueuePriority::Medium)
    }

    /// Get a global queue with `QueuePriority::Low`.
    pub fn global_low() -> Self {
        Self::global(QueuePriority::Low)
    }

    /// Get a global queue with `QueuePriority::Background`.
    pub fn global_bg() -> Self {
        Self::global(QueuePriority::Background)
    }

    /// Execute a closure asynchronously.
    pub fn invoke(&self, work: impl FnOnce() + Send + 'static) {
        self.imp.invoke(work)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn it_works() {
        let queue = Queue::global(QueuePriority::High);
        let barrier = Arc::new(Barrier::new(2));

        let c = barrier.clone();
        queue.invoke(move || {
            c.wait();
        });

        barrier.wait();
    }
}

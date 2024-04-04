#![cfg_attr(not(test), no_std)]

use core::{
    cell::RefCell,
    mem,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// AtomicQueue consists of the following two atomic operations;
/// - index update for read/write
/// - element replacement
#[derive(Debug)]
pub struct AtomicQueue<T, const N: usize> {
    read: AtomicUsize,
    write: AtomicUsize,
    buffer: [RefCell<mem::MaybeUninit<T>>; N],
    flags: [AtomicBool; N],
}

const ATOMIC_BOOL_FALSE: AtomicBool = AtomicBool::new(false);

impl<T, const N: usize> AtomicQueue<T, N> {
    /// Creates a new instance.
    /// The type parameter `N` is the buffer capacity including spaces.
    ///
    /// ```rust
    /// # use atomic_queue::AtomicQueue;
    /// let queue = AtomicQueue::<i32, 4>::new();
    /// assert_eq!(queue.capacity(), 3);
    /// ```
    ///
    /// ## Panics
    /// - If `N` is less than or equal to 1.
    pub const fn new() -> AtomicQueue<T, N> {
        if N < 2 {
            panic!("N must be larger than 1")
        }
        AtomicQueue {
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            buffer: unsafe { mem::zeroed() },
            flags: [ATOMIC_BOOL_FALSE; N],
        }
    }

    /// Enqueues the entry.
    /// Returns false if enqueue fails.
    ///
    /// ```rust
    /// # use atomic_queue::AtomicQueue;
    /// let queue = AtomicQueue::<i32, 2>::new();
    /// assert!(queue.enqueue(1));
    /// assert!(!queue.enqueue(2)); // fails because the buffer is full.
    /// ```
    pub fn enqueue(&self, entry: T) -> bool {
        if self.is_full() {
            return false;
        }
        match self
            .write
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| Some((v + 1) % N))
        {
            Ok(write) => {
                // If the entry is already locked, start over from the beginning.
                if self.flags[write]
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return self.enqueue(entry);
                }

                let _ = mem::replace(
                    // Safety: the entry at the `write` index is occupied by the above operation.
                    &mut *self.buffer[write].borrow_mut(),
                    mem::MaybeUninit::new(entry),
                );

                // release lock.
                self.flags[write].store(false, Ordering::SeqCst);

                true
            }
            // If the write index is changing, start over from the beginning.
            Err(_) => self.enqueue(entry),
        }
    }

    /// Dequeues the entry.
    /// Returns `None` if the queue is empty.
    ///
    /// ```rust
    /// # use atomic_queue::AtomicQueue;
    /// let queue = AtomicQueue::<i32, 2>::new();
    /// queue.enqueue(1);
    /// assert_eq!(queue.dequeue(), Some(1));
    /// assert_eq!(queue.dequeue(), None);
    /// ```
    pub fn dequeue(&self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        match self
            .read
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| Some((v + 1) % N))
        {
            Ok(read) => {
                // If the entry is already locked, start over from the beginning.
                if self.flags[read]
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return self.dequeue();
                }

                let value = mem::replace(
                    // Safety: the entry at the `read` index is occupied by the above operation.
                    &mut *self.buffer[read].borrow_mut(),
                    mem::MaybeUninit::uninit(),
                );
                // Safety: the value must be meaningful by the above operation.
                let value = unsafe { value.assume_init() };

                // release lock.
                self.flags[read].store(false, Ordering::SeqCst);

                Some(value)
            }
            // If the read index is changing, start over from the beginning.
            Err(_) => self.dequeue(),
        }
    }

    pub fn is_full(&self) -> bool {
        self.read.load(Ordering::SeqCst) == ((self.write.load(Ordering::SeqCst) + 1) % N)
    }

    pub fn is_empty(&self) -> bool {
        self.read.load(Ordering::SeqCst) == self.write.load(Ordering::SeqCst)
    }

    pub const fn capacity(&self) -> usize {
        N - 1
    }
}

unsafe impl<T, const N: usize> Send for AtomicQueue<T, N> where T: Send {}

unsafe impl<T, const N: usize> Sync for AtomicQueue<T, N> where T: Sync {}

impl<T, const N: usize> Iterator for AtomicQueue<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.dequeue()
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::sync::Arc;
    use tokio::{self, task::JoinHandle};

    use crate::AtomicQueue;

    #[test]
    fn basic() {
        let queue = super::AtomicQueue::<i32, 3>::new();
        assert!(queue.enqueue(0));
        assert!(queue.enqueue(1));
        assert!(!queue.enqueue(-1)); // discarded.
        assert_eq!(queue.dequeue(), Some(0));
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), None);
    }

    #[tokio::test]
    async fn concurrency() {
        fn spawn_enqueue<T: Copy + Send + Sync + 'static, const N: usize>(
            queue: &Arc<AtomicQueue<T, N>>,
            entry: T,
            count: usize,
        ) -> JoinHandle<()> {
            let enqueue = Arc::clone(queue);
            tokio::spawn(async move {
                for _ in 0..count {
                    while !enqueue.enqueue(entry) {}
                }
            })
        }

        let queue = Arc::new(super::AtomicQueue::<i32, 1024>::new());

        for _ in 0..3 {
            spawn_enqueue(&queue, 1337, 300);
        }

        let dequeue1 = Arc::clone(&queue);
        let task3 = tokio::spawn(async move {
            let mut c = 0;
            while c < 300 * 3 {
                while let Some(i) = dequeue1.dequeue() {
                    assert_eq!(i, 1337);
                    c += 1;
                }
            }
        });

        tokio::select! {
            _ = task3 => {}
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                panic!("timeout")
            }
        };
    }
}

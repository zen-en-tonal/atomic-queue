#![cfg_attr(not(test), no_std)]

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
pub struct AtomicQueue<T, const N: usize> {
    read: AtomicUsize,
    write: AtomicUsize,
    buffer: [UnsafeCell<Option<T>>; N],
}

impl<T, const N: usize> AtomicQueue<T, N> {
    pub const fn new() -> AtomicQueue<T, N> {
        if N < 2 {
            panic!("N must be larger than 1")
        }
        AtomicQueue {
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            buffer: unsafe { core::mem::zeroed() },
        }
    }

    pub fn enqueue(&self, entry: T) -> bool {
        if self.is_full() {
            return false;
        }
        loop {
            let Ok(write) = self
                .write
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |i| Some((i + 1) % N))
            else {
                continue;
            };
            unsafe {
                self.buffer[write].get().replace(Some(entry));
            }
            return true;
        }
    }

    pub fn dequeue(&self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        loop {
            let Ok(read) = self
                .read
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |i| Some((i + 1) % N))
            else {
                continue;
            };
            unsafe { return self.buffer[read].get().replace(None) }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.read.load(Ordering::Relaxed) == self.write.load(Ordering::Relaxed)
    }

    pub fn is_full(&self) -> bool {
        self.read.load(Ordering::Relaxed) == (self.write.load(Ordering::Relaxed) + 1) % N
    }

    pub fn capacity(&self) -> usize {
        N - 1
    }
}

unsafe impl<T, const N: usize> Send for AtomicQueue<T, N> where T: Send {}

unsafe impl<T, const N: usize> Sync for AtomicQueue<T, N> where T: Sync {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn basic() {
        let queue = super::AtomicQueue::<i32, 4>::new();
        queue.enqueue(0);
        queue.enqueue(1);
        assert_eq!(queue.dequeue(), Some(0));
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), None);
    }

    #[test]
    fn concurrency() {
        let queue = Arc::new(super::AtomicQueue::<i32, 4>::new());

        let enqueue1 = Arc::clone(&queue);
        let enqueue_handler1 = thread::spawn(move || {
            for _ in 0..10 {
                while !enqueue1.enqueue(1) {}
            }
        });

        let enqueue2 = Arc::clone(&queue);
        let enqueue_handler2 = thread::spawn(move || {
            for _ in 0..10 {
                while !enqueue2.enqueue(1) {}
            }
        });

        let dequeue1 = Arc::clone(&queue);
        let dequeue_handler1 = thread::spawn(move || {
            let mut c = 0;
            while c < 20 {
                while let Some(i) = dequeue1.dequeue() {
                    c += i;
                }
            }
        });

        enqueue_handler1.join().unwrap();
        enqueue_handler2.join().unwrap();
        dequeue_handler1.join().unwrap();
    }
}

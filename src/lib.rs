#![cfg_attr(not(test), no_std)]

use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Debug)]
pub struct AtomicQueue<T, const N: usize> {
    read: AtomicUsize,
    write: AtomicUsize,
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
    flags: [AtomicBool; N],
}

impl<T, const N: usize> AtomicQueue<T, N> {
    pub fn new() -> AtomicQueue<T, N> {
        if N < 2 {
            panic!("N must be larger than 1")
        }
        AtomicQueue {
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            buffer: core::array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
            flags: core::array::from_fn(|_| AtomicBool::new(false)),
        }
    }

    pub fn enqueue(&self, entry: T) -> bool {
        if self.is_full() {
            return false;
        }
        match self
            .write
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some((v + 1) % N))
        {
            Ok(write) => {
                // If the entry is already locked,
                // try from the start again.
                if self.flags[write]
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_err()
                {
                    return self.enqueue(entry);
                }

                unsafe { self.buffer[write].get().replace(MaybeUninit::new(entry)) };

                // release lock.
                self.flags[write].store(false, Ordering::Release);

                true
            }
            Err(_) => self.enqueue(entry),
        }
    }

    pub fn dequeue(&self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        match self
            .read
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some((v + 1) % N))
        {
            Ok(read) => {
                // If the entry is already locked,
                // try from the start again.
                if self.flags[read]
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_err()
                {
                    return self.dequeue();
                }

                let value = unsafe {
                    self.buffer[read]
                        .get()
                        .replace(MaybeUninit::uninit())
                        .assume_init()
                };

                // release lock.
                self.flags[read].store(false, Ordering::Release);

                Some(value)
            }
            Err(_) => self.dequeue(),
        }
    }

    pub fn is_full(&self) -> bool {
        self.read.load(Ordering::Relaxed) == ((self.write.load(Ordering::Relaxed) + 1) % N)
    }

    pub fn is_empty(&self) -> bool {
        self.read.load(Ordering::Relaxed) == self.write.load(Ordering::Relaxed)
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
        let queue = super::AtomicQueue::<i32, 3>::new();
        assert!(queue.enqueue(0));
        assert!(queue.enqueue(1));
        assert!(!queue.enqueue(-1)); // discarded.
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

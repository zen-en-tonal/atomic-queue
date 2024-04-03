use std::{sync::Arc, thread};

use criterion::{criterion_group, criterion_main, Criterion};

use atomic_queue::AtomicQueue;

fn bm1(c: &mut Criterion) {
    c.bench_function("8", |b| {
        b.iter(|| {
            let q = Arc::new(AtomicQueue::<i32, 8>::new());
            let enqueue = Arc::clone(&q);
            let enqueue_handler = thread::spawn(move || {
                for i in 0..5 {
                    while !enqueue.enqueue(i) {}
                }
            });
            let dequeue = Arc::clone(&q);
            let dequeue_handler = thread::spawn(move || {
                let mut c = 0;
                while c < 4 {
                    while let Some(_) = dequeue.dequeue() {
                        c += 1;
                    }
                }
            });
            enqueue_handler.join().unwrap();
            dequeue_handler.join().unwrap();
        })
    });
}

fn bm2(c: &mut Criterion) {
    c.bench_function("16", |b| {
        b.iter(|| {
            let q = Arc::new(AtomicQueue::<i32, 16>::new());
            let enqueue = Arc::clone(&q);
            let enqueue_handler = thread::spawn(move || {
                for i in 0..5 {
                    while !enqueue.enqueue(i) {}
                }
            });
            let dequeue = Arc::clone(&q);
            let dequeue_handler = thread::spawn(move || {
                let mut c = 0;
                while c < 4 {
                    while let Some(_) = dequeue.dequeue() {
                        c += 1;
                    }
                }
            });
            enqueue_handler.join().unwrap();
            dequeue_handler.join().unwrap();
        })
    });
}

criterion_group!(benches, bm1, bm2);
criterion_main!(benches);

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use atomic_queue::AtomicQueue;

fn bm1(c: &mut Criterion) {
    c.bench_function("1024", |b| {
        b.iter(|| {
            let q = Arc::new(AtomicQueue::<i32, 1024>::new());
            for i in 0..500 {
                while !q.enqueue(i) {}
            }
            while let Some(_) = q.dequeue() {}
        })
    });
}

fn bm2(c: &mut Criterion) {
    c.bench_function("1000", |b| {
        b.iter(|| {
            let q = Arc::new(AtomicQueue::<i32, 1000>::new());
            for i in 0..500 {
                while !q.enqueue(i) {}
            }
            while let Some(_) = q.dequeue() {}
        })
    });
}

criterion_group!(benches, bm1, bm2);
criterion_main!(benches);

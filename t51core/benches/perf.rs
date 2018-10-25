#![feature(integer_atomics)]
#[macro_use]
extern crate criterion;
extern crate indexmap;
extern crate t51core;

use criterion::black_box;
use criterion::Criterion;
use indexmap::IndexMap;
use std::any::TypeId;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use t51core::sync::RwCell;
use std::sync::RwLock;

pub struct Indexer<'a> {
    entity_map: &'a IndexMap<usize, (usize, usize, usize)>,
    comp_a: *const i32,
    comp_b: *const u64,
    comp_c: *mut u64,
}

impl<'a> Indexer<'a> {
    #[inline(always)]
    pub fn for_each<F>(&mut self, mut exec: F)
    where
        F: FnMut(&Indexer, &i32, &u64, &mut u64),
    {
        for (_, (comp_a, comp_b, comp_c)) in self.entity_map.iter() {
            let comp_a = unsafe { &*self.comp_a.offset(*comp_a as isize) };
            let comp_b = unsafe { &*self.comp_b.offset(*comp_b as isize) };
            let comp_c = unsafe { &mut *self.comp_c.offset(*comp_c as isize) };
            exec(self, comp_a, comp_b, comp_c);
        }
    }

    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = (&Indexer, &i32, &u64, &mut u64)> {
        self.entity_map.iter().map(move |(_, &(comp_a, comp_b, comp_c))| unsafe {
            let a: &i32 = &*self.comp_a.offset(comp_a as isize);
            let b: &u64 = &*self.comp_b.offset(comp_b as isize);
            let c: &mut u64 = &mut *self.comp_c.offset(comp_c as isize);

            (self, a, b, c)
        })
    }
}

struct BenchData {
    entity_map: IndexMap<usize, (usize, usize, usize)>,
    comp_a: Vec<i32>,
    comp_b: Vec<u64>,
    comp_c: Vec<u64>,
}

fn for_each_bench(c: &mut Criterion) {
    let mut entity_map: IndexMap<usize, (usize, usize, usize)> = IndexMap::new();
    let mut v1 = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    for i in 0..1000000 {
        entity_map.insert(i, (i, i, i));
        v1.push(i as i32);
        v2.push(i as u64);
        v3.push(i as u64);
    }

    let mut data = BenchData {
        entity_map,
        comp_a: v1,
        comp_b: v2,
        comp_c: v3,
    };

    c.bench_function("foreach 1", move |b| {
        b.iter(|| {
            let mut indexer = Indexer {
                entity_map: &data.entity_map,
                comp_a: data.comp_a.as_ptr(),
                comp_b: data.comp_b.as_ptr(),
                comp_c: data.comp_c.as_mut_ptr(),
            };

            let mut d = 0u64;

            indexer.for_each(|_, a, b, c| {
                if *a % 2 == 0 {
                    let val = *a as u64 + *b + *c;
                    d += val;
                }
            });

            black_box(d);
        })
    });
}

fn iter_bench(c: &mut Criterion) {
    let mut entity_map: IndexMap<usize, (usize, usize, usize)> = IndexMap::new();
    let mut v1 = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    for i in 0..1000000 {
        entity_map.insert(i, (i, i, i));
        v1.push(i as i32);
        v2.push(i as u64);
        v3.push(i as u64);
    }

    let mut data = BenchData {
        entity_map,
        comp_a: v1,
        comp_b: v2,
        comp_c: v3,
    };

    c.bench_function("iter 1", move |b| {
        b.iter(|| {
            let indexer = Indexer {
                entity_map: &data.entity_map,
                comp_a: data.comp_a.as_ptr(),
                comp_b: data.comp_b.as_ptr(),
                comp_c: data.comp_c.as_mut_ptr(),
            };

            let mut d = 0u64;

            for (_, a, b, c) in indexer.iter() {
                if *a % 2 == 0 {
                    let val = *a as u64 + *b + *c;
                    d += val;
                }
            }

            black_box(d);
        })
    });
}

fn fluent_bench(c: &mut Criterion) {
    let mut entity_map: IndexMap<usize, (usize, usize, usize)> = IndexMap::new();
    let mut v1 = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    for i in 0..1000000 {
        entity_map.insert(i, (i, i, i));
        v1.push(i as i32);
        v2.push(i as u64);
        v3.push(i as u64);
    }

    let mut data = BenchData {
        entity_map,
        comp_a: v1,
        comp_b: v2,
        comp_c: v3,
    };

    c.bench_function("fluent 1", move |b| {
        b.iter(|| {
            let indexer = Indexer {
                entity_map: &data.entity_map,
                comp_a: data.comp_a.as_ptr(),
                comp_b: data.comp_b.as_ptr(),
                comp_c: data.comp_c.as_mut_ptr(),
            };

            let d: u64 = indexer
                .iter()
                .filter(|(_id, a, _b, _c)| *a % 2 == 0)
                .map(|(_id, a, b, c)| *a as u64 + *b + *c)
                .sum();

            black_box(d);
        })
    });
}

fn indexmap_bench(c: &mut Criterion) {
    let lock = RwLock::new(15);
    let cell = RwCell::new(15, Arc::new(AtomicI64::new(0)));

    c.bench_function("indexmap", move |b| {
        b.iter(|| {
            for _ in 0..10000 {
                let mut res = lock.write().unwrap();
                *res = 8;
                black_box(&res);
            }
        })
    });

    c.bench_function("cell", move |b| {
        b.iter(|| {
            for _ in 0..10000 {
                let mut res = cell.write();
                *res = 9;
                black_box(&res);
            }
        })
    });
}

criterion_group!(benches, indexmap_bench);
criterion_main!(benches);

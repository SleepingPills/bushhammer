#![feature(integer_atomics)]
#[macro_use]
extern crate criterion;
extern crate indexmap;
extern crate t51core;

use criterion::black_box;
use criterion::Criterion;
use indexmap::IndexMap;
use rand::prelude::*;
use std::marker::PhantomData;

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
            let a: &i32 = &*self.comp_a.add(comp_a);
            let b: &u64 = &*self.comp_b.add(comp_b);
            let c: &mut u64 = &mut *self.comp_c.add(comp_c);

            (self, a, b, c)
        })
    }
}

pub struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

pub struct Iterable<'a> {
    len: usize,
    counter: usize,
    entities: *const (),
    comps: (*const i32, *const V3, *mut V3),
    _x: PhantomData<&'a usize>,
}

impl<'a> Iterator for Iterable<'a> {
    type Item = (&'a i32, &'a V3, &'a mut V3);

    #[inline]
    fn next(&mut self) -> Option<(&'a i32, &'a V3, &'a mut V3)> {
        unsafe {
            if self.counter < self.len {
                let bucket = &*(self.entities as *const (usize, usize, usize)).add(self.counter);

                self.counter += 1;

                Some((
                    &*self.comps.0.add(bucket.0),
                    &*self.comps.1.add(bucket.1),
                    &mut *self.comps.2.add(bucket.2),
                ))
            } else {
                None
            }
        }
    }
}

impl<'a> ExactSizeIterator for Iterable<'a> {
    fn len(&self) -> usize {
        self.len
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

fn loop_bench(c: &mut Criterion) {
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

    c.bench_function("loop 1", move |b| {
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

fn iter_loop_bench(c: &mut Criterion) {
    let mut entities: Vec<(usize, usize, usize)> = Vec::new();
    let mut v1 = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    for i in 0..5000 {
        entities.push((i, i, i));
        v1.push(i as i32);
        v2.push(V3 {
            x: i as f32,
            y: i as f32,
            z: i as f32,
        });
        v3.push(V3 {
            x: 0f32,
            y: 0f32,
            z: 0f32,
        });
    }

    c.bench_function("iterloop 1", move |b| {
        b.iter(|| {
            let iter = Iterable {
                len: entities.len(),
                counter: 0,
                entities: entities.as_ptr() as *const (),
                comps: (v1.as_ptr(), v2.as_ptr(), v3.as_mut_ptr()),
                _x: PhantomData,
            };

            let mut d = 0f32;

            for (a, b, c) in iter {
                if *a % 2 == 0 {
                    c.x = (*a as f32) * b.x;
                    c.y = (*a as f32) * b.y;
                    c.z = (*a as f32) * b.z;

                    d += c.x + c.y + c.z;
                }
            }

            black_box(d);
        })
    });
}

fn iter_loop_rand_bench(c: &mut Criterion) {
    let mut entities: Vec<(usize, usize, usize)> = Vec::new();
    let mut v1 = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    let mut rng = thread_rng();

    for i in 0..5000 {
        let idx = (
            rng.gen_range::<usize>(0, 9999),
            rng.gen_range::<usize>(0, 9999),
            rng.gen_range::<usize>(0, 9999),
        );
        entities.push(idx);
    }

    for i in 0..10000 {
        v1.push(i as i32);
        v2.push(V3 {
            x: i as f32,
            y: i as f32,
            z: i as f32,
        });
        v3.push(V3 {
            x: 0f32,
            y: 0f32,
            z: 0f32,
        });
    }

    c.bench_function("iterloop 1", move |b| {
        b.iter(|| {
            let iter = Iterable {
                len: entities.len(),
                counter: 0,
                entities: entities.as_ptr() as *const (),
                comps: (v1.as_ptr(), v2.as_ptr(), v3.as_mut_ptr()),
                _x: PhantomData,
            };

            let mut d = 0f32;

            for (a, b, c) in iter {
                if *a % 2 == 0 {
                    c.x = (*a as f32) * b.x;
                    c.y = (*a as f32) * b.y;
                    c.z = (*a as f32) * b.z;

                    d += c.x + c.y + c.z;
                }
            }

            black_box(d);
        })
    });
}

pub struct Adder;

pub trait AddTwo {
    fn add(&self, input: f32) -> f32;
}

impl AddTwo for Adder {
    #[inline]
    fn add(&self, input: f32) -> f32 {
        input + 2f32
    }
}

fn iter_boxed_and_raw(c: &mut Criterion) {
    let adder1 = Box::new(Adder {});
    let boxed: Box<AddTwo> = adder1;

    c.bench_function("boxed", move |b| {
        b.iter(|| {
            let d = boxed.add(5f32);;
            black_box(d);
        })
    });

    let adder = Adder {};

    c.bench_function("raw", move |b| {
        b.iter(|| {
            let d = adder.add(5f32);;
            black_box(d);
        })
    });
}

use t51core::sync::GuardCell;

fn rwcell(c: &mut Criterion) {
    let cell = GuardCell::guard();

    c.bench_function("rwcell", move |b| {
        b.iter(|| {
            cell.apply(|value| {
                black_box(value);
            });
        })
    });
}

// for_each_bench, loop_bench, fluent_bench,
criterion_group!(benches, rwcell);
criterion_main!(benches);

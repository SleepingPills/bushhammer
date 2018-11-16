#[macro_use]
extern crate criterion;
#[macro_use]
extern crate t51core;

use criterion::black_box;
use criterion::Criterion;
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::marker::PhantomData;
use t51core::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct C1 {
    x: f32,
    y: f32,
    z: f32,
}

impl C1 {
    fn new(init: f32) -> C1 {
        C1 {
            x: init,
            y: init,
            z: init,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct C2 {
    x: f32,
    y: f32,
    z: f32,
}

impl C2 {
    fn new(init: f32) -> C2 {
        C2 {
            x: init,
            y: init,
            z: init,
        }
    }
}

pub struct Iterable<'a> {
    len: usize,
    counter: usize,
    entities: &'a Vec<(usize, usize)>,
    comps: (*const C1, *mut C2),
}

impl<'a> Iterator for Iterable<'a> {
    type Item = (&'a C1, &'a mut C2);

    #[inline]
    fn next(&mut self) -> Option<(&'a C1, &'a mut C2)> {
        unsafe {
            if self.counter < self.len {
                let bucket = self.entities.get_unchecked(self.counter);

                self.counter += 1;

                Some((&*self.comps.0.add(bucket.0), &mut *self.comps.1.add(bucket.1)))
            } else {
                None
            }
        }
    }
}

fn iter_loop_rand_bench(c: &mut Criterion) {
    let mut entities: Vec<(usize, usize)> = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    let mut rng = thread_rng();

    for _ in 0..5000 {
        let idx = (rng.gen_range::<usize>(0, 99999), rng.gen_range::<usize>(0, 99999));
        entities.push(idx);
    }

    for i in 0..100000 {
        v2.push(C1::new(i as f32));
        v3.push(C2::new(0f32));
    }

    c.bench_function("Random Access Iterator Loop", move |b| {
        b.iter(|| {
            let iter = Iterable {
                len: entities.len(),
                counter: 0,
                entities: &entities,
                comps: (v2.as_ptr(), v3.as_mut_ptr()),
            };

            let mut c = 0f32;

            for (a, b) in iter {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            }

            black_box(c);
        })
    });
}

fn iter_loop_linear_bench(c: &mut Criterion) {
    let mut entities: Vec<(usize, usize)> = Vec::new();
    let mut v2 = Vec::new();
    let mut v3 = Vec::new();

    for i in 0..5000 {
        entities.push((i, i));

        v2.push(C1::new(i as f32));
        v3.push(C2::new(0f32));
    }

    c.bench_function("Linear Access Iterator Loop", move |b| {
        b.iter(|| {
            let iter = Iterable {
                len: entities.len(),
                counter: 0,
                entities: &entities,
                comps: (v2.as_ptr(), v3.as_mut_ptr()),
            };

            let mut c = 0f32;

            for (a, b) in iter {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            }

            black_box(c);
        })
    });
}

fn system_loop_linear_bench(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> System for TestSystem<'a> {
        require!(Read<'a, C1>, Write<'a, C2>);

        #[inline]
        fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: EntityStore) {
            let mut c = 0f32;

            for (a, b) in ctx.iter() {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            }

            black_box(c);
        }
    }

    // Create World
    let mut world = World::new();

    // Register Components
    world.register_component::<C1>();
    world.register_component::<C2>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    // Add Entities
    for i in 0..5000 {
        world.entities().create().with(C1::new(i as f32)).with(C2::new(0f32)).build();
    }

    world.process_transactions();

    c.bench_function("Linear Access System Loop", move |b| {
        b.iter(|| {
            world.process_systems();
        })
    });
}

fn system_loop_multi_shards(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> System for TestSystem<'a> {
        require!(Read<'a, C1>, Write<'a, C2>);

        #[inline]
        fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: EntityStore) {
            let mut c = 0f32;

            for (a, b) in ctx.iter() {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            }

            black_box(c);
        }
    }

    // Create World
    let mut world = World::new();

    // Register a truckload of components to fracture the data space into many shards
    world.register_component::<C1>();
    world.register_component::<C2>();
    world.register_component::<i8>();
    world.register_component::<i16>();
    world.register_component::<i32>();
    world.register_component::<i64>();
    world.register_component::<u8>();
    world.register_component::<u16>();
    world.register_component::<u32>();
    world.register_component::<u64>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    fn make_ent<T>(world: &mut World, specialized: T, i: i32)
    where
        T: 'static,
    {
        world
            .entities()
            .create()
            .with(specialized)
            .with(C1::new(i as f32))
            .with(C2::new(i as f32))
            .build();
    }

    // Add Entities
    for i in 0..625 {
        make_ent(&mut world, i as i8, i);
        make_ent(&mut world, i as i16, i);
        make_ent(&mut world, i as i32, i);
        make_ent(&mut world, i as i64, i);
        make_ent(&mut world, i as u8, i);
        make_ent(&mut world, i as u16, i);
        make_ent(&mut world, i as u32, i);
        make_ent(&mut world, i as u64, i);
    }

    world.process_transactions();

    c.bench_function("Sharded Access System Loop", move |b| {
        b.iter(|| {
            world.process_systems();
        })
    });
}

fn system_loop_foreach_ent(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> System for TestSystem<'a> {
        require!(Read<'a, C1>, Write<'a, C2>);

        #[inline]
        fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: EntityStore) {
            let mut c = 0f32;
            let entity_ids: Vec<_> = (0..5000).map(|id| id.into()).collect();

            ctx.for_each(&entity_ids, |(a, b)| {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            });

            black_box(c);
        }
    }

    // Create World
    let mut world = World::new();

    // Register Components
    world.register_component::<C1>();
    world.register_component::<C2>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    // Add Entities
    for i in 0..5000 {
        world.entities().create().with(C1::new(i as f32)).with(C2::new(0f32)).build();
    }

    world.process_transactions();

    c.bench_function("Foreach Ent System Loop", move |b| {
        b.iter(|| {
            world.process_systems();
        })
    });
}

criterion_group!(
    benches,
    iter_loop_rand_bench,
    iter_loop_linear_bench,
    system_loop_linear_bench,
    system_loop_multi_shards,
    system_loop_foreach_ent
);
criterion_main!(benches);

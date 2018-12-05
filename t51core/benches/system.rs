#[macro_use]
extern crate criterion;

extern crate t51core;
use criterion::black_box;
use criterion::Criterion;
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};
use std::marker::PhantomData;
use t51core::prelude::*;
use t51core_proc::Component;
use t51core::component::Component;

#[derive(Component, Serialize, Deserialize, Debug)]
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

#[derive(Component, Serialize, Deserialize, Debug)]
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

    impl<'a> RunSystem for TestSystem<'a> {
        type Data = (Read<'a, C1>, Write<'a, C2>);

        #[inline]
        fn run(&mut self, mut data: Context<Self::Data>, _tx: &mut TransactionContext) {
            let mut c = 0f32;

            for (a, b) in data.components() {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            }

            black_box(c);
        }
    }

    // Create World
    let mut world = World::default();

    // Register Components
    world.register_component::<C1>();
    world.register_component::<C2>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    // Build the world
    world.build();

    {
        let mut batcher = world.entities().batch::<(C1, C2)>();

        // Add Entities
        for i in 0..5000 {
            batcher.add(C1::new(i as f32), C2::new(0f32));
        }
    }

    world.process_transactions();

    c.bench_function("Linear Access System Loop", move |b| {
        b.iter(|| {
            world.process_systems();
        })
    });
}

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C3(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C4(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C5(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C6(u32);

fn system_loop_multi_shards(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> RunSystem for TestSystem<'a> {
        type Data = (Read<'a, C1>, Write<'a, C2>);

        #[inline]
        fn run(&mut self, mut data: Context<Self::Data>, _tx: &mut TransactionContext) {
            let mut c = 0f32;

            for (a, b) in data.components() {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            }

            black_box(c);
        }
    }

    // Create World
    let mut world = World::default();

    // Register a truckload of components to fracture the data space into many shards
    world.register_component::<C1>();
    world.register_component::<C2>();
    world.register_component::<C3>();
    world.register_component::<C4>();
    world.register_component::<C5>();
    world.register_component::<C6>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    // Build the world
    world.build();

    fn make_ent<T>(world: &mut World, specialized: T)
    where
        T: 'static + Component + Clone,
    {
        let mut builder = world.entities().batch::<(C1, C2, T)>();

        for i in 0..1250 {
            builder.add(C1::new(i as f32), C2::new(i as f32), specialized.clone());
        }
    }

    // Add Entities
    make_ent(&mut world, C3(0));
    make_ent(&mut world, C4(0));
    make_ent(&mut world, C5(0));
    make_ent(&mut world, C6(0));

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

    impl<'a> RunSystem for TestSystem<'a> {
        type Data = (Read<'a, C1>, Write<'a, C2>);

        #[inline]
        fn run(&mut self, mut   data: Context<Self::Data>, _tx: &mut TransactionContext) {
            let mut c = 0f32;
            let entity_ids: Vec<_> = (0..5000).map(|id| id.into()).collect();

            data.components().for_each(&entity_ids, |(a, b)| {
                b.x += a.x;
                b.y += a.y;
                b.z += a.z;
                c += b.x + b.y + b.z;
            });

            black_box(c);
        }
    }

    // Create World
    let mut world = World::default();

    // Register Components
    world.register_component::<C1>();
    world.register_component::<C2>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    // Build the world
    world.build();

    {
        let mut batcher = world.entities().batch::<(C1, C2)>();

        // Add Entities
        for i in 0..5000 {
            batcher.add(C1::new(i as f32), C2::new(0f32));
        }
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

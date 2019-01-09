#[macro_use]
extern crate criterion;

extern crate neutronium;
use criterion::Criterion;
use std::marker::PhantomData;
use neutronium::prelude::*;
use neutronium_proc::Component;

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C1(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C2(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C3(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C4(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C5(u32);

fn add_ents(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> RunSystem for TestSystem<'a> {
        type Data = Components<(Read<'a, C1>, Write<'a, C2>)>;

        #[inline]
        fn run(&mut self, _data: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {}
    }

    c.bench_function("Add Entity", move |b| {
        b.iter_with_setup(
            || {
                // Create World
                let mut world = World::default();

                // Register Components
                world.register_component::<C1>();
                world.register_component::<C2>();
                world.register_component::<C3>();
                world.register_component::<C4>();
                world.register_component::<C5>();

                // Register System
                world.register_system(TestSystem { _p: PhantomData });

                // Build World
                world.build();
                world
            },
            |mut world| {
                let entities = world.entities();

                {
                    let mut batcher = entities.batch::<(C1, C2)>();

                    for i in 0..2500 {
                        batcher.add(C1(i), C2(i));
                    }
                }

                {
                    let mut batcher = entities.batch::<(C2, C3)>();

                    for i in 0..2500 {
                        batcher.add(C2(i), C3(i));
                    }
                }

                {
                    let mut batcher = entities.batch::<(C3, C4)>();

                    for i in 0..2500 {
                        batcher.add(C3(i), C4(i));
                    }
                }

                {
                    let mut batcher = entities.batch::<(C4, C5)>();

                    for i in 0..2500 {
                        batcher.add(C4(i), C5(i));
                    }
                }

                world.process_transactions();
                world
            },
        )
    });
}

fn remove_ents(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> RunSystem for TestSystem<'a> {
        type Data = Components<(Read<'a, C1>, Write<'a, C2>)>;

        #[inline]
        fn run(&mut self, _data: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {}
    }

    c.bench_function("Remove Entity", move |b| {
        b.iter_with_setup(
            || {
                // Create World
                let mut world = World::default();

                // Register Components
                world.register_component::<C1>();
                world.register_component::<C2>();
                world.register_component::<C3>();
                world.register_component::<C4>();
                world.register_component::<C5>();

                // Register System
                world.register_system(TestSystem { _p: PhantomData });

                // Build World
                world.build();

                let entities = world.entities();

                {
                    let mut batcher = entities.batch::<(C1, C2)>();

                    for i in 0..2500 {
                        batcher.add(C1(i), C2(i));
                    }
                }

                {
                    let mut batcher = entities.batch::<(C2, C3)>();

                    for i in 0..2500 {
                        batcher.add(C2(i), C3(i));
                    }
                }

                {
                    let mut batcher = entities.batch::<(C3, C4)>();

                    for i in 0..2500 {
                        batcher.add(C3(i), C4(i));
                    }
                }

                {
                    let mut batcher = entities.batch::<(C4, C5)>();

                    for i in 0..2500 {
                        batcher.add(C4(i), C5(i));
                    }
                }

                world.process_transactions();
                world
            },
            |mut world| {
                let entities = world.entities();

                // These are moved
                for i in 0..10000 {
                    entities.remove(i.into())
                }

                world.process_transactions();
                world
            },
        )
    });
}

criterion_group!(benches, add_ents, remove_ents);
criterion_main!(benches);

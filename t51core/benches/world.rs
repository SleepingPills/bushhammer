#[macro_use]
extern crate criterion;
#[macro_use]
extern crate t51core;

use criterion::black_box;
use criterion::Criterion;
use rand::prelude::*;
use std::marker::PhantomData;
use t51core::prelude::*;

fn add_ents(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> System for TestSystem<'a> {
        require!(Read<'a, i8>, Write<'a, i16>);

        #[inline]
        fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: EntityStore) {}
    }

    // Create World
    let mut world = World::new();

    // Register Components
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

    c.bench_function("Add Entity", move |b| {
        b.iter(|| {
            let mut entities = world.entities();

            for i in 0..100 {
                entities.create().with(i as i8).with(i as i16).build();
            }

            for i in 0..100 {
                entities.create().with(i as i16).with(i as i16).with(i as i32).build();
            }

            for i in 0..100 {
                entities.create().with(i as i32).with(i as u8).with(i as u16).build();
            }

            for i in 0..100 {
                entities.create().with(i as u8).with(i as u16).with(i as u32).build();
            }

            for i in 0..100 {
                entities.create().with(i as u16).with(i as u32).with(i as u64).build();
            }

            world.process_transactions();
        })
    });
}

fn edit_ents(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> System for TestSystem<'a> {
        require!(Read<'a, i8>, Write<'a, i16>);

        #[inline]
        fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: EntityStore) {}
    }

    // Create World
    let mut world = World::new();

    // Register Components
    world.register_component::<i8>();
    world.register_component::<i16>();
    world.register_component::<i32>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    for i in 0..500 {
        world.entities().create().with(i as i8).with(i as i16).build();
    }

    world.process_transactions();

    c.bench_function("Edit Entity", move |b| {
        b.iter(|| {
            let mut entities = world.entities();

            // These are moved
            for i in 0..250 {
                entities.edit(i.into()).unwrap().with(i as i32).with(i as i16).commit();
            }

            // These are updated
            for i in 250..500 {
                entities.edit(i.into()).unwrap().with(i as i8).with(i as i16).commit();
            }

            world.process_transactions();
        })
    });
}

fn remove_ents(c: &mut Criterion) {
    struct TestSystem<'a> {
        _p: PhantomData<&'a ()>,
    }

    impl<'a> System for TestSystem<'a> {
        require!(Read<'a, i8>, Write<'a, i16>);

        #[inline]
        fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: EntityStore) {}
    }

    // Create World
    let mut world = World::new();

    // Register Components
    world.register_component::<i8>();
    world.register_component::<i16>();
    world.register_component::<i32>();

    // Register System
    world.register_system(TestSystem { _p: PhantomData });

    for i in 0..250 {
        world.entities().create().with(i as i8).with(i as i16).build();
    }

    for i in 0..250 {
        world.entities().create().with(i as i16).with(i as i32).build();
    }

    world.process_transactions();

    c.bench_function("Remove Entity", move |b| {
        b.iter(|| {
            let mut entities = world.entities();

            // These are moved
            for i in 0..500 {
                entities.remove(i.into())
            }

            world.process_transactions();
        })
    });
}

criterion_group!(benches, add_ents, edit_ents, remove_ents);
criterion_main!(benches);

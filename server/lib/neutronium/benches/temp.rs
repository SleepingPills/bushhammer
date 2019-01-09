#[macro_use]
extern crate criterion;

extern crate neutronium;
use criterion::black_box;
use criterion::Criterion;
use hashbrown::HashSet;
use serde_derive::{Deserialize, Serialize};
use neutronium::component::Component;
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
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C6(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C7(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C8(u32);

fn hashset_vs_bitset(c: &mut Criterion) {
    // Create World
    let mut world = World::default();

    // Register Components
    world.register_component::<C1>();
    world.register_component::<C2>();
    world.register_component::<C3>();
    world.register_component::<C4>();
    world.register_component::<C5>();
    world.register_component::<C6>();
    world.register_component::<C7>();
    world.register_component::<C8>();

    // Build the world
    world.build();

    c.bench_function("Hashset Insertion", move |b| {
        b.iter_with_setup(
            || {
                let mut hash_set = HashSet::new();

                hash_set.insert(C1::get_unique_id());
                hash_set.insert(C2::get_unique_id());
                hash_set.insert(C3::get_unique_id());
                hash_set.insert(C4::get_unique_id());
                hash_set.insert(C5::get_unique_id());
                hash_set.insert(C7::get_unique_id());
                hash_set.insert(C8::get_unique_id());
                hash_set
            },
            |mut hash_set| {
                hash_set.insert(C6::get_unique_id());
                black_box(hash_set);
            },
        )
    });

    c.bench_function("Drain", move |b| {
        b.iter_with_setup(
            || {
                let mut hash_set = HashSet::new();

                hash_set.insert(C1::get_unique_id());
                hash_set.insert(C2::get_unique_id());
                hash_set.insert(C3::get_unique_id());
                hash_set.insert(C4::get_unique_id());
                hash_set.insert(C5::get_unique_id());
                hash_set.insert(C7::get_unique_id());
                hash_set.insert(C8::get_unique_id());
                hash_set
            },
            |mut hash_set| {
                for id in hash_set.drain() {
                    black_box(id);
                }
            },
        )
    });

    c.bench_function("Bitset Insertion", move |b| {
        b.iter_with_setup(
            || {
                C1::get_unique_id()
                    + C2::get_unique_id()
                    + C3::get_unique_id()
                    + C4::get_unique_id()
                    + C5::get_unique_id()
                    + C7::get_unique_id()
                    + C8::get_unique_id()
            },
            |mut bit_set| {
                bit_set += C6::get_unique_id();
                black_box(bit_set);
            },
        )
    });

    c.bench_function("Bitset Drain", move |b| {
        b.iter_with_setup(
            || {
                C1::get_unique_id()
                    + C2::get_unique_id()
                    + C3::get_unique_id()
                    + C4::get_unique_id()
                    + C5::get_unique_id()
                    + C7::get_unique_id()
                    + C8::get_unique_id()
            },
            |bit_set| {
                for id in bit_set.decompose() {
                    black_box(id);
                }
            },
        )
    });
}

criterion_group!(benches, hashset_vs_bitset);
criterion_main!(benches);

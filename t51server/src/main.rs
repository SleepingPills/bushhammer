#![allow(unused_imports, dead_code, unused_variables)]
#[macro_use]
extern crate t51core;

use t51core::identity::EntityId;
use t51core::prelude::*;
use t51core::system::context::Context;
use t51core::system::SystemDef;

pub struct Goof<'a> {
    coll: Vec<&'a i32>,
}

impl<'a> System for Goof<'a> {
    require!(Read<'a, EntityId>, Read<'a, i32>, Write<'a, u64>);

    fn run(&mut self, mut data: Context<Self::JoinItem>, mut entities: EntityStore) {
        for (a, b, c) in data.iter() {
            *c = 5;
            self.coll.push(b);
        }

        data.for_each(&Vec::new(), |(a, b, c)| {
            *c = 5;
        });

        entities.create().with(15).with("123").build();
        entities.edit(15).unwrap().with(15).remove::<i32>().commit();
    }
}

fn main() {
    println!("*** YO YO YO {} ***", 1 << 16);
}

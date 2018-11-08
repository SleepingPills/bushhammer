#![allow(unused_imports, dead_code, unused_variables)]
#[macro_use]
extern crate t51core;

use t51core::object::EntityId;
use t51core::prelude::*;
use t51core::system::context::Context;
use t51core::system::SystemDef;

pub struct Goof<'a> {
    coll: Vec<&'a i32>,
}

impl<'a> System for Goof<'a> {
    require!(Read<'a, EntityId>, Read<'a, i32>, Write<'a, u64>);

    fn run(&mut self, mut data: Context<Self::JoinItem>, mut entities: EntityStore) {
        let (d, e, f) = data.get_entity(5).unwrap();

        for (a, b, c) in data.iter() {
            *c = 5;
            self.coll.push(b);
        }

        entities.create().with(15).with("123").build();
        entities.edit(15).unwrap().with(15).remove::<i32>().build();
    }
}

fn main() {}

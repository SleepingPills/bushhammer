#[allow(unused_imports)]
#[macro_use]
extern crate t51core;
extern crate indexmap;


use t51core::prelude::*;
use indexmap::IndexMap;


#[allow(dead_code, unused_variables)]
fn test3(comp: (IndexMap<usize, (usize, usize)>, ReadStore<i32>, WriteStore<i32>)) {
    for (id, a, b) in comp.join() {
        *b = 5;
    }
}


fn main() {
}

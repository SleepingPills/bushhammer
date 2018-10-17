#[macro_use]
extern crate t51core;
extern crate indexmap;


use t51core::prelude::*;
use indexmap::IndexMap;


/*fn test3(comp: (IndexMap<usize, (usize, usize)>, ReadStore<i32>, WriteStore<i32>)) {
    let ctx = comp.join();

    for (id, a, b) in ctx {
        *b = 5;
        let (c, d) = ctx.get(5);
    }
}*/


fn main() {
}


/*
struct ReadStore<T> {
    data: Rc<RefCell<Vec<T>>>,
    mapping: IndexMap<usize, usize>,
}


impl<T> ReadStore<T> {
    pub fn iter(&self) -> impl Iterator<Item=&T> {
        let ptr = self.data.borrow().as_ptr();

        unsafe {
            self.mapping.values().map(move |x| { &*ptr.offset(*x as isize) })
        }
    }
}


struct WriteStore<T> {
    data: Rc<RefCell<Vec<T>>>,
    mapping: IndexMap<usize, usize>,
}


impl<T> WriteStore<T> {
    pub fn iter(&self) -> impl Iterator<Item=&mut T> {
        let ptr = self.data.borrow_mut().as_mut_ptr();

        unsafe {
            self.mapping.values().map(move |x| { &mut *ptr.offset(*x as isize) })
        }
    }
}

struct Join2<A, B> where A: Iterator, B: Iterator{
    a: A,
    b: B,
}

impl<A, B> Join2<A, B> where A: Iterator, B: Iterator{
    pub fn iter(self) -> impl Iterator<Item=(A::Item, B::Item)> {
        self.a.zip(self.b)
    }
}

fn test(left: ReadStore<i32>, right: WriteStore<i32>) {
    let j = Join2{a: left.iter(), b: right.iter()};
    for (a, b) in j.iter() {
        *b = 5;
    }
}
*/

use std::sync::atomic::{AtomicI64, Ordering};
use std::collections::HashMap;
use anymap::AnyMap;
use std::hash::Hash;
use sync::RwCell;
use std::sync::Arc;


pub struct Entry {
    guard: Arc<AtomicI64>,
    mapping: AnyMap
}

pub struct Registry<K> where K: Eq + Hash {
    pub(crate) data: HashMap<K, Entry>
}

//impl<K> Registry<K> where K: Eq + Hash {
//    pub fn get<T:'static>(&self, key: &K) -> Rc<T>{
//        let rc_map = self.data.get(key).unwrap();
//        let mut ty_map = rc_map.borrow_mut();
//        let res = ty_map.write::<Rc<T>>().unwrap();
//        res.clone()
//    }
//}

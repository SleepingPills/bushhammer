use crate::alloc::VecPool;
use crate::object::{ComponentId, IdType, ShardId};
use hashbrown::HashMap;
use serde::de::DeserializeOwned;
use serde_json;
use std::any::Any;

pub(crate) type ShardKey = IdType;
pub(crate) type ComponentCoords = (usize, usize);

#[inline]
pub(crate) fn composite_key<'a>(keys: impl Iterator<Item = &'a ComponentId>) -> ShardKey {
    keys.fold(0 as ShardKey, |acc, cid| acc + cid.id)
}

#[derive(Debug)]
pub struct ShardedColumn<T> {
    data: VecPool<Vec<T>>,
}

impl<T> ShardedColumn<T> {
    #[inline]
    pub(crate) fn new() -> ShardedColumn<T> {
        ShardedColumn { data: VecPool::new() }
    }

    #[inline]
    pub(crate) fn get(&self, section: usize, loc: usize) -> Option<&T> {
        self.data.get(section).get(loc)
    }

    #[inline]
    pub(crate) fn push(&mut self, instance: T, section: usize) -> usize {
        let storage = self.data.get_mut(section);
        let loc = storage.len();
        storage.push(instance);
        loc
    }

    #[inline]
    pub(crate) fn update(&mut self, instance: T, section: usize, loc: usize) {
        self.data.get_mut(section)[loc] = instance;
    }

    #[inline]
    pub(crate) fn section_len(&self, section: usize) -> usize {
        self.data.get(section).len()
    }

    #[inline]
    pub(crate) fn get_data_ptr(&self, section: usize) -> *const T {
        self.data.get(section).as_ptr()
    }

    #[inline]
    pub(crate) fn get_data_mut_ptr(&mut self, section: usize) -> *mut T {
        self.data.get_mut(section).as_mut_ptr()
    }
}

pub trait Column {
    fn ingest_box(&mut self, boxed: Box<Any>, section: usize) -> usize;
    fn ingest_json(&mut self, json: String, section: usize) -> usize;
    fn update_box(&mut self, boxed: Box<Any>, section: usize, loc: usize);
    fn update_json(&mut self, json: String, section: usize, loc: usize);
    fn swap_remove(&mut self, section: usize, loc: usize);
    fn swap_remove_return(&mut self, section: usize, loc: usize) -> Box<Any>;
    fn new_section(&mut self) -> usize;
    fn section_len(&self, section: usize) -> usize;
}

impl<T> Column for ShardedColumn<T>
where
    T: 'static + DeserializeOwned,
{
    fn ingest_box(&mut self, boxed: Box<Any>, section: usize) -> usize {
        self.push(*boxed.downcast::<T>().expect("Incorrect boxed component"), section)
    }

    fn ingest_json(&mut self, json: String, section: usize) -> usize {
        self.push(serde_json::from_str(&json).expect("Error deserializing component"), section)
    }

    fn update_box(&mut self, boxed: Box<Any>, section: usize, loc: usize) {
        self.update(*boxed.downcast::<T>().expect("Incorrect boxed component"), section, loc)
    }

    fn update_json(&mut self, json: String, section: usize, loc: usize) {
        self.update(serde_json::from_str(&json).expect("Error deserializing component"), section, loc)
    }

    fn swap_remove(&mut self, section: usize, loc: usize) {
        unsafe {
            let storage = self.data.get_unchecked_mut(section);
            storage.swap_remove(loc);
        }
    }

    fn swap_remove_return(&mut self, section: usize, loc: usize) -> Box<Any> {
        unsafe {
            let storage = self.data.get_unchecked_mut(section);
            let instance = storage.swap_remove(loc);
            Box::new(instance)
        }
    }

    fn new_section(&mut self) -> usize {
        let section = self.data.len();
        self.data.push(Vec::new());
        section
    }

    fn section_len(&self, section: usize) -> usize {
        ShardedColumn::<T>::section_len(self, section)
    }
}

#[derive(Debug)]
pub struct Shard {
    pub(crate) id: ShardId,
    pub(crate) shard_key: ShardKey,
    pub(crate) sections: HashMap<ComponentId, usize>,
}

impl Shard {
    #[inline]
    pub(crate) fn new(id: ShardId, shard_key: ShardKey, sections: HashMap<ComponentId, usize>) -> Shard {
        Shard { id, shard_key, sections }
    }

    #[inline]
    pub(crate) fn get_section(&self, id: ComponentId) -> usize {
        self.sections[&id]
    }
}

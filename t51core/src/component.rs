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
        unsafe {
            let data = self.data.get_unchecked(section);
            data.get(loc)
        }
    }

    #[inline]
    pub(crate) fn push_to_section(&mut self, instance: T, section: usize) -> usize {
        unsafe {
            let storage = self.data.get_unchecked_mut(section);
            let loc = storage.len();
            storage.push(instance);
            loc
        }
    }

    #[inline]
    pub(crate) fn section_len(&self, section: usize) -> usize {
        unsafe { self.data.get_unchecked(section).len() }
    }

    #[inline]
    pub(crate) fn get_data_ptr(&self, section: usize) -> *const T {
        unsafe { self.data.get_unchecked(section).as_ptr() }
    }

    #[inline]
    pub(crate) fn get_data_mut_ptr(&mut self, section: usize) -> *mut T {
        unsafe { self.data.get_unchecked_mut(section).as_mut_ptr() }
    }
}

pub trait Column {
    fn ingest_box(&mut self, boxed: Box<Any>, section: usize) -> usize;
    fn ingest_json(&mut self, json: String, section: usize) -> usize;
    fn swap_remove(&mut self, section: usize, loc: usize);
    fn new_section(&mut self) -> usize;
    fn section_len(&self, section: usize) -> usize;
}

impl<T> Column for ShardedColumn<T>
where
    T: 'static + DeserializeOwned,
{
    fn ingest_box(&mut self, boxed: Box<Any>, section: usize) -> usize {
        self.push_to_section(*boxed.downcast::<T>().expect("Incorrect boxed component"), section)
    }

    fn ingest_json(&mut self, json: String, section: usize) -> usize {
        self.push_to_section(serde_json::from_str(&json).expect("Error deserializing component"), section)
    }

    fn swap_remove(&mut self, section: usize, loc: usize) {
        unsafe {
            let storage = self.data.get_unchecked_mut(section);
            storage.swap_remove(loc);
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

pub struct Shard {
    pub(crate) id: ShardId,
    pub(crate) shard_key: ShardKey,
    sections: HashMap<ComponentId, usize>,
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

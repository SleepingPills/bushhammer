use crate::alloc::VecPool;
use crate::object::{ComponentId, IdType, ShardId};
use hashbrown::HashMap;
use serde::de::DeserializeOwned;
use serde_json;
use std::any::Any;

pub(crate) type ShardKey = IdType;
pub(crate) type ComponentCoords = (usize, usize);

#[inline]
pub(crate) fn composite_key<'a>(keys: impl Iterator<Item=&'a ComponentId>) -> ShardKey {
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
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn get_item(&self, (section, loc): ComponentCoords) -> &T {
        unsafe { self.data.get_unchecked(section).get_unchecked(loc) }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn get_item_mut(&mut self, (section, loc): ComponentCoords) -> &mut T {
        unsafe { self.data.get_unchecked_mut(section).get_unchecked_mut(loc) }
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
    fn new_section(&mut self) -> usize;
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

    fn new_section(&mut self) -> usize {
        let section = self.data.len();
        self.data.push(Vec::new());
        section
    }
}

pub struct Shard {
    pub(crate) id: ShardId,
    sections: HashMap<ComponentId, usize>,
}

impl Shard {
    #[inline]
    pub(crate) fn new(id: ShardId, sections: HashMap<ComponentId, usize>) -> Shard {
        Shard { id, sections }
    }

    #[inline]
    pub(crate) fn get_loc(&self, id: ComponentId) -> usize {
        self.sections[&id]
    }
}

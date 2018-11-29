use crate::alloc::VecPool;
use crate::entity2::EntityId;
use crate::identity2::{ComponentId, ShardKey};
use hashbrown::HashMap;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::fmt::Debug;
use crate::entity2::dynamic::DynVec;

pub(crate) type ComponentCoords = (usize, usize);

pub trait Component: DeserializeOwned + Debug {
    fn acquire_unique_id() -> ComponentId;
    fn get_unique_id() -> ComponentId;

    #[inline]
    fn get_type_indexer() -> usize {
        Self::get_unique_id().indexer()
    }

    #[inline]
    fn get_type_name() -> &'static str {
        unsafe { ComponentId::get_name_vec()[Self::get_type_indexer()] }
    }
}

#[derive(Debug)]
pub struct ShardedColumn<T> {
    data: VecPool<Vec<T>>,
    coords: HashMap<EntityId, ComponentCoords>,
}

impl<T> ShardedColumn<T> {
    #[inline]
    pub(crate) fn new() -> ShardedColumn<T> {
        ShardedColumn { data: VecPool::new(), coords: HashMap::new() }
    }

    #[inline]
    pub(crate) fn get_coords(&self, id: EntityId) -> Option<ComponentCoords> {
        self.coords.get(&id).and_then(|coords| Some(*coords))
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
    fn ingest_entity_ids(&mut self, entity_ids: &Vec<EntityId>, section: usize);
    fn ingest_component_data(&mut self, data: &mut DynVec, section: usize);
    fn swap_remove(&mut self, section: usize, loc: usize);
    fn new_section(&mut self) -> usize;
    fn section_len(&self, section: usize) -> usize;
}

impl<T> Column for ShardedColumn<T>
where
    T: 'static + Component,
{
    fn ingest_entity_ids(&mut self, entity_ids: &Vec<EntityId>, section: usize) {
        let mut loc = self.section_len(section);

        for &eid in entity_ids {
            self.coords.insert(eid, (section, loc));
            loc += 1;
        }
    }

    fn ingest_component_data(&mut self, data: &mut DynVec, section: usize) {
        let incoming = data.cast_mut::<T>();
        let section_data = self.data.get_mut(section);
        section_data.append(incoming);
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

#[derive(Debug)]
pub struct Shard {
    pub(crate) shard_key: ShardKey,
    pub(crate) sections: HashMap<ComponentId, usize>,
}

impl Shard {
    #[inline]
    pub(crate) fn new(shard_key: ShardKey, sections: HashMap<ComponentId, usize>) -> Shard {
        Shard { shard_key, sections}
    }

    #[inline]
    pub(crate) fn get_section(&self, id: ComponentId) -> usize {
        self.sections[&id]
    }
}

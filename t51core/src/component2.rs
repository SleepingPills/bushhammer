use crate::alloc::VecPool;
use crate::entity2::dynamic::DynVec;
use crate::entity2::EntityId;
use crate::identity2::{ComponentId, ShardKey};
use hashbrown::HashMap;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::fmt::Debug;

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
    entity_ids: VecPool<Vec<EntityId>>,
    coords: HashMap<EntityId, ComponentCoords>,
}

impl<T> ShardedColumn<T> {
    #[inline]
    pub(crate) fn new() -> ShardedColumn<T> {
        ShardedColumn {
            data: VecPool::new(),
            entity_ids: VecPool::new(),
            coords: HashMap::new(),
        }
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
    pub(crate) fn ingest_core(&mut self, data: &mut Vec<T>, section: usize) {
        let storage = self.data.get_mut(section);
        storage.append(data);
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
    fn ingest_entity_ids(&mut self, entity_ids: &[EntityId], section: usize);
    fn ingest_component_data(&mut self, data: &mut DynVec, section: usize);
    fn ingest(&mut self, entity_ids: &[EntityId], data: &mut DynVec, section: usize);
    fn swap_remove(&mut self, entity_id: EntityId , section: usize);
    fn new_section(&mut self) -> usize;
    fn section_len(&self, section: usize) -> usize;
}

impl<T> Column for ShardedColumn<T>
where
    T: 'static + Component,
{
    #[inline]
    fn ingest_entity_ids(&mut self, entity_ids: &[EntityId], section: usize) {
        let mut loc = self.section_len(section);
        let section_entity_ids = self.entity_ids.get_mut(section);

        for &eid in entity_ids {
            self.coords.insert(eid, (section, loc));
            section_entity_ids.push(eid);
            loc += 1;
        }
    }

    #[inline]
    fn ingest_component_data(&mut self, data: &mut DynVec, section: usize) {
        self.ingest_core(data.cast_mut::<T>(), section);
    }

    #[inline]
    fn ingest(&mut self, entity_ids: &[EntityId], data: &mut DynVec, section: usize) {
        self.ingest_entity_ids(entity_ids, section);
        self.ingest_component_data(data, section);
    }

    #[inline]
    fn swap_remove(&mut self, entity_id: EntityId, section: usize) {
        let storage = self.data.get_mut(section);
        let eid_storage = self.entity_ids.get_mut(section);

        unsafe {
            if let Some((_, loc)) = self.coords.remove(&entity_id) {
                storage.swap_remove(loc);
                let eid_swapped = *eid_storage.get_unchecked(eid_storage.len() - 1);
                eid_storage.swap_remove(loc);

                // Overwrite coords of swapped in entity
                self.coords.insert(eid_swapped, (section, loc));
            }
        }
    }

    #[inline]
    fn new_section(&mut self) -> usize {
        let section = self.data.len();
        self.data.push(Vec::new());
        self.entity_ids.push(Vec::new());
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
        Shard { shard_key, sections }
    }

    #[inline]
    pub(crate) fn get_section(&self, id: ComponentId) -> usize {
        self.sections[&id]
    }
}

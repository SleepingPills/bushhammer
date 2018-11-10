use crate::alloc::VecPool;
use crate::object::{ComponentId, ShardId};
use hashbrown::HashMap;
use std::any::Any;
use serde_json;
use serde::de::DeserializeOwned;

pub(crate) type ComponentCoords = (usize, usize);

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
}

impl<T> Column for ShardedColumn<T> where T:'static + DeserializeOwned {
    fn ingest_box(&mut self, boxed: Box<Any>, section: usize) -> usize {
        self.push_to_section(*boxed.downcast::<T>().expect("Incorrect boxed component"), section)
    }

    fn ingest_json(&mut self, json: String, section: usize) -> usize {
        self.push_to_section(serde_json::from_str(&json).expect("Error deserializing component"), section)
    }
}

pub struct Shard {
    pub(crate) id: ShardId,
    sections: HashMap<ComponentId, usize>,
}

impl Shard {
    #[inline]
    pub(crate) fn get_loc(&self, id: ComponentId) -> usize {
        self.sections[&id]
    }
}

//pub trait Store {
//    fn add_component(&mut self, id: ComponentId, ptr: VoidPtr) -> usize;
//    fn add_component_json(&mut self, id: ComponentId, json: String) -> usize;;
//}
//
//impl<T: 'static> Store for ComponentStore<T> {
//    #[inline]
//    fn add_component(&mut self, id: ComponentId, ptr: VoidPtr) -> usize {
//        unsafe {
//            let instance = *Box::from_raw(ptr.cast::<T>().as_ptr());
//            let index = self.data.len();
//            self.data.push(instance);
//            index
//        }
//    }
//
//    #[inline]
//    fn add_component_json(&mut self, id: ComponentId, json: String) -> usize {
//        unimplemented!()
//    }
//}

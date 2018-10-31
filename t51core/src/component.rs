use std::sync::Arc;
use crate::alloc::VecPool;
use crate::sync::RwCell;
use crate::object::ComponentId;

pub struct ComponentStore<T> {
    pub(crate) pool: VecPool<T>,
}

impl<T> ComponentStore<T> {
    #[inline]
    pub unsafe fn get_pool_ptr(&self) -> *const T {
        self.pool.get_store_ptr()
    }

    #[inline]
    pub unsafe fn get_pool_mut_ptr(&mut self) -> *mut T {
        self.pool.get_store_mut_ptr()
    }
}

pub type ComponentField<T> = Arc<RwCell<ComponentStore<T>>>;


pub trait ComponentManager {
    fn add_component(&mut self, id: ComponentId, ptr: *const ()) -> usize;
    fn add_component_json(&mut self, id: ComponentId, json: String) -> usize;
    fn reclaim(&mut self, index: usize);
}

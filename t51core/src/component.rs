use crate::alloc::VoidPtr;
use crate::object::{BundleId, ComponentId, EntityId};
use crate::system::support::BundleDef;
use std::collections::HashMap;

pub struct ComponentStore<T> {
    pub(crate) data: Vec<T>,
}

pub trait Store {
    fn add_component(&mut self, id: ComponentId, ptr: VoidPtr) -> usize;
    fn add_component_json(&mut self, id: ComponentId, json: String) -> usize;

    unsafe fn get_vec_ptr(&self) -> VoidPtr;
}

impl<T: 'static> Store for ComponentStore<T> {
    #[inline]
    fn add_component(&mut self, id: ComponentId, ptr: VoidPtr) -> usize {
        unsafe {
            let instance = *Box::from_raw(ptr.cast::<T>().as_ptr());
            let index = self.data.len();
            self.data.push(instance);
            index
        }
    }

    #[inline]
    fn add_component_json(&mut self, id: ComponentId, json: String) -> usize {
        unimplemented!()
    }

    #[inline]
    unsafe fn get_vec_ptr(&self) -> VoidPtr {
        VoidPtr::new_unchecked(&self.data as *const _ as *mut ())
    }
}

pub struct ComponentBundle {
    id: BundleId,
    components: HashMap<ComponentId, Box<Store>>,
    entities: HashMap<EntityId, usize>,
}

pub trait Bundle {
    fn query(&self, request: Vec<ComponentId>) -> BundleDef;
}

impl Bundle for ComponentBundle {
    fn query(&self, request: Vec<ComponentId>) -> BundleDef {
        unsafe {
            BundleDef(
                self.id,
                &self.entities as *const _,
                request.iter().map(|cid| self.components[cid].get_vec_ptr()).collect(),
            )
        }
    }
}

pub struct BundleRegistry {}

pub trait ComponentManager {
    fn add_component(&mut self, id: ComponentId, ptr: *const ()) -> usize;
    fn add_component_json(&mut self, id: ComponentId, json: String) -> usize;
    fn reclaim(&mut self, index: usize);
}

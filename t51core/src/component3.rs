use crate::component2::Component;
use crate::entity2::dynamic::DynVec;
use crate::entity2::{EntityId, ShardDef};
use crate::identity2::{ComponentId, ShardKey};
use hashbrown::HashMap;

pub trait ComponentVec {
    fn ingest(&mut self, data: &mut DynVec);
    fn remove(&mut self, loc: usize);
    unsafe fn get_ptr(&self) -> *mut ();
}

impl<T> ComponentVec for Vec<T>
where
    T: Component,
{
    #[inline]
    fn ingest(&mut self, data: &mut DynVec) {
        let data_vec = data.cast_mut::<T>();
        self.append(data_vec);
    }

    #[inline]
    fn remove(&mut self, loc: usize) {
        self.swap_remove(loc);
    }

    #[inline]
    unsafe fn get_ptr(&self) -> *mut () {
        self as *const Vec<T> as *mut ()
    }
}

pub struct Shard {
    pub(crate) key: ShardKey,
    entities: Vec<EntityId>,
    locs: HashMap<EntityId, usize>,
    store: HashMap<ComponentId, Box<ComponentVec>>,
}

impl Shard {
    pub fn ingest(&mut self, shard_def: &mut ShardDef) {
        for (id, data) in shard_def.components.iter_mut() {
            self.store.get_mut(id).unwrap().ingest(data);
        }

        let mut loc_count = self.entities.len();

        self.entities.extend(&shard_def.entity_ids);

        for &entity_id in shard_def.entity_ids.iter() {
            self.locs.insert(entity_id, loc_count);
            loc_count += 1;
        }

        unsafe {
            let entity_vec = &mut *(self.store[&EntityId::get_unique_id()].get_ptr() as *mut Vec<EntityId>);
            entity_vec.append(&mut shard_def.entity_ids);
        }
    }

    pub fn remove(&mut self, id: EntityId) {
        if let Some(del_loc) = self.locs.remove(&id) {
            self.entities.swap_remove(del_loc);
            let swapped_id = self.entities[del_loc];
            self.locs.insert(swapped_id, del_loc);

            for data in self.store.values_mut() {
                data.remove(del_loc);
            }
        }
    }

    #[inline]
    pub fn data_ptr<T>(&self) -> *const Vec<T>
    where
        T: Component,
    {
        unsafe { self.store.get(&T::get_unique_id()).unwrap().get_ptr() as *const Vec<T> }
    }

    #[inline]
    pub fn data_mut_ptr<T>(&self) -> *mut Vec<T>
        where
            T: Component,
    {
        if T::get_unique_id() == EntityId::get_unique_id() {
            panic!("Entity ID vector is not writeable")
        }

        unsafe { self.store.get(&T::get_unique_id()).unwrap().get_ptr() as *mut  Vec<T> }
    }
}

use crate::component2::Component;
use crate::entity2::dynamic::DynVec;
use crate::entity2::{EntityId, ShardDef};
use crate::identity2::{ComponentId, ShardKey};
use hashbrown::HashMap;
use std::ops::Range;

pub(crate) type ComponentCoords = (ShardKey, usize);

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
    store: HashMap<ComponentId, Box<ComponentVec>>,
}

impl Shard {
    pub fn ingest(&mut self, shard_def: &mut ShardDef) -> Range<usize> {
        for (id, data) in shard_def.components.iter_mut() {
            self.store.get_mut(id).unwrap().ingest(data);
        }

        let loc_count = self.entities.len();

        self.entities.append(&mut shard_def.entity_ids);

        (loc_count..self.entities.len())
    }

    #[inline]
    pub fn remove(&mut self, loc: usize) -> Option<EntityId> {
        self.entities.swap_remove(loc);

        for data in self.store.values_mut() {
            data.remove(loc);
        }

        self.entities.get(loc).and_then(|eid| Some(*eid))
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

/*
TODO: Entity removal/lookup handling solution below

The world will store the full (ShardKey, Loc) coords of each entity. This allows quick lookups and removes
the need to maintain this info per shard.

 - Ingest returns the locations of the newly added entities. This is just a range so it's efficient.
 - Remove will remove the entries at the location looked up in the registry in World. It will return the id of
   the swapped entity if there is anything in the vector remaining.
*/

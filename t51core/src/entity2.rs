use crate::component::ShardKey;
use crate::component::{key_count, ComponentCoords};
use crate::identity::{ComponentId, EntityId, ShardId};
use hashbrown::HashMap;
use std::any::Any;
use std::any::TypeId;
use std::ops::Deref;

/// Entity root object. Maintains a registry of components and indices, along with the systems
/// it is registerered with.
#[derive(Debug)]
pub struct Entity {
    pub id: EntityId,
    pub shard_id: ShardId,
    pub shard_loc: usize,
    pub comp_sections: HashMap<ComponentId, usize>,
}

impl Entity {
    #[inline]
    pub(crate) fn get_coords(&self, comp_id: &ComponentId) -> ComponentCoords {
        (self.comp_sections[comp_id], self.shard_loc)
    }
}

#[derive(Debug)]
pub struct AnyVec {
    inst: Box<Any>,
    ptr: *mut (),
}

impl AnyVec {
    pub fn new<T>(instance: Vec<T>) -> AnyVec
    where
        T: 'static,
    {
        let mut inst: Box<Any> = Box::new(instance);
        let ptr = inst.downcast_mut::<Vec<T>>().unwrap() as *mut Vec<T> as *mut ();

        AnyVec { inst, ptr }
    }

    #[inline]
    pub fn cast<T>(&self) -> &Vec<T> {
        unsafe { &*(self.ptr as *const Vec<T>) }
    }

    #[inline]
    pub unsafe fn cast_mut<T>(&self) -> &mut Vec<T> {
        &mut *(self.ptr as *mut Vec<T>)
    }
}

#[derive(Debug)]
pub struct TransactionContext {
    added: HashMap<ShardKey, HashMap<ComponentId, AnyVec>>,
    deleted: HashMap<ShardId, Vec<EntityId>>,
    component_ids: HashMap<TypeId, ComponentId>,
}

impl TransactionContext {
    pub fn batch<'a, T>(&'a mut self) -> T::Batcher
    where
        T: BatchDef<'a>,
    {
        T::new(&mut self.added, &self.component_ids)
    }
}

pub trait BatchDef<'a> {
    type Batcher;

    fn new(
        shard_map: &'a mut HashMap<ShardKey, HashMap<ComponentId, AnyVec>>,
        component_ids: &HashMap<TypeId, ComponentId>,
    ) -> Self::Batcher;
}

// TODO: To macro
impl<'a, A, B, C> BatchDef<'a> for (A, B, C)
where
    A: 'static,
    B: 'static,
    C: 'static,
{
    type Batcher = (&'a mut Vec<A>, &'a mut Vec<B>, &'a mut Vec<C>);

    fn new(
        shard_map: &'a mut HashMap<ShardKey, HashMap<ComponentId, AnyVec>>,
        component_ids: &HashMap<TypeId, ComponentId>,
    ) -> Self::Batcher {
        let comp_id_a = component_ids[&TypeId::of::<A>()];
        let comp_id_b = component_ids[&TypeId::of::<B>()];
        let comp_id_c = component_ids[&TypeId::of::<C>()];

        let shard_key = comp_id_a.id | comp_id_b.id | comp_id_c.id;

        // Ensure that all types are distinct and no duplicate mutable entries are returned.
        if key_count(shard_key) != 3 {
            panic!("Invalid shard key rank")
        }

        let vec_map = shard_map.entry(shard_key).or_insert_with(|| {
            let mut map = HashMap::new();
            map.insert(comp_id_a, AnyVec::new(Vec::<A>::new()));
            map.insert(comp_id_b, AnyVec::new(Vec::<B>::new()));
            map.insert(comp_id_c, AnyVec::new(Vec::<C>::new()));
            map
        });

        unsafe {
            (
                vec_map[&comp_id_a].cast_mut::<A>(),
                vec_map[&comp_id_a].cast_mut::<B>(),
                vec_map[&comp_id_a].cast_mut::<C>(),
            )
        }
    }
}

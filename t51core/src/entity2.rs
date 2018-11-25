use crate::component2::{Component, ComponentCoords};
use crate::identity2::{ComponentId, ShardKey};
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use t51core_proc::Component;

#[repr(transparent)]
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct EntityId(usize);

impl From<usize> for EntityId {
    #[inline]
    fn from(id: usize) -> Self {
        EntityId(id)
    }
}

impl Into<usize> for EntityId {
    #[inline]
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for EntityId {
    #[inline]
    fn from(id: u32) -> Self {
        EntityId(id as usize)
    }
}

impl From<i32> for EntityId {
    #[inline]
    fn from(id: i32) -> Self {
        EntityId(id as usize)
    }
}

/// Entity root object. Maintains a registry of components and indices, along with the systems
/// it is registerered with.
#[derive(Debug)]
pub struct Entity {
    pub id: EntityId,
    pub shard_key: ShardKey,
    pub shard_loc: usize,
    pub comp_sections: HashMap<ComponentId, usize>,
}

impl Entity {
    #[inline]
    pub(crate) fn get_coords(&self, comp_id: ComponentId) -> ComponentCoords {
        (self.comp_sections[&comp_id], self.shard_loc)
    }
}

/// Context for recording entity transactions. Prepared by the `World` after all components have been
/// registered and the world is finalized.
#[derive(Debug, Clone)]
pub struct TransactionContext {
    pub(crate) added: HashMap<ShardKey, HashMap<ComponentId, dynamic::DynVec>>,
    pub(crate) deleted: Vec<EntityId>,
    pub(crate) builders: Vec<Box<dynamic::BuildDynVec>>,
    pub(crate) id_counter: Arc<AtomicUsize>,
}

impl TransactionContext {
    pub fn new(counter: Arc<AtomicUsize>) -> TransactionContext {
        TransactionContext {
            added: HashMap::new(),
            deleted: Vec::new(),
            builders: Vec::new(),
            id_counter: counter,
        }
    }

    /// Create a batch entity builder optimized for rapidly adding entities with the same components.
    #[inline]
    pub fn batch<'a, T>(&'a mut self) -> T::Builder
    where
        T: BatchDef<'a>,
    {
        T::new_batch_builder(self)
    }

    /// Create a batch entity builder for ingesting JSON data
    pub fn batch_json<'i>(&'i mut self, comp_ids: &'i [ComponentId]) -> JsonBatchBuilder<'i> {
        let entity_comp_id = EntityId::get_unique_id();
        let shard_key = ShardKey::from_iter(comp_ids.iter()) + entity_comp_id;

        let builders = &self.builders;
        let shard = self.added.entry(shard_key).or_insert_with(|| {
            let mut map: HashMap<_, _> = comp_ids.iter().map(|id| (*id, builders[id.indexer()].build())).collect();
            map.insert(entity_comp_id, builders[entity_comp_id.indexer()].build());
            map
        });

        unsafe {
            JsonBatchBuilder {
                comp_ids,
                shard,
                //entity_vec: shard[&entity_comp_id].cast_mut_unchecked::<EntityId>(),
                id_counter: self.id_counter.clone(),
                batch_counter: 0,
            }
        }
    }

    /// Add a single entity with the supplied tuple of components.
    #[inline]
    pub fn add<'a, T>(&'a mut self, tuple: T) -> EntityId
    where
        T: ComponentIngress<'a>,
    {
        tuple.ingest(self)
    }

    /// Delete the entity with the given id.
    #[inline]
    pub fn delete(&mut self, id: EntityId) {
        self.deleted.push(id);
    }

    /// Add a vector builder for the given component type. This is required to be able to
    /// create shards for collecting 'weakly' typed input like json strings.
    pub(crate) fn add_builder<T>(&mut self)
    where
        T: 'static + Component,
    {
        self.builders.push(Box::new(dynamic::DynVecFactory::<T>(PhantomData)));
    }
}

pub struct JsonBatchBuilder<'a> {
    comp_ids: &'a [ComponentId],
    shard: &'a mut HashMap<ComponentId, dynamic::DynVec>,
    //entity_vec: &'a mut Vec<EntityId>,
    id_counter: Arc<AtomicUsize>,
    batch_counter: usize,
}

impl<'a> JsonBatchBuilder<'a> {
    #[inline]
    pub fn add(&mut self, json_str: &Vec<String>) {
        if self.comp_ids.len() != json_str.len() {
            panic!("Number of component Ids does not match the number of data inputs")
        }

        for (id, json) in self.comp_ids.iter().zip(json_str) {
            self.shard.get_mut(id).unwrap().push_json(json);
        }

        self.batch_counter += 1;
    }
    pub fn commit(&mut self) -> &Vec<EntityId> {
        // Bump the id counter by the number of recorded entries in the batch
        let start_id = self.id_counter.fetch_add(self.batch_counter, Ordering::AcqRel);

        // Generate entity Ids
//        for id in start_id..(start_id + self.batch_counter) {
//            self.entity_vec.push(EntityId(id));
//        }

        // Reset the batch counter
        self.batch_counter = 0;

//        self.entity_vec

        unimplemented!()
    }
}

impl<'a> Drop for JsonBatchBuilder<'a> {
    fn drop(&mut self) {
        self.commit();
    }
}

/// Tuple defining a batch builder that can efficiently add uniform entities.
pub trait BatchDef<'a>: ComponentTuple<'a> {
    type Builder;

    fn new_batch_builder(ctx: &'a mut TransactionContext) -> Self::Builder;
}

macro_rules! batch_def_tup {
    ($( $field_type:ident:$field_seq:tt ),*) => {
        impl<'a, $($field_type),*> BatchDef<'a> for ($($field_type),*,)
        where
            $($field_type: 'static + Component),*,
        {
            type Builder = BatchBuilder<'a, ($(&'a mut Vec<$field_type>),*,)>;

            fn new_batch_builder(ctx: &'a mut TransactionContext) -> Self::Builder {
                let ids = Self::get_ids();

                let id_counter = ctx.id_counter.clone();
                let shard = Self::get_shard(&ids, ctx);

                // The below is safe because of previous checks
                unsafe {
                    let tup = (
                        $(shard[&ids.$field_seq].cast_mut_unchecked::<$field_type>()),*,
                    );

                    let entity_vec = shard[&EntityId::get_unique_id()].cast_mut_unchecked::<EntityId>();

                    BatchBuilder::new(tup, entity_vec, id_counter)
                }
            }
        }
    };
}

batch_def_tup!(A:0);
batch_def_tup!(A:0, B:1);
batch_def_tup!(A:0, B:1, C:2);
batch_def_tup!(A:0, B:1, C:2, D:3);
batch_def_tup!(A:0, B:1, C:2, D:3, E:4);
batch_def_tup!(A:0, B:1, C:2, D:3, E:4, F:5);
batch_def_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
batch_def_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

pub struct BatchBuilder<'a, T> {
    tup: T,
    entity_vec: &'a mut Vec<EntityId>,
    id_counter: Arc<AtomicUsize>,
    batch_counter: usize,
}

impl<'a, T> BatchBuilder<'a, T> {
    #[inline]
    pub fn new(tup: T, entity_vec: &'a mut Vec<EntityId>, id_counter: Arc<AtomicUsize>) -> BatchBuilder<'a, T> {
        BatchBuilder {
            tup,
            entity_vec,
            id_counter,
            batch_counter: 0,
        }
    }

    pub fn commit(&mut self) -> &Vec<EntityId> {
        // Bump the id counter by the number of recorded entries in the batch
        let start_id = self.id_counter.fetch_add(self.batch_counter, Ordering::AcqRel);

        // Generate entity Ids
        for id in start_id..(start_id + self.batch_counter) {
            self.entity_vec.push(EntityId(id));
        }

        // Reset the batch counter
        self.batch_counter = 0;

        self.entity_vec
    }
}

impl<'a, T> Drop for BatchBuilder<'a, T> {
    fn drop(&mut self) {
        self.commit();
    }
}

macro_rules! batch_builder_tup {
    ($tup_name:ident, $( $field_type:ident:$field_name:ident:$field_seq:tt ),*) => {
        impl<'a, $($field_type),*> BatchBuilder<'a, ($(&'a mut Vec<$field_type>),*,)>
        where
            $($field_type: Component),*
        {
            #[inline]
            pub fn add(&mut self, $($field_name: $field_type),*) {
                self.batch_counter += 1;
                $(self.tup.$field_seq.push($field_name));*;
            }
        }
    };
}

batch_builder_tup!(B1, A:a:0);
batch_builder_tup!(B2, A:a:0, B:b:1);
batch_builder_tup!(B3, A:a:0, B:b:1, C:c:2);
batch_builder_tup!(B4, A:a:0, B:b:1, C:c:2, D:d:3);
batch_builder_tup!(B5, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4);
batch_builder_tup!(B6, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4, F:f:5);
batch_builder_tup!(B7, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4, F:f:5, G:g:6);
batch_builder_tup!(B8, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4, F:f:5, G:g:6, H:h:7);

/// Utility functionality for tuples of components
pub trait ComponentTuple<'a> {
    type IdTuple;

    fn get_ids() -> Self::IdTuple;
    fn get_shard(ids: &Self::IdTuple, ctx: &'a mut TransactionContext) -> &'a mut HashMap<ComponentId, dynamic::DynVec>;
}

macro_rules! _decl_entity_replace_expr {
    ($_t:tt $sub:ty) => {
        $sub
    };
}

macro_rules! comp_tup {
    ($field_count:tt, $( $field_type:ident:$field_seq:tt ),*) => {
        impl<'a, $($field_type),*> ComponentTuple<'a> for ($($field_type),*,)
        where
            $($field_type: 'static + Component),*,
        {
            type IdTuple = ($(_decl_entity_replace_expr!($field_type ComponentId)),*,);

            #[inline]
            fn get_ids() -> Self::IdTuple {
                (
                    $($field_type::get_unique_id()),*,
                )
            }

            fn get_shard(ids: &Self::IdTuple, ctx: &'a mut TransactionContext) -> &'a mut HashMap<ComponentId, dynamic::DynVec> {
                let entity_comp_id = EntityId::get_unique_id();
                let shard_key: ShardKey = ($(ids.$field_seq)|* | entity_comp_id).into();

                // Ensure that all types are distinct and no duplicate mutable entries are returned.
                // +1 is added to account for the Entity Id.
                if shard_key.count() != ($field_count + 1) {
                    panic!("Invalid shard key rank")
                }

                // Get a cached shard builder or create a new one if necessary
                ctx.added.entry(shard_key).or_insert_with(|| {
                    let mut map = HashMap::new();
                    map.insert(entity_comp_id, dynamic::DynVec::new(Vec::<EntityId>::new()));
                    $(map.insert(ids.$field_seq, dynamic::DynVec::new(Vec::<$field_type>::new())));*;
                    map
                })
            }
        }
    };
}

comp_tup!(1, A:0);
comp_tup!(2, A:0, B:1);
comp_tup!(3, A:0, B:1, C:2);
comp_tup!(4, A:0, B:1, C:2, D:3);
comp_tup!(5, A:0, B:1, C:2, D:3, E:4);
comp_tup!(6, A:0, B:1, C:2, D:3, E:4, F:5);
comp_tup!(7, A:0, B:1, C:2, D:3, E:4, F:5, G:6);
comp_tup!(8, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

/// Trait for handling the ingress of a single data-tuple
pub trait ComponentIngress<'a>: ComponentTuple<'a> {
    fn ingest(self, ctx: &mut TransactionContext) -> EntityId;
}

macro_rules! comp_ingress {
    ($( $field_type:ident:$field_seq:tt ),*) => {
        impl<'a, $($field_type),*> ComponentIngress<'a> for ($($field_type),*,)
        where
            $($field_type: 'static + Component),*,
        {
            #[inline]
            fn ingest(self, ctx: &mut TransactionContext) -> EntityId {
                let ids = Self::get_ids();

                let entity_id = EntityId(ctx.id_counter.fetch_add(1, Ordering::AcqRel));

                let shard = Self::get_shard(&ids, ctx);

                shard.get_mut(&EntityId::get_unique_id()).expect("Missing EntityId").push(entity_id);
                $(shard.get_mut(&ids.$field_seq).expect("Missing component").push(self.$field_seq));*;

                entity_id
            }
        }
    };
}

comp_ingress!(A:0);
comp_ingress!(A:0, B:1);
comp_ingress!(A:0, B:1, C:2);
comp_ingress!(A:0, B:1, C:2, D:3);
comp_ingress!(A:0, B:1, C:2, D:3, E:4);
comp_ingress!(A:0, B:1, C:2, D:3, E:4, F:5);
comp_ingress!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
comp_ingress!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

pub mod dynamic {
    use super::*;

    pub trait BuildDynVec: Debug {
        fn build(&self) -> DynVec;
        fn clone_box(&self) -> Box<BuildDynVec>;
    }

    #[derive(Debug)]
    pub struct DynVecFactory<T>(pub PhantomData<T>)
    where
        T: 'static + Component;

    impl<T> BuildDynVec for DynVecFactory<T>
    where
        T: 'static + Component,
    {
        #[inline]
        fn build(&self) -> DynVec {
            DynVec::new(Vec::<T>::new())
        }

        #[inline]
        fn clone_box(&self) -> Box<BuildDynVec> {
            Box::new(DynVecFactory::<T>(PhantomData))
        }
    }

    impl Clone for Box<BuildDynVec> {
        fn clone(&self) -> Self {
            self.clone_box()
        }
    }

    pub trait AnyVec: Debug {
        unsafe fn get_ptr(&mut self) -> *mut ();
        fn push_json(&mut self, json: &str);
        fn clone_box(&self) -> Box<AnyVec>;
    }

    impl<T> AnyVec for Vec<T>
    where
        T: 'static + Component,
    {
        #[inline]
        unsafe fn get_ptr(&mut self) -> *mut () {
            self as *mut Vec<T> as *mut ()
        }

        #[inline]
        fn push_json(&mut self, json: &str) {
            self.push(serde_json::from_str(json).expect("Error deserializing component"));
        }

        #[inline]
        fn clone_box(&self) -> Box<AnyVec> {
            Box::new(Vec::<T>::new())
        }
    }

    #[derive(Debug)]
    pub struct DynVec {
        inst: Box<AnyVec>,
        ptr: *mut (),
    }

    impl Clone for DynVec {
        fn clone(&self) -> Self {
            unsafe {
                let mut inst = self.inst.clone_box();
                let ptr = inst.get_ptr();
                DynVec { inst, ptr }
            }
        }
    }

    impl DynVec {
        pub fn new<T>(instance: Vec<T>) -> DynVec
        where
            T: 'static + Component,
        {
            unsafe {
                let mut inst: Box<AnyVec> = Box::new(instance);
                let ptr = inst.get_ptr();

                DynVec { inst, ptr }
            }
        }

        #[inline]
        pub fn push<T>(&mut self, item: T)
        where
            T: Component,
        {
            unsafe {
                self.cast_mut_unchecked::<T>().push(item);
            }
        }

        #[inline]
        pub fn push_json(&mut self, json: &str) {
            self.inst.push_json(json);
        }

        #[inline]
        pub fn cast<T>(&self) -> &Vec<T>
        where
            T: Component,
        {
            unsafe { &*(self.ptr as *const Vec<T>) }
        }

        #[inline]
        pub fn cast_mut<T>(&mut self) -> &mut Vec<T>
        where
            T: Component,
        {
            unsafe { &mut *(self.ptr as *mut Vec<T>) }
        }

        #[inline]
        pub unsafe fn cast_mut_unchecked<T>(&self) -> &mut Vec<T>
        where
            T: Component,
        {
            &mut *(self.ptr as *mut Vec<T>)
        }
    }
}

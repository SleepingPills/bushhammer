use crate::alloc::{DynVec, DynVecOps, CloneBox};
use crate::component::Component;
use crate::identity::{ComponentId, ShardKey};
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
}

/// Shard definition for accumulating components for new entities.
#[derive(Debug, Clone)]
pub struct ShardDef {
    pub(crate) entity_ids: Vec<EntityId>,
    pub(crate) components: HashMap<ComponentId, CompDefVec>,
}

impl ShardDef {
    #[inline]
    fn new(comp_ids: &[ComponentId], builders: &Vec<Box<BuildCompDef>>) -> ShardDef {
        let map: HashMap<_, _> = comp_ids.iter().map(|id| (*id, builders[id.indexer()].build())).collect();
        ShardDef {
            entity_ids: Vec::new(),
            components: map,
        }
    }

    #[inline]
    fn get_mut_vec(&mut self, comp_id: &ComponentId) -> &mut CompDefVec {
        self.components.get_mut(comp_id).unwrap()
    }
}

/// Context for recording entity transactions. Prepared by the `World` after all components have been
/// registered and the world is finalized.
#[derive(Debug, Clone)]
pub struct TransactionContext {
    pub(crate) added: HashMap<ShardKey, ShardDef>,
    pub(crate) deleted: Vec<EntityId>,
    pub(crate) builders: Vec<Box<BuildCompDef>>,
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
        let shard_key = ShardKey::from_iter(comp_ids.iter());

        let builders = &self.builders;
        let shard = self
            .added
            .entry(shard_key)
            .or_insert_with(|| ShardDef::new(comp_ids, builders));

        JsonBatchBuilder {
            comp_ids,
            shard,
            id_counter: self.id_counter.clone(),
            batch_counter: 0,
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
    pub fn remove(&mut self, id: EntityId) {
        self.deleted.push(id);
    }

    /// Add a vector builder for the given component type. This is required to be able to
    /// create shards for collecting 'weakly' typed input like json strings.
    pub(crate) fn add_builder<T>(&mut self)
    where
        T: 'static + Component,
    {
        self.builders.push(Box::new(CompDefBuilder::<T>(PhantomData)));
    }
}

pub struct JsonBatchBuilder<'a> {
    comp_ids: &'a [ComponentId],
    shard: &'a mut ShardDef,
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
            self.shard.get_mut_vec(id).push_json(json);
        }

        self.batch_counter += 1;
    }
    pub fn commit(&mut self) -> &[EntityId] {
        // Bump the id counter by the number of recorded entries in the batch
        let start_id = self.id_counter.fetch_add(self.batch_counter, Ordering::AcqRel);

        let new_slice_start = self.shard.entity_ids.len();

        // Generate entity Ids
        for id in start_id..(start_id + self.batch_counter) {
            self.shard.entity_ids.push(EntityId(id));
        }

        // Reset the batch counter
        self.batch_counter = 0;

        unsafe { self.shard.entity_ids.get_unchecked(new_slice_start..) }
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
                        $(shard.components[&ids.$field_seq].cast_mut_unchecked::<$field_type>()),*,
                    );

                    BatchBuilder::new(tup, &mut shard.entity_ids, id_counter)
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

    pub fn commit(&mut self) -> &[EntityId] {
        // Bump the id counter by the number of recorded entries in the batch
        let start_id = self.id_counter.fetch_add(self.batch_counter, Ordering::AcqRel);

        let new_slice_start = self.entity_vec.len();

        // Generate entity Ids
        for id in start_id..(start_id + self.batch_counter) {
            self.entity_vec.push(EntityId(id));
        }

        // Reset the batch counter
        self.batch_counter = 0;

        unsafe { self.entity_vec.get_unchecked(new_slice_start..) }
    }
}

impl<'a, T> Drop for BatchBuilder<'a, T> {
    fn drop(&mut self) {
        if self.batch_counter > 0 {
            self.commit();
        }
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
    fn get_shard(ids: &Self::IdTuple, ctx: &'a mut TransactionContext) -> &'a mut ShardDef;
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

            fn get_shard(ids: &Self::IdTuple, ctx: &'a mut TransactionContext) -> &'a mut ShardDef {
                let shard_key: ShardKey = ($(ids.$field_seq)|*).into();

                // Ensure that all types are distinct and no duplicate mutable entries are returned.
                // +1 is added to account for the Entity Id.
                if shard_key.count() != $field_count {
                    panic!("Invalid shard key rank")
                }

                // Get a cached shard builder or create a new one if necessary
                ctx.added.entry(shard_key).or_insert_with(|| {
                    let mut map = HashMap::new();
                    $(map.insert(ids.$field_seq, DynVec::new(Vec::<$field_type>::new())));*;
                    ShardDef {entity_ids: Vec::new(), components: map}
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

                shard.entity_ids.push(entity_id);

                $(shard.get_mut_vec(&ids.$field_seq).push(self.$field_seq));*;

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

pub trait CompDef: DynVecOps + Debug {
    fn push_json(&mut self, json: &str);
    fn clone_box(&self) -> Box<CompDef>;
}

impl<T> CompDef for Vec<T>
    where
        T: 'static + Component,
{
    #[inline]
    fn push_json(&mut self, json: &str) {
        self.push(serde_json::from_str(json).expect("Error deserializing component"));
    }

    #[inline]
    fn clone_box(&self) -> Box<CompDef> {
        Box::new(Vec::<T>::new())
    }
}

pub type CompDefVec = DynVec<CompDef>;

impl CompDefVec {
    #[inline]
    pub fn push<T>(&mut self, item: T)
        where
            T: 'static + Component,
    {
        self.cast_mut_vector::<T>().push(item);
    }

    #[inline]
    pub unsafe fn cast_mut_unchecked<T>(&self) -> &mut Vec<T>
        where
            T: 'static + Component,
    {
        &mut *(self.get_inner_ptr().cast_checked_raw())
    }
}

impl Clone for CompDefVec {
    fn clone(&self) -> Self {
        DynVec::from_box(self.clone_box())
    }
}

pub trait BuildCompDef: Debug {
    fn build(&self) -> CompDefVec;
    fn clone_box(&self) -> Box<BuildCompDef>;
}

#[derive(Debug)]
pub struct CompDefBuilder<T>(pub PhantomData<T>)
where
    T: 'static + Component;

impl<T> BuildCompDef for CompDefBuilder<T>
where
    T: 'static + Component,
{
    #[inline]
    fn build(&self) -> CompDefVec {
        DynVec::new(Vec::<T>::new())
    }

    #[inline]
    fn clone_box(&self) -> Box<BuildCompDef> {
        Box::new(CompDefBuilder::<T>(PhantomData))
    }
}

impl Clone for Box<BuildCompDef> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

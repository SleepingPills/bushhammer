use crate::component::{key_count, Component, ComponentCoords, ShardKey};
use crate::identity::{ComponentId, EntityId, ShardId};
use hashbrown::HashMap;
use serde_json;
use std::any::TypeId;
use std::fmt::Debug;

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
pub struct TransactionContext {
    added: HashMap<ShardKey, HashMap<ComponentId, dynamic::DynVec>>,
    deleted: Vec<EntityId>,
    component_ids: HashMap<TypeId, ComponentId>,
}

impl TransactionContext {
    #[inline]
    pub fn batch<'a, T>(&'a mut self) -> T::Batcher
    where
        T: BatchDef<'a>,
    {
        T::new_batch(self)
    }

    #[inline]
    pub fn add<'a, T>(&'a mut self, tuple: T)
    where
        T: ComponentIngress<'a>,
    {
        tuple.ingest(self);
    }

    #[inline]
    pub fn delete(&mut self, id: EntityId) {
        self.deleted.push(id);
    }
}

pub trait BatchDef<'a>: ComponentTuple<'a> {
    type Batcher;

    fn new_batch(ctx: &'a mut TransactionContext) -> Self::Batcher;
}

macro_rules! batch_def_tup {
    ($tup_name:ident, $( $field_type:ident:$field_seq:tt ),*) => {
        impl<'a, $($field_type),*> BatchDef<'a> for ($($field_type),*,)
        where
            $($field_type: 'static + Component),*,
        {
            type Batcher = $tup_name<'a, $($field_type),*>;

            #[inline]
            fn new_batch(ctx: &'a mut TransactionContext) -> Self::Batcher {
                let ids = Self::get_ids(ctx);
                let shard = Self::get_shard(&ids, ctx);

                // The below is safe because of previous checks
                unsafe {
                    $tup_name(
                        $(shard[&ids.$field_seq].cast_mut_unchecked::<$field_type>()),*,
                    )
                }
            }
        }
    };
}

batch_def_tup!(B1, A:0);
batch_def_tup!(B2, A:0, B:1);
batch_def_tup!(B3, A:0, B:1, C:2);
batch_def_tup!(B4, A:0, B:1, C:2, D:3);
batch_def_tup!(B5, A:0, B:1, C:2, D:3, E:4);
batch_def_tup!(B6, A:0, B:1, C:2, D:3, E:4, F:5);
batch_def_tup!(B7, A:0, B:1, C:2, D:3, E:4, F:5, G:6);
batch_def_tup!(B8, A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

macro_rules! batch_tup {
    ($tup_name:ident, $( $field_type:ident:$field_name:ident:$field_seq:tt ),*) => {
        pub struct $tup_name<'a, $($field_type),*>($(&'a mut Vec<$field_type>),*,)
        where
            $($field_type: Component),*;

        impl<'a, $($field_type),*> $tup_name<'a, $($field_type),*>
        where
            $($field_type: Component),*
        {
            #[inline]
            pub fn add(&mut self, $($field_name: $field_type),*) {
                $(self.$field_seq.push($field_name));*;
            }

            #[inline]
            pub fn add_json(&mut self, $($field_name: &str),*) {
                $(self.$field_seq.push(serde_json::from_str($field_name).expect("Error deserializing component")));*;
            }
        }
    };
}

batch_tup!(B1, A:a:0);
batch_tup!(B2, A:a:0, B:b:1);
batch_tup!(B3, A:a:0, B:b:1, C:c:2);
batch_tup!(B4, A:a:0, B:b:1, C:c:2, D:d:3);
batch_tup!(B5, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4);
batch_tup!(B6, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4, F:f:5);
batch_tup!(B7, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4, F:f:5, G:g:6);
batch_tup!(B8, A:a:0, B:b:1, C:c:2, D:d:3, E:e:4, F:f:5, G:g:6, H:h:7);

pub trait ComponentTuple<'a> {
    type IdTuple;

    fn get_ids(ctx: &TransactionContext) -> Self::IdTuple;
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
            fn get_ids(ctx: &TransactionContext) -> Self::IdTuple {
                (
                    $(ctx.component_ids[&TypeId::of::<$field_type>()]),*,
                )
            }

            fn get_shard(ids: &Self::IdTuple, ctx: &'a mut TransactionContext) -> &'a mut HashMap<ComponentId, dynamic::DynVec> {
                let shard_key = $(ids.$field_seq.id)|*;

                // Ensure that all types are distinct and no duplicate mutable entries are returned.
                if key_count(shard_key) != $field_count {
                    panic!("Invalid shard key rank")
                }

                // Get a cached shard builder or create a new one if necessary
                ctx.added.entry(shard_key).or_insert_with(|| {
                    let mut map = HashMap::new();
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

pub trait ComponentIngress<'a>: ComponentTuple<'a> {
    fn ingest(self, ctx: &mut TransactionContext);
}

macro_rules! comp_ingress {
    ($( $field_type:ident:$field_seq:tt ),*) => {
        impl<'a, $($field_type),*> ComponentIngress<'a> for ($($field_type),*,)
        where
            $($field_type: 'static + Component),*,
        {
            #[inline]
            fn ingest(self, ctx: &mut TransactionContext) {
                let ids = Self::get_ids(ctx);
                let shard = Self::get_shard(&ids, ctx);

                $(shard.get_mut(&ids.$field_seq).expect("Missing component").push(self.$field_seq));*;
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

mod dynamic {
    use super::*;

    pub trait AnyVec: Debug {
        unsafe fn get_ptr(&mut self) -> *mut ();
        fn push_json(&mut self, json: &str);
    }

    impl<T> AnyVec for Vec<T>
    where
        T: Component,
    {
        #[inline]
        unsafe fn get_ptr(&mut self) -> *mut () {
            self as *mut Vec<T> as *mut ()
        }

        #[inline]
        fn push_json(&mut self, json: &str) {
            self.push(serde_json::from_str(json).expect("Error deserializing component"));
        }
    }

    #[derive(Debug)]
    pub struct DynVec {
        inst: Box<AnyVec>,
        ptr: *mut (),
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
        pub fn push<T>(&mut self, item: T) {
            unsafe {
                self.cast_mut_unchecked::<T>().push(item);
            }
        }

        #[inline]
        pub fn push_json(&mut self, json: &str) {
            self.inst.push_json(json);
        }

        #[inline]
        pub fn cast<T>(&self) -> &Vec<T> {
            unsafe { &*(self.ptr as *const Vec<T>) }
        }

        #[inline]
        pub unsafe fn cast_mut_unchecked<T>(&self) -> &mut Vec<T> {
            &mut *(self.ptr as *mut Vec<T>)
        }
    }
}

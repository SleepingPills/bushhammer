use crate::component;
use crate::component::{ComponentCoords, ShardedColumn};
use crate::entity::{Entity, EntityStore, Transaction};
use crate::identity::{ComponentId, EntityId, ShardId};
use crate::registry::Registry;
use crate::sync::RwCell;
use hashbrown::HashMap;
use indexmap::IndexMap;
use std::any::TypeId;
use std::sync::Arc;

#[macro_export]
macro_rules! require {
    ($($exprs:ty),*) => {
        type Data = ($($exprs),*);
        type JoinItem = <Self::Data as SystemDef>::JoinItem;
    };
}

pub trait System {
    type Data: SystemDef;
    type JoinItem: Joined;

    fn run(&mut self, ctx: context::Context<<Self::Data as SystemDef>::JoinItem>, entities: EntityStore);
}

pub trait SystemRuntime {
    fn run(&mut self, entity_map: &HashMap<EntityId, Entity>, comp_id_map: &HashMap<TypeId, ComponentId>);
    fn add_shard(&mut self, shard: &component::Shard);
    fn remove_shard(&mut self, id: ShardId);
    fn check_shard(&self, shard_key: component::ShardKey) -> bool;
    fn get_required_components(&self) -> &Vec<ComponentId>;
    fn get_transactions(&mut self) -> &mut Vec<Transaction>;
}

pub struct SystemData<T>
where
    T: SystemDef,
{
    stores: T,
    shards: IndexMap<ShardId, <T::JoinItem as Joined>::Indexer>,
    comp_ids: Vec<ComponentId>,
}

impl<T> SystemData<T>
where
    T: SystemDef,
{
    #[inline]
    pub(crate) fn new(comp_map: &Registry<ComponentId>, comp_ids: Vec<ComponentId>) -> SystemData<T> {
        SystemData {
            stores: T::new(&comp_ids, comp_map),
            shards: IndexMap::new(),
            comp_ids,
        }
    }

    #[inline]
    pub fn context<'a, 'b>(&'a self, entity_map: &'b HashMap<EntityId, Entity>) -> context::Context<<T as SystemDef>::JoinItem>
    where
        'b: 'a,
    {
        context::Context::new(self.stores.as_joined(), &self.shards, entity_map, &self.comp_ids)
    }

    #[inline]
    fn add_shard(&mut self, shard: &component::Shard) {
        self.shards.insert(shard.id, T::reify_shard(&self.comp_ids, shard));
    }

    #[inline]
    fn remove_shard(&mut self, id: ShardId) {
        self.shards.remove(&id);
    }
}

pub struct SystemEntry<T>
where
    T: System,
{
    system: T,
    shard_key: component::ShardKey,
    data: SystemData<T::Data>,
    transactions: Vec<Transaction>,
}

impl<T> SystemEntry<T>
where
    T: System,
{
    #[inline]
    pub(crate) fn new(system: T, comp_map: &Registry<ComponentId>, comp_id_map: &HashMap<TypeId, ComponentId>) -> SystemEntry<T> {
        let comp_ids = T::Data::get_comp_ids(comp_id_map);
        SystemEntry {
            system,
            shard_key: component::compose_key(comp_ids.iter()),
            data: SystemData::new(comp_map, comp_ids),
            transactions: Vec::new(),
        }
    }

    #[inline]
    pub fn get_system_mut(&mut self) -> &mut T {
        &mut self.system
    }
}

impl<T> SystemRuntime for SystemEntry<T>
where
    T: System,
{
    #[inline]
    fn run(&mut self, entity_map: &HashMap<EntityId, Entity>, comp_id_map: &HashMap<TypeId, ComponentId>) {
        self.system.run(
            self.data.context(entity_map),
            EntityStore::new(entity_map, comp_id_map, &mut self.transactions),
        );
    }

    #[inline]
    fn add_shard(&mut self, shard: &component::Shard) {
        self.data.add_shard(shard);
    }

    #[inline]
    fn remove_shard(&mut self, id: ShardId) {
        self.data.remove_shard(id);
    }

    #[inline]
    fn check_shard(&self, shard_key: component::ShardKey) -> bool {
        (self.shard_key & shard_key) == self.shard_key
    }

    #[inline]
    fn get_required_components(&self) -> &Vec<ComponentId> {
        &self.data.comp_ids
    }

    #[inline]
    fn get_transactions(&mut self) -> &mut Vec<Transaction> {
        &mut self.transactions
    }
}

pub trait Indexable {
    type Item;

    fn index(&self, idx: usize) -> Self::Item;
}

pub trait Query {
    type DataPtr: Indexable;
    type Item;

    fn len(&self, section: usize) -> usize;
    fn get_by_coords(&self, coords: ComponentCoords) -> Self::Item;
    fn unwrap(&mut self, section: usize) -> Self::DataPtr;
    fn null() -> Self::DataPtr;
}

pub trait Queryable {
    type QueryItem: Query;
    type DataType;

    fn new(store: Arc<RwCell<ShardedColumn<Self::DataType>>>) -> Self;
    fn get_query(&self) -> Self::QueryItem;
}

pub trait IndexablePtrTup {
    type ItemTup;

    fn index(&self, idx: usize) -> Self::ItemTup;
}

pub mod store {
    use super::{Arc, ComponentCoords, Indexable, Query, Queryable, RwCell, ShardedColumn};
    use std::marker::PhantomData;
    use std::ptr;

    #[repr(transparent)]
    pub struct ReadPtr<'a, T>(*const T, PhantomData<&'a ()>);

    impl<'a, T> ReadPtr<'a, T> {
        #[inline]
        fn new(ptr: *const T) -> ReadPtr<'a, T> {
            ReadPtr(ptr, PhantomData)
        }
    }

    #[repr(transparent)]
    pub struct RwPtr<'a, T>(*mut T, PhantomData<&'a ()>);

    impl<'a, T> RwPtr<'a, T> {
        #[inline]
        fn new(ptr: *mut T) -> RwPtr<'a, T> {
            RwPtr(ptr, PhantomData)
        }
    }

    impl<'a, T: 'a> Indexable for ReadPtr<'a, T> {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            unsafe { &*self.0.add(idx) }
        }
    }

    impl<'a, T: 'a> Indexable for RwPtr<'a, T> {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            unsafe { &mut *self.0.add(idx) }
        }
    }

    #[repr(transparent)]
    pub struct ReadQuery<'a, T> {
        store: *const ShardedColumn<T>,
        _x: PhantomData<&'a T>,
    }

    #[repr(transparent)]
    pub struct WriteQuery<'a, T> {
        store: *mut ShardedColumn<T>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> ReadQuery<'a, T> {
        #[inline]
        fn new(store: &RwCell<ShardedColumn<T>>) -> ReadQuery<'a, T> {
            // No need for explicit guards as the scheduler guarantees to maintain the reference aliasing invariants.
            unsafe {
                ReadQuery {
                    store: store.get_ptr_raw(),
                    _x: PhantomData,
                }
            }
        }

        #[inline]
        fn store_ref(&self) -> &ShardedColumn<T> {
            unsafe { &*self.store }
        }
    }

    impl<'a, T> WriteQuery<'a, T> {
        #[inline]
        fn new(store: &RwCell<ShardedColumn<T>>) -> WriteQuery<'a, T> {
            // No need for explicit guards as the scheduler guarantees to maintain the reference aliasing invariants.
            unsafe {
                WriteQuery {
                    store: store.get_ptr_raw(),
                    _x: PhantomData,
                }
            }
        }

        #[inline]
        fn store_ref(&self) -> &ShardedColumn<T> {
            unsafe { &*self.store }
        }

        #[inline]
        fn store_mut_ref(&mut self) -> &mut ShardedColumn<T> {
            unsafe { &mut *self.store }
        }
    }

    impl<'a, T: 'a> Query for ReadQuery<'a, T> {
        type DataPtr = ReadPtr<'a, T>;
        type Item = &'a T;

        #[inline]
        fn len(&self, section: usize) -> usize {
            self.store_ref().section_len(section)
        }

        #[inline]
        fn get_by_coords(&self, coords: ComponentCoords) -> &'a T {
            let (section, loc) = coords;
            let ptr = self.store_ref().get_data_ptr(section);
            unsafe { &*ptr.add(loc) }
            // Can't do the safe thing here due to lack of generic associated types as the
            // lifetimes would clash.
            // self.store.get_item(coords)
        }

        #[inline]
        fn unwrap(&mut self, section: usize) -> ReadPtr<'a, T> {
            ReadPtr::new(self.store_ref().get_data_ptr(section))
        }

        #[inline]
        fn null() -> ReadPtr<'a, T> {
            ReadPtr::new(ptr::null())
        }
    }

    impl<'a, T: 'a> Query for WriteQuery<'a, T> {
        type DataPtr = RwPtr<'a, T>;
        type Item = &'a mut T;

        #[inline]
        fn len(&self, section: usize) -> usize {
            self.store_ref().section_len(section)
        }

        #[inline]
        fn get_by_coords(&self, coords: ComponentCoords) -> &'a mut T {
            let (section, loc) = coords;
            let ptr = self.store_ref().get_data_ptr(section);
            unsafe { &mut *(ptr.add(loc) as *mut _) }
            // Can't do the safe thing here due to lack of generic associated types as the
            // lifetimes would clash.
            // self.store.get_item_mut(coords)
        }

        #[inline]
        fn unwrap(&mut self, section: usize) -> RwPtr<'a, T> {
            RwPtr::new(self.store_mut_ref().get_data_mut_ptr(section))
        }

        #[inline]
        fn null() -> RwPtr<'a, T> {
            RwPtr::new(ptr::null_mut())
        }
    }

    pub struct Read<'a, T> {
        store: Arc<RwCell<ShardedColumn<T>>>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> Queryable for Read<'a, T> {
        type QueryItem = ReadQuery<'a, T>;
        type DataType = T;

        #[inline]
        fn new(store: Arc<RwCell<ShardedColumn<T>>>) -> Self {
            Read { store, _x: PhantomData }
        }

        #[inline]
        fn get_query(&self) -> ReadQuery<'a, T> {
            ReadQuery::new(&self.store)
        }
    }

    pub struct Write<'a, T> {
        store: Arc<RwCell<ShardedColumn<T>>>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> Queryable for Write<'a, T> {
        type QueryItem = WriteQuery<'a, T>;
        type DataType = T;

        #[inline]
        fn new(store: Arc<RwCell<ShardedColumn<T>>>) -> Self {
            Write { store, _x: PhantomData }
        }

        #[inline]
        fn get_query(&self) -> WriteQuery<'a, T> {
            WriteQuery::new(&self.store)
        }
    }
}

pub trait Joined {
    type PtrTup: IndexablePtrTup;
    type ItemTup;
    type Indexer;

    fn get_ptr_tup(&mut self, idx: &Self::Indexer) -> (usize, Self::PtrTup);
    fn get_entity(&self, entity: &Entity, components: &Vec<ComponentId>) -> Self::ItemTup;
    unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
}

pub trait SystemDef {
    type JoinItem: Joined;

    fn as_joined(&self) -> Self::JoinItem;
    fn reify_shard(sys_comps: &Vec<ComponentId>, shard: &component::Shard) -> <Self::JoinItem as Joined>::Indexer;
    fn get_comp_ids(comp_ids: &HashMap<TypeId, ComponentId>) -> Vec<ComponentId>;
    fn new(comp_ids: &[ComponentId], comp_map: &Registry<ComponentId>) -> Self;
}

pub mod join {
    use super::{
        ComponentId, Entity, HashMap, Indexable, IndexablePtrTup, Joined, Query, Queryable, Registry, ShardedColumn, SystemDef,
        TypeId,
    };
    use crate::component;

    macro_rules! _decl_system_replace_expr {
        ($_t:tt $sub:ty) => {
            $sub
        };
    }

    macro_rules! ptr_tup {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> IndexablePtrTup for ($($field_type),*,)
            where
                $($field_type: Indexable),*
            {
                type ItemTup = ($($field_type::Item),*);

                #[inline]
                fn index(&self, idx: usize) -> ($($field_type::Item),*) {
                    ($(self.$field_seq.index(idx)),*)
                }
            }
        };
    }

    ptr_tup!(A:0);
    ptr_tup!(A:0, B:1);
    ptr_tup!(A:0, B:1, C:2);
    ptr_tup!(A:0, B:1, C:2, D:3);
    ptr_tup!(A:0, B:1, C:2, D:3, E:4);
    ptr_tup!(A:0, B:1, C:2, D:3, E:4, F:5);
    ptr_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    ptr_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

    macro_rules! joined {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> Joined for ($($field_type),*,)
            where
                $($field_type: Query),*,
            {
                type PtrTup = ($($field_type::DataPtr),*,);
                type ItemTup = ($($field_type::Item),*,);
                type Indexer = ($(_decl_system_replace_expr!($field_type usize)),*,);

                #[inline]
                fn get_ptr_tup(&mut self, idx: &Self::Indexer) -> (usize, Self::PtrTup) {
                    (self.0.len(idx.0), ($(self.$field_seq.unwrap(idx.$field_seq)),*,))
                }

                #[inline]
                fn get_entity(&self, entity: &Entity, components: &Vec<ComponentId>) -> Self::ItemTup {
                    ($(self.$field_seq.get_by_coords(entity.get_coords(&components[$field_seq]))),*,)
                }

                #[inline]
                unsafe fn get_zero_ptr_tup() -> Self::PtrTup {
                    ($($field_type::null()),*,)
                }
            }
        };
    }

    joined!(A:0);
    joined!(A:0, B:1);
    joined!(A:0, B:1, C:2);
    joined!(A:0, B:1, C:2, D:3);
    joined!(A:0, B:1, C:2, D:3, E:4);
    joined!(A:0, B:1, C:2, D:3, E:4, F:5);
    joined!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    joined!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

    macro_rules! system_def {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> SystemDef for ($($field_type),*,)
            where
                $($field_type: Queryable),*,
                $($field_type::DataType: 'static),*
            {
                type JoinItem = ($($field_type::QueryItem),*,);

                #[inline]
                fn as_joined(&self) -> ($($field_type::QueryItem),*,) {
                    ($(self.$field_seq.get_query()),*,)
                }

                #[inline]
                fn reify_shard(sys_comps: &Vec<ComponentId>,
                                shard: &component::Shard) -> <Self::JoinItem as Joined>::Indexer {
                    ($(shard.get_section(sys_comps[$field_seq])),*,)
                }

                #[inline]
                fn get_comp_ids(comp_ids: &HashMap<TypeId, ComponentId>) -> Vec<ComponentId> {
                    vec![$(comp_ids[&TypeId::of::<$field_type::DataType>()]),*]
                }

                #[inline]
                fn new(comp_ids: &[ComponentId], comp_map: &Registry<ComponentId>) -> Self {
                    ($($field_type::new(comp_map.get::<ShardedColumn<$field_type::DataType>>(&comp_ids[$field_seq]))),*,)
                }
            }
        };
    }

    system_def!(A:0);
    system_def!(A:0, B:1);
    system_def!(A:0, B:1, C:2);
    system_def!(A:0, B:1, C:2, D:3);
    system_def!(A:0, B:1, C:2, D:3, E:4);
    system_def!(A:0, B:1, C:2, D:3, E:4, F:5);
    system_def!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    system_def!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
}

pub mod context {
    use super::{ComponentId, Entity, EntityId, HashMap, IndexMap, IndexablePtrTup, Joined, ShardId};
    use indexmap::map::Values;

    pub struct Context<'a, T>
    where
        T: Joined,
    {
        stores: T,
        shards: &'a IndexMap<ShardId, T::Indexer>,
        entity_map: &'a HashMap<EntityId, Entity>,
        components: &'a Vec<ComponentId>,
    }

    impl<'a, T> Context<'a, T>
    where
        T: Joined,
    {
        #[inline]
        pub fn new(
            stores: T,
            shards: &'a IndexMap<ShardId, T::Indexer>,
            entity_map: &'a HashMap<EntityId, Entity>,
            components: &'a Vec<ComponentId>,
        ) -> Context<'a, T> {
            Context {
                stores,
                shards,
                entity_map,
                components,
            }
        }

        #[inline]
        pub fn for_each<F>(&mut self, entities: &[EntityId], f: F)
        where
            F: FnMut(T::ItemTup),
        {
            entities
                .iter()
                .filter_map(move |eid| {
                    if let Some(entity) = self.entity_map.get(eid) {
                        Some(self.stores.get_entity(&entity, self.components))
                    } else {
                        None
                    }
                })
                .for_each(f);
        }

        #[inline]
        pub fn iter(&mut self) -> ComponentIterator<T> {
            let mut stream = self.shards.values();

            unsafe {
                let (size, shard) = match stream.next() {
                    Some(item) => self.stores.get_ptr_tup(item),
                    _ => (0usize, T::get_zero_ptr_tup()),
                };

                ComponentIterator {
                    stores: &mut self.stores,
                    stream,
                    shard,
                    size,
                    counter: 0,
                }
            }
        }
    }

    pub struct ComponentIterator<'a, T>
    where
        T: Joined,
    {
        stores: &'a mut T,
        stream: Values<'a, ShardId, T::Indexer>,
        shard: T::PtrTup,
        size: usize,
        counter: usize,
    }

    impl<'a, T> Iterator for ComponentIterator<'a, T>
    where
        T: Joined,
    {
        type Item = <T::PtrTup as IndexablePtrTup>::ItemTup;

        #[inline]
        fn next(&mut self) -> Option<<T::PtrTup as IndexablePtrTup>::ItemTup> {
            loop {
                if self.counter < self.size {
                    let idx = self.counter;
                    self.counter += 1;
                    return Some(self.shard.index(idx));
                }

                if let Some(item) = self.stream.next() {
                    let (size, shard) = self.stores.get_ptr_tup(item);
                    self.shard = shard;
                    self.size = size;
                    self.counter = 0;
                } else {
                    return None;
                }
            }
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use crate::entity;
    use crate::prelude::*;
    use std::marker::PhantomData;

    #[test]
    fn test_for_each() {
        struct TestSystem<'a> {
            collector: Vec<(EntityId, i32, f32)>,
            _p: PhantomData<&'a ()>,
        }

        impl<'a> System for TestSystem<'a> {
            require!(Read<'a, EntityId>, Read<'a, i32>, Write<'a, f32>);

            fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: entity::EntityStore) {
                let entity_ids: Vec<_> = (0..4).map(|id| id.into()).collect();
                ctx.for_each(&entity_ids, |(id, a, b)| {
                    self.collector.push((*id, *a, *b));
                });
            }
        }

        let mut world = World::new();

        world.register_component::<i32>();
        world.register_component::<f32>();
        world.register_component::<f64>();

        let system_id = world.register_system(TestSystem {
            collector: Vec::new(),
            _p: PhantomData,
        });
        let system = world.get_system::<TestSystem>(system_id);

        world.entities().create().with(0i32).with(0f32).build();
        world.entities().create().with(1i32).with(1f32).build();
        world.entities().create().with(2i32).with(2f32).build();
        world.entities().create().with(3i32).with(3f32).with(5f64).build();

        world.run();

        let state: Vec<_> = system.write().get_system_mut().collector.drain(..).collect();

        assert_eq!(state.len(), 4);
        assert_eq!(state[0], (0.into(), 0, 0f32));
        assert_eq!(state[1], (1.into(), 1, 1f32));
        assert_eq!(state[2], (2.into(), 2, 2f32));
        assert_eq!(state[3], (3.into(), 3, 3f32));
    }
}
*/

use crate::component::Component;
use crate::component::{ComponentCoords, Shard};
use crate::entity::{EntityId, TransactionContext};
use crate::identity::ShardKey;
use crate::messagebus::{Batcher, Bus, Message};
use crate::sentinel::Take;
use anymap::AnyMap;
use hashbrown::HashMap;
use indexmap::IndexMap;
use std::marker::PhantomData;
use std::time;

// TODO: Add optional components. These will return Option<Component> and allow intersection queries.
//       To implement, the data_ptr() on a shard needs to return an Option, and then current queries
//       will unwrap it, but a special OptionalReadQuery will unwrap into either a regular reader or
//       None returning reader, depending on the presence of the component in a shard.

pub trait RunSystem {
    type Data: DataDef;

    fn run(&mut self, ctx: Context<Self::Data>, tx: &mut TransactionContext, msg: Router);
    fn init(&mut self) {}
}

pub trait DataDef {
    type Components: ComponentQueryTup;
    type Resources: ResourceQueryTup;
}

pub struct Components<T>(PhantomData<T>);
pub struct Resources<T>(PhantomData<T>);
pub struct Combo<A, B>(PhantomData<A>, PhantomData<B>);

impl DataDef for () {
    type Components = ();
    type Resources = ();
}

impl<T> DataDef for Components<T>
where
    T: ComponentQueryTup,
{
    type Components = T;
    type Resources = ();
}

impl<T> DataDef for Resources<T>
where
    T: ResourceQueryTup,
{
    type Components = ();
    type Resources = T;
}

impl<A, B> DataDef for Combo<A, B>
where
    A: ComponentQueryTup,
    B: ResourceQueryTup,
{
    type Components = A;
    type Resources = B;
}

pub struct Context<'a, T>
where
    T: DataDef,
{
    system_data: &'a mut SystemData<T>,
    entities: &'a HashMap<EntityId, ComponentCoords>,
    pub delta: f32,
    pub timestamp: time::Instant,
}

impl<'a, T> Context<'a, T>
where
    T: DataDef,
{
    #[inline]
    pub fn components(&mut self) -> context::ComponentContext<<T::Components as ComponentQueryTup>::DataTup> {
        self.system_data.components(self.entities)
    }

    #[inline]
    pub fn resources(&mut self) -> <<T::Resources as ResourceQueryTup>::DataTup as ResourceDataTup>::ItemTup {
        self.system_data.resources()
    }
}

pub struct SystemData<T>
where
    T: DataDef,
{
    shards: IndexMap<ShardKey, <T::Components as ComponentQueryTup>::DataTup>,
    resource_tup: Take<<T::Resources as ResourceQueryTup>::DataTup>,
}

impl<T> SystemData<T>
where
    T: DataDef,
{
    #[inline]
    fn new() -> SystemData<T> {
        SystemData {
            shards: IndexMap::new(),
            resource_tup: Take::empty(),
        }
    }

    #[inline]
    pub fn components<'a>(
        &'a mut self,
        entities: &'a HashMap<EntityId, ComponentCoords>,
    ) -> context::ComponentContext<<T::Components as ComponentQueryTup>::DataTup> {
        context::ComponentContext::new(&mut self.shards, entities)
    }

    #[inline]
    pub fn resources(&mut self) -> <<T::Resources as ResourceQueryTup>::DataTup as ResourceDataTup>::ItemTup {
        self.resource_tup.borrow()
    }

    #[inline]
    pub fn init_resources(&mut self, resources: &AnyMap) {
        self.resource_tup
            .put(<T::Resources as ResourceQueryTup>::reify(resources));
    }

    #[inline]
    pub(crate) fn add_shard(&mut self, shard: &Shard) {
        self.shards.insert(shard.key, T::Components::reify_shard(shard));
    }

    #[inline]
    pub(crate) fn remove_shard(&mut self, key: ShardKey) {
        self.shards.remove(&key);
    }
}

pub struct SystemRuntime<T>
where
    T: RunSystem,
{
    shard_key: ShardKey,
    runstate: T,
    data: SystemData<T::Data>,
    messages: Bus,
}

impl<T> SystemRuntime<T>
where
    T: RunSystem,
{
    #[inline]
    pub(crate) fn new(system: T) -> SystemRuntime<T> {
        SystemRuntime {
            shard_key: <<T::Data as DataDef>::Components as ComponentQueryTup>::get_shard_key(),
            runstate: system,
            data: SystemData::new(),
            messages: Bus::new(),
        }
    }

    #[inline]
    pub fn get_system_mut(&mut self) -> &mut T {
        &mut self.runstate
    }
}

pub trait System {
    fn run(
        &mut self,
        entities: &HashMap<EntityId, ComponentCoords>,
        transactions: &mut TransactionContext,
        incoming: &Bus,
        delta: f32,
        timestamp: time::Instant,
    );
    fn init(&mut self, resources: &AnyMap);
    fn transfer_messages(&mut self, central_bus: &mut Bus);
    fn add_shard(&mut self, shard: &Shard);
    fn remove_shard(&mut self, key: ShardKey);
    fn check_shard(&self, shard_key: ShardKey) -> bool;
}

impl<T> System for SystemRuntime<T>
where
    T: RunSystem,
{
    #[inline]
    fn run(
        &mut self,
        entities: &HashMap<EntityId, ComponentCoords>,
        transactions: &mut TransactionContext,
        incoming: &Bus,
        delta: f32,
        timestamp: time::Instant,
    ) {
        self.runstate.run(
            Context {
                system_data: &mut self.data,
                entities,
                delta,
                timestamp,
            },
            transactions,
            Router {
                incoming,
                outgoing: &mut self.messages,
            },
        );
    }

    #[inline]
    fn init(&mut self, resources: &AnyMap) {
        self.data.init_resources(resources);
        self.runstate.init();
    }

    fn transfer_messages(&mut self, central_bus: &mut Bus) {
        central_bus.transfer(&mut self.messages);
    }

    #[inline]
    fn add_shard(&mut self, shard: &Shard) {
        if self.check_shard(shard.key) {
            self.data.add_shard(shard);
        }
    }

    #[inline]
    fn remove_shard(&mut self, key: ShardKey) {
        if self.check_shard(key) {
            self.data.remove_shard(key);
        }
    }

    #[inline]
    fn check_shard(&self, shard_key: ShardKey) -> bool {
        shard_key.contains_key(self.shard_key)
    }
}

/// Routes messages to the correct bus.
pub struct Router<'a> {
    incoming: &'a Bus,
    outgoing: &'a mut Bus,
}

impl<'a> Router<'_> {
    /// Read the messages for a particular topic.
    #[inline]
    pub fn read<T>(&self) -> &[T]
    where
        T: 'static + Message,
    {
        self.incoming.read::<T>()
    }

    /// Publish the supplied message on the bus.
    #[inline]
    pub fn publish<T>(&mut self, message: T)
    where
        T: 'static + Message,
    {
        self.outgoing.publish(message);
    }

    /// Batch publish messages of a given type.
    #[inline]
    pub fn batch<T>(&mut self) -> Batcher<T>
    where
        T: 'static + Message,
    {
        self.outgoing.batch::<T>()
    }
}

pub struct Read<'a, T> {
    _x: PhantomData<&'a T>,
}

pub struct Write<'a, T> {
    _x: PhantomData<&'a T>,
}

pub trait IndexablePtrTup {
    type ItemTup;

    fn index(&self, idx: usize) -> Self::ItemTup;
}

pub trait ComponentDataTup {
    type PtrTup: IndexablePtrTup;
    type ItemTup;

    fn get_entity(&mut self, loc: usize) -> Self::ItemTup;

    fn get_ptr_tup(&mut self) -> (usize, Self::PtrTup);
    unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
}

pub trait ComponentQueryTup {
    type DataTup: ComponentDataTup;

    fn reify_shard(shard: &Shard) -> Self::DataTup;
    fn get_shard_key() -> ShardKey;
}

pub mod store {
    use super::{
        Component, ComponentDataTup, ComponentQueryTup, IndexablePtrTup, PhantomData, Read, Shard, ShardKey,
        Write,
    };
    use std::ptr;

    pub trait Indexable {
        type Item;

        fn index(&self, idx: usize) -> Self::Item;
    }

    pub trait Data {
        type DataPtr: Indexable;
        type Item;

        fn len(&self) -> usize;
        fn get(&mut self, loc: usize) -> Self::Item;
        fn unwrap(&mut self) -> Self::DataPtr;
        fn null() -> Self::DataPtr;
    }

    pub trait Query {
        type QueryItem: Data;
        type DataType;

        fn execute(shard: &Shard) -> Self::QueryItem;
    }

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
    pub struct ReadData<'a, T> {
        store: *const Vec<T>,
        _x: PhantomData<&'a T>,
    }

    #[repr(transparent)]
    pub struct WriteData<'a, T> {
        store: *mut Vec<T>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> ReadData<'a, T> {
        #[inline]
        fn new(store: *const Vec<T>) -> ReadData<'a, T> {
            ReadData {
                store,
                _x: PhantomData,
            }
        }

        #[inline]
        fn store_ref(&self) -> &'a Vec<T> {
            unsafe { &*self.store }
        }

        #[allow(dead_code)]
        #[inline]
        pub(crate) fn get_ptr(&self) -> *const Vec<T> {
            self.store
        }
    }

    impl<'a, T> WriteData<'a, T> {
        #[inline]
        fn new(store: *mut Vec<T>) -> WriteData<'a, T> {
            WriteData {
                store,
                _x: PhantomData,
            }
        }

        #[inline]
        fn store_ref(&self) -> &'a Vec<T> {
            unsafe { &*self.store }
        }

        #[inline]
        fn store_mut_ref(&mut self) -> &'a mut Vec<T> {
            unsafe { &mut *self.store }
        }
    }

    impl<'a, T: 'a> Data for ReadData<'a, T> {
        type DataPtr = ReadPtr<'a, T>;
        type Item = &'a T;

        #[inline]
        fn len(&self) -> usize {
            self.store_ref().len()
        }

        #[inline]
        fn get(&mut self, loc: usize) -> &'a T {
            unsafe { self.store_ref().get_unchecked(loc) }
        }

        #[inline]
        fn unwrap(&mut self) -> ReadPtr<'a, T> {
            ReadPtr::new(self.store_ref().as_ptr())
        }

        #[inline]
        fn null() -> ReadPtr<'a, T> {
            ReadPtr::new(ptr::null())
        }
    }

    impl<'a, T: 'a> Data for WriteData<'a, T> {
        type DataPtr = RwPtr<'a, T>;
        type Item = &'a mut T;

        #[inline]
        fn len(&self) -> usize {
            self.store_ref().len()
        }

        #[inline]
        fn get(&mut self, loc: usize) -> &'a mut T {
            unsafe { self.store_mut_ref().get_unchecked_mut(loc) }
        }

        #[inline]
        fn unwrap(&mut self) -> RwPtr<'a, T> {
            RwPtr::new(self.store_mut_ref().as_mut_ptr())
        }

        #[inline]
        fn null() -> RwPtr<'a, T> {
            RwPtr::new(ptr::null_mut())
        }
    }

    impl<'a, T> Query for Read<'a, T>
    where
        T: 'static + Component,
    {
        type QueryItem = ReadData<'a, T>;
        type DataType = T;

        #[inline]
        fn execute(shard: &Shard) -> ReadData<'a, T> {
            ReadData::new(shard.data_ptr::<T>())
        }
    }

    impl<'a, T> Query for Write<'a, T>
    where
        T: 'static + Component,
    {
        type QueryItem = WriteData<'a, T>;
        type DataType = T;

        #[inline]
        fn execute(shard: &Shard) -> WriteData<'a, T> {
            WriteData::new(shard.data_mut_ptr::<T>())
        }
    }

    macro_rules! ptr_tup {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> IndexablePtrTup for ($($field_type,)*)
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

    impl IndexablePtrTup for () {
        type ItemTup = ();

        fn index(&self, _idx: usize) -> Self::ItemTup {
            unimplemented!()
        }
    }

    impl<T> IndexablePtrTup for T
    where
        T: Indexable,
    {
        type ItemTup = T::Item;

        #[inline]
        fn index(&self, idx: usize) -> Self::ItemTup {
            self.index(idx)
        }
    }

    macro_rules! component_tup {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> ComponentDataTup for ($($field_type,)*)
            where
                $($field_type: Data,)*
            {
                type PtrTup = ($($field_type::DataPtr,)*);
                type ItemTup = ($($field_type::Item,)*);

                #[inline]
                fn get_entity(&mut self, loc: usize) -> Self::ItemTup {
                    ($(self.$field_seq.get(loc),)*)
                }

                #[inline]
                fn get_ptr_tup(&mut self) -> (usize, Self::PtrTup) {
                    (self.0.len(), ($(self.$field_seq.unwrap(),)*))
                }

                #[inline]
                unsafe fn get_zero_ptr_tup() -> Self::PtrTup {
                    ($($field_type::null(),)*)
                }
            }
        };
    }

    component_tup!(A:0);
    component_tup!(A:0, B:1);
    component_tup!(A:0, B:1, C:2);
    component_tup!(A:0, B:1, C:2, D:3);
    component_tup!(A:0, B:1, C:2, D:3, E:4);
    component_tup!(A:0, B:1, C:2, D:3, E:4, F:5);
    component_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    component_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

    impl ComponentDataTup for () {
        type PtrTup = ();
        type ItemTup = ();

        fn get_entity(&mut self, _loc: usize) -> Self::ItemTup {
            unimplemented!()
        }

        fn get_ptr_tup(&mut self) -> (usize, Self::PtrTup) {
            unimplemented!()
        }

        unsafe fn get_zero_ptr_tup() -> Self::PtrTup {
            unimplemented!()
        }
    }

    impl<T> ComponentDataTup for T
    where
        T: Data,
    {
        type PtrTup = T::DataPtr;
        type ItemTup = T::Item;

        #[inline]
        fn get_entity(&mut self, loc: usize) -> Self::ItemTup {
            self.get(loc)
        }

        #[inline]
        fn get_ptr_tup(&mut self) -> (usize, Self::PtrTup) {
            (self.len(), self.unwrap())
        }

        #[inline]
        unsafe fn get_zero_ptr_tup() -> Self::PtrTup {
            T::null()
        }
    }

    macro_rules! component_def {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> ComponentQueryTup for ($($field_type,)*)
            where
                $($field_type: Query,)*
                $($field_type::DataType: 'static + Component,)*
            {
                type DataTup = ($($field_type::QueryItem,)*);

                #[inline]
                fn reify_shard(shard: &Shard) -> Self::DataTup {
                    ($($field_type::execute(shard),)*)
                }

                #[inline]
                fn get_shard_key() -> ShardKey {
                    ($($field_type::DataType::get_class())|*).into()
                }
            }
        };
    }

    component_def!(A:0);
    component_def!(A:0, B:1);
    component_def!(A:0, B:1, C:2);
    component_def!(A:0, B:1, C:2, D:3);
    component_def!(A:0, B:1, C:2, D:3, E:4);
    component_def!(A:0, B:1, C:2, D:3, E:4, F:5);
    component_def!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    component_def!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

    impl ComponentQueryTup for () {
        type DataTup = ();

        fn reify_shard(_shard: &Shard) -> Self::DataTup {
            unimplemented!()
        }

        #[inline]
        fn get_shard_key() -> ShardKey {
            ShardKey::empty()
        }
    }

    impl<T> ComponentQueryTup for T
    where
        T: Query,
        T::DataType: 'static + Component,
    {
        type DataTup = T::QueryItem;

        #[inline]
        fn reify_shard(shard: &Shard) -> Self::DataTup {
            T::execute(shard)
        }

        #[inline]
        fn get_shard_key() -> ShardKey {
            T::DataType::get_class().into()
        }
    }
}

pub trait ResourceDataTup {
    type ItemTup;

    fn borrow(&mut self) -> Self::ItemTup;
}

pub trait ResourceQueryTup {
    type DataTup: ResourceDataTup;

    fn reify(resources: &AnyMap) -> Self::DataTup;
}

pub mod resource {
    use super::{AnyMap, PhantomData, Read, ResourceDataTup, ResourceQueryTup, Write};
    use std::ptr::NonNull;

    pub trait Data {
        type Item;

        fn get_item(&mut self) -> Self::Item;
    }

    pub struct Reader<'a, T> {
        data: NonNull<T>,
        _x: PhantomData<&'a ()>,
    }

    impl<'a, T> Data for Reader<'a, T>
    where
        T: 'a,
    {
        type Item = &'a T;

        fn get_item(&mut self) -> Self::Item {
            unsafe { &*self.data.as_ptr() }
        }
    }

    pub struct Writer<'a, T> {
        data: NonNull<T>,
        _x: PhantomData<&'a ()>,
    }

    impl<'a, T> Data for Writer<'a, T>
    where
        T: 'a,
    {
        type Item = &'a mut T;

        fn get_item(&mut self) -> Self::Item {
            unsafe { &mut *self.data.as_ptr() }
        }
    }

    pub trait Query {
        type Data: Data;

        fn acquire(resources: &AnyMap) -> Self::Data;
    }

    impl<'a, T> Query for Read<'a, T>
    where
        T: 'static,
    {
        type Data = Reader<'a, T>;

        fn acquire(resources: &AnyMap) -> Self::Data {
            Reader {
                data: *resources.get::<NonNull<T>>().expect("Resource missing"),
                _x: PhantomData,
            }
        }
    }

    impl<'a, T> Query for Write<'a, T>
    where
        T: 'static,
    {
        type Data = Writer<'a, T>;

        fn acquire(resources: &AnyMap) -> Self::Data {
            Writer {
                data: *resources.get::<NonNull<T>>().expect("Resource missing"),
                _x: PhantomData,
            }
        }
    }

    macro_rules! resource_tup {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> ResourceDataTup for ($($field_type,)*)
            where
                $($field_type: Data,)*
            {
                type ItemTup = ($($field_type::Item,)*);

                #[inline]
                fn borrow(&mut self) -> Self::ItemTup {
                    ($(self.$field_seq.get_item(),)*)
                }
            }
        };
    }

    resource_tup!(A:0);
    resource_tup!(A:0, B:1);
    resource_tup!(A:0, B:1, C:2);
    resource_tup!(A:0, B:1, C:2, D:3);
    resource_tup!(A:0, B:1, C:2, D:3, E:4);
    resource_tup!(A:0, B:1, C:2, D:3, E:4, F:5);
    resource_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    resource_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

    impl ResourceDataTup for () {
        type ItemTup = ();

        fn borrow(&mut self) -> Self::ItemTup {
            unimplemented!()
        }
    }

    impl<T> ResourceDataTup for T
    where
        T: Data,
    {
        type ItemTup = T::Item;

        #[inline]
        fn borrow(&mut self) -> Self::ItemTup {
            self.get_item()
        }
    }

    macro_rules! resource_def {
        ($( $field_type:ident ),*) => {
            impl<$($field_type),*> ResourceQueryTup for ($($field_type,)*)
            where
                $($field_type: Query,)*
            {
                type DataTup = ($($field_type::Data,)*);

                #[inline]
                fn reify(resources: &AnyMap) -> Self::DataTup {
                    ($($field_type::acquire(resources),)*)
                }
            }
        };
    }

    resource_def!(A);
    resource_def!(A, B);
    resource_def!(A, B, C);
    resource_def!(A, B, C, D);
    resource_def!(A, B, C, D, E);
    resource_def!(A, B, C, D, E, F);
    resource_def!(A, B, C, D, E, F, G);
    resource_def!(A, B, C, D, E, F, G, H);

    impl ResourceQueryTup for () {
        type DataTup = ();

        fn reify(_: &AnyMap) -> Self::DataTup {}
    }

    impl<T> ResourceQueryTup for T
    where
        T: Query,
    {
        type DataTup = T::Data;

        #[inline]
        fn reify(resources: &AnyMap) -> Self::DataTup {
            T::acquire(resources)
        }
    }
}

pub mod context {
    use super::{ComponentCoords, ComponentDataTup, EntityId, HashMap, IndexMap, IndexablePtrTup, ShardKey};
    use indexmap::map::ValuesMut;

    pub struct ComponentContext<'a, T>
    where
        T: ComponentDataTup,
    {
        shards: &'a mut IndexMap<ShardKey, T>,
        entities: &'a HashMap<EntityId, ComponentCoords>,
    }

    impl<'a, T> ComponentContext<'a, T>
    where
        T: ComponentDataTup,
    {
        #[inline]
        pub fn new(
            shards: &'a mut IndexMap<ShardKey, T>,
            entities: &'a HashMap<EntityId, ComponentCoords>,
        ) -> ComponentContext<'a, T> {
            ComponentContext { shards, entities }
        }

        #[allow(unused_variables)]
        #[inline]
        pub fn for_each<F>(&mut self, entities: &[EntityId], f: F)
        where
            F: FnMut(T::ItemTup),
        {
            entities
                .iter()
                .filter_map(move |id| {
                    let (shard_key, loc) = self.entities.get(id)?;
                    let shard = self.shards.get_mut(shard_key)?;
                    Some(shard.get_entity(*loc))
                })
                .for_each(f);
        }

        #[inline]
        pub fn iter(&mut self) -> ComponentIterator<T> {
            Self::iter_core(&mut self.shards)
        }

        #[inline]
        fn iter_core(shards: &mut IndexMap<ShardKey, T>) -> ComponentIterator<T> {
            let mut stream = shards.values_mut();

            unsafe {
                let (size, shard) = match stream.next() {
                    Some(item) => item.get_ptr_tup(),
                    _ => (0usize, T::get_zero_ptr_tup()),
                };

                ComponentIterator {
                    stream,
                    shard,
                    size,
                    counter: 0,
                }
            }
        }
    }

    impl<'a, T> IntoIterator for ComponentContext<'a, T>
    where
        T: ComponentDataTup,
    {
        type Item = <T::PtrTup as IndexablePtrTup>::ItemTup;
        type IntoIter = ComponentIterator<'a, T>;

        #[inline]
        fn into_iter(self) -> ComponentIterator<'a, T> {
            Self::iter_core(self.shards)
        }
    }

    pub struct ComponentIterator<'a, T>
    where
        T: ComponentDataTup,
    {
        stream: ValuesMut<'a, ShardKey, T>,
        shard: T::PtrTup,
        size: usize,
        counter: usize,
    }

    impl<'a, T> Iterator for ComponentIterator<'a, T>
    where
        T: ComponentDataTup,
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

                let item = self.stream.next()?;
                let (size, shard) = item.get_ptr_tup();
                self.shard = shard;
                self.size = size;
                self.counter = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentVec;
    use crate::component_init;
    use crate::identity::{ComponentClass, Topic};
    use crate::topic_init;
    use serde_derive::{Deserialize, Serialize};
    use std::marker::PhantomData;
    use std::sync::atomic::ATOMIC_USIZE_INIT;
    use std::sync::Arc;

    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompA(i32);

    component_init!(CompA);

    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompB(u64);

    component_init!(CompB);

    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompC {
        x: i32,
        y: i32,
    }

    component_init!(CompC);

    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
    struct CompD(u8);

    component_init!(CompD);

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct Msg(i32);

    topic_init!(Msg);

    fn setup() -> (ComponentClass, ComponentClass, ComponentClass, ComponentClass) {
        (
            CompA::get_class(),
            CompB::get_class(),
            CompC::get_class(),
            CompD::get_class(),
        )
    }

    fn make_shard_1() -> Shard {
        let mut map: HashMap<_, Box<ComponentVec>> = HashMap::new();
        let comp_1_id = CompA::get_class();
        let comp_2_id = CompB::get_class();

        let data_a = vec![CompA(0), CompA(1), CompA(2)];
        let data_b = vec![CompB(0), CompB(1), CompB(2)];

        map.insert(comp_1_id, Box::new(data_a));
        map.insert(comp_2_id, Box::new(data_b));

        let entities: Vec<EntityId> = vec![0.into(), 1.into(), 2.into()];

        Shard::new_with_ents(comp_1_id + comp_2_id + EntityId::get_class(), entities, map)
    }

    fn make_shard_2() -> Shard {
        let mut map: HashMap<_, Box<ComponentVec>> = HashMap::new();
        let comp_1_id = CompB::get_class();
        let comp_2_id = CompC::get_class();

        map.insert(comp_1_id, Box::new(Vec::<CompB>::new()));
        map.insert(comp_2_id, Box::new(Vec::<CompC>::new()));

        Shard::new(comp_1_id + comp_2_id + EntityId::get_class(), map)
    }

    #[test]
    fn test_check_shard() {
        let (a_id, b_id, c_id, d_id) = setup();

        struct TestSystem<'a>(PhantomData<&'a ()>);

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = Components<(Read<'a, CompA>, Read<'a, CompB>, Write<'a, CompC>)>;

            fn run(&mut self, _ctx: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {
                unimplemented!()
            }
        }

        let system = SystemRuntime::new(TestSystem(PhantomData));

        assert!(system.check_shard(a_id + b_id + c_id + d_id));
        assert!(system.check_shard(a_id + b_id + c_id));
        assert!(!system.check_shard(a_id + b_id));
        assert!(!system.check_shard(b_id + c_id));
        assert!(!system.check_shard(a_id.into()));
    }

    #[test]
    fn test_add_shard() {
        struct TestSystem<'a>(PhantomData<&'a ()>);

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = Components<Read<'a, CompB>>;

            fn run(&mut self, _ctx: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {
                unimplemented!()
            }
        }

        let mut system = SystemRuntime::new(TestSystem(PhantomData));

        let shard_1 = make_shard_1();
        let shard_2 = make_shard_2();

        system.add_shard(&shard_1);
        system.add_shard(&shard_2);

        assert_eq!(
            system.data.shards[&shard_1.key].get_ptr(),
            shard_1.data_ptr::<CompB>()
        );
        assert_eq!(
            system.data.shards[&shard_2.key].get_ptr(),
            shard_2.data_ptr::<CompB>()
        );
    }

    #[test]
    fn test_remove_shard() {
        struct TestSystem<'a>(PhantomData<&'a ()>);

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = Components<Read<'a, CompB>>;

            fn run(&mut self, _ctx: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {
                unimplemented!()
            }
        }

        let mut system = SystemRuntime::new(TestSystem(PhantomData));

        let shard_1 = make_shard_1();
        let shard_2 = make_shard_2();

        system.add_shard(&shard_1);
        system.add_shard(&shard_2);

        system.remove_shard(shard_1.key);

        assert_eq!(
            system.data.shards[&shard_2.key].get_ptr(),
            shard_2.data_ptr::<CompB>()
        );
        assert!(!system.data.shards.contains_key(&shard_1.key));
    }

    #[test]
    fn test_run() {
        struct TestSystem<'a> {
            collect_run: Vec<(EntityId, CompA, CompB)>,
            collect_foreach: Vec<(EntityId, CompA, CompB)>,
            collect_messages: Vec<Msg>,
            _p: PhantomData<&'a ()>,
        };

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = Components<(Read<'a, EntityId>, Read<'a, CompA>, Write<'a, CompB>)>;

            fn run(&mut self, mut ctx: Context<Self::Data>, _tx: &mut TransactionContext, mut msg: Router) {
                let mut entities = Vec::new();

                for (&id, a, b) in ctx.components() {
                    entities.push(id);
                    self.collect_run.push((id, a.clone(), b.clone()));
                }

                ctx.components().for_each(&entities, |(id, a, b)| {
                    self.collect_foreach.push((*id, a.clone(), b.clone()));
                });

                for message in msg.read::<Msg>() {
                    self.collect_messages.push(message.clone());
                }

                msg.publish(Msg(100));
                msg.publish(Msg(101));
                msg.publish(Msg(102));
            }
        }

        let mut system = SystemRuntime::new(TestSystem {
            collect_run: Vec::new(),
            collect_foreach: Vec::new(),
            collect_messages: Vec::new(),
            _p: PhantomData,
        });

        let shard_1 = make_shard_1();

        system.add_shard(&shard_1);

        let mut entities: HashMap<EntityId, _> = HashMap::new();
        entities.insert(0.into(), (shard_1.key, 0));
        entities.insert(1.into(), (shard_1.key, 1));
        entities.insert(2.into(), (shard_1.key, 2));

        let mut transactions = TransactionContext::new(Arc::new(ATOMIC_USIZE_INIT));

        // Set up central bus with some messages
        let mut messages = Bus::new();
        messages.publish(Msg(1));
        messages.publish(Msg(2));

        system.run(
            &entities,
            &mut transactions,
            &messages,
            0.02,
            time::Instant::now(),
        );

        assert_eq!(system.runstate.collect_run.len(), 3);
        assert_eq!(system.runstate.collect_run[0], (0.into(), CompA(0), CompB(0)));
        assert_eq!(system.runstate.collect_run[1], (1.into(), CompA(1), CompB(1)));
        assert_eq!(system.runstate.collect_run[2], (2.into(), CompA(2), CompB(2)));

        assert_eq!(system.runstate.collect_foreach.len(), 3);
        assert_eq!(system.runstate.collect_foreach[0], (0.into(), CompA(0), CompB(0)));
        assert_eq!(system.runstate.collect_foreach[1], (1.into(), CompA(1), CompB(1)));
        assert_eq!(system.runstate.collect_foreach[2], (2.into(), CompA(2), CompB(2)));

        assert_eq!(system.messages.read::<Msg>(), &[Msg(100), Msg(101), Msg(102)]);
        assert_eq!(system.runstate.collect_messages, vec![Msg(1), Msg(2)])
    }
}

use crate::component2::Component;
use crate::component3::Shard;
use crate::entity2::{EntityId, TransactionContext};
use crate::identity2::ShardKey;
use indexmap::IndexMap;

pub trait System {
    type Data: QueryTup;

    fn run(&mut self, data: &mut SystemData<Self::Data>, transactions: &mut TransactionContext);
}

pub trait SystemRuntime {
    fn run(&mut self, transactions: &mut TransactionContext);
    fn add_shard(&mut self, shard: &Shard);
    fn remove_shard(&mut self, key: ShardKey);
    fn check_shard(&self, shard_key: ShardKey) -> bool;
}

pub struct SystemData<T>
where
    T: QueryTup,
{
    shards: IndexMap<ShardKey, T::DataTup>,
}

impl<T> SystemData<T>
where
    T: QueryTup,
{
    #[inline]
    pub(crate) fn new() -> SystemData<T> {
        SystemData { shards: IndexMap::new() }
    }

    #[inline]
    pub fn context(&mut self) -> context::Context<<T as QueryTup>::DataTup> {
        context::Context::new(&mut self.shards)
    }

    #[inline]
    pub(crate) fn add_shard(&mut self, shard: &Shard) {
        self.shards.insert(shard.key, T::reify_shard(shard));
    }

    #[inline]
    pub(crate) fn remove_shard(&mut self, key: ShardKey) {
        self.shards.remove(&key);
    }
}

pub struct SystemEntry<T>
where
    T: System,
{
    system: T,
    shard_key: ShardKey,
    data: SystemData<T::Data>,
}

impl<T> SystemEntry<T>
where
    T: System,
{
    #[inline]
    pub(crate) fn new(system: T) -> SystemEntry<T> {
        SystemEntry {
            system,
            shard_key: T::Data::get_shard_key(),
            data: SystemData::new(),
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
    fn run(&mut self, transactions: &mut TransactionContext) {
        self.system.run(&mut self.data, transactions);
    }

    #[inline]
    fn add_shard(&mut self, shard: &Shard) {
        self.data.add_shard(shard);
    }

    #[inline]
    fn remove_shard(&mut self, key: ShardKey) {
        self.data.remove_shard(key);
    }

    #[inline]
    fn check_shard(&self, shard_key: ShardKey) -> bool {
        self.shard_key.contains_key(shard_key)
    }
}

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

pub trait IndexablePtrTup {
    type ItemTup;

    fn index(&self, idx: usize) -> Self::ItemTup;
}

pub mod store {
    use super::{Component, Data, Indexable, Query, Shard};
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
            // No need for explicit guards as the scheduler guarantees to maintain the reference aliasing invariants.
            ReadData { store, _x: PhantomData }
        }

        #[inline]
        fn store_ref(&self) -> &'a Vec<T> {
            unsafe { &*self.store }
        }
    }

    impl<'a, T> WriteData<'a, T> {
        #[inline]
        fn new(store: *mut Vec<T>) -> WriteData<'a, T> {
            // No need for explicit guards as the scheduler guarantees to maintain the reference aliasing invariants.
            WriteData { store, _x: PhantomData }
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

    pub struct Read<'a, T>
    where
        T: Component,
    {
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> Query for Read<'a, T>
    where
        T: Component,
    {
        type QueryItem = ReadData<'a, T>;
        type DataType = T;

        #[inline]
        fn execute(shard: &Shard) -> ReadData<'a, T> {
            ReadData::new(shard.data_ptr::<T>())
        }
    }

    pub struct Write<'a, T>
    where
        T: Component,
    {
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> Query for Write<'a, T>
    where
        T: Component,
    {
        type QueryItem = WriteData<'a, T>;
        type DataType = T;

        #[inline]
        fn execute(shard: &Shard) -> WriteData<'a, T> {
            WriteData::new(shard.data_mut_ptr::<T>())
        }
    }
}

pub trait DataTup {
    type PtrTup: IndexablePtrTup;
    type ItemTup;

    fn get_entity(&mut self, loc: usize) -> Self::ItemTup;

    fn get_ptr_tup(&mut self) -> (usize, Self::PtrTup);
    unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
}

pub trait QueryTup {
    type DataTup: DataTup;

    fn reify_shard(shard: &Shard) -> Self::DataTup;
    fn get_shard_key() -> ShardKey;
}

pub mod join {
    use super::{Component, Data, DataTup, Indexable, IndexablePtrTup, Query, QueryTup, Shard, ShardKey};

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

    impl<T> IndexablePtrTup for T where T: Indexable {
        type ItemTup = T::Item;

        #[inline]
        fn index(&self, idx: usize) -> Self::ItemTup {
            self.index(idx)
        }
    }

    macro_rules! data_tup {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> DataTup for ($($field_type,)*)
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

    data_tup!(A:0);
    data_tup!(A:0, B:1);
    data_tup!(A:0, B:1, C:2);
    data_tup!(A:0, B:1, C:2, D:3);
    data_tup!(A:0, B:1, C:2, D:3, E:4);
    data_tup!(A:0, B:1, C:2, D:3, E:4, F:5);
    data_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    data_tup!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);

    impl DataTup for () {
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

    impl<T> DataTup for T where T: Data {
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

    macro_rules! system_def {
        ($( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> QueryTup for ($($field_type,)*)
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
                    ($($field_type::DataType::get_unique_id())|*).into()
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

    impl QueryTup for () {
        type DataTup = ();

        fn reify_shard(_shard: &Shard) -> Self::DataTup {
            unimplemented!()
        }

        fn get_shard_key() -> ShardKey {
            ShardKey::empty()
        }
    }

    impl<T> QueryTup for T where T: Query, T::DataType: 'static + Component {
        type DataTup = T::QueryItem;

        fn reify_shard(shard: &Shard) -> Self::DataTup {
            T::execute(shard)
        }

        fn get_shard_key() -> ShardKey {
            T::DataType::get_unique_id().into()
        }
    }
}

pub mod context {
    use super::{DataTup, EntityId, IndexMap, IndexablePtrTup, ShardKey};
    use indexmap::map::ValuesMut;

    pub struct Context<'a, T>
    where
        T: DataTup,
    {
        shards: &'a mut IndexMap<ShardKey, T>,
    }

    impl<'a, T> Context<'a, T>
    where
        T: DataTup,
    {
        #[inline]
        pub fn new(shards: &'a mut IndexMap<ShardKey, T>) -> Context<'a, T> {
            Context { shards }
        }

        #[allow(unused_variables)]
        #[inline]
        pub fn for_each<F>(&mut self, entities: &[EntityId], f: F)
        where
            F: FnMut(T::ItemTup),
        {
            // TODO: Add entity getting
            //            entities
            //                .iter()
            //                .filter_map(move |&id| {
            //                    self.stores.get_entity(id)
            //                })
            //                .for_each(f);
            unimplemented!()
        }

        #[inline]
        pub fn iter(&mut self) -> ComponentIterator<T> {
            Self::iter_core(&mut self.shards)
        }

        #[inline]
        fn iter_core(shards: & mut IndexMap<ShardKey, T>) -> ComponentIterator<T> {
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

    impl<'a, T> IntoIterator for Context<'a, T> where T: DataTup {
        type Item = <T::PtrTup as IndexablePtrTup>::ItemTup;
        type IntoIter = ComponentIterator<'a, T>;

        fn into_iter(self) -> ComponentIterator<'a, T> {
            Self::iter_core(self.shards)
        }
    }

    pub struct ComponentIterator<'a, T>
    where
        T: DataTup,
    {
        stream: ValuesMut<'a, ShardKey, T>,
        shard: T::PtrTup,
        size: usize,
        counter: usize,
    }

    impl<'a, T> Iterator for ComponentIterator<'a, T>
    where
        T: DataTup,
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

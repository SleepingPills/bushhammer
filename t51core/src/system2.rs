use crate::entity::{Entity, EntityStore};
use crate::object::{BundleId, ComponentId, EntityId};
use indexmap::IndexMap;

pub trait System {
    type Data: SystemDef;

    fn run(&mut self, data: &SystemData<Self::Data>, entities: EntityStore);
}

pub struct SystemData<T>
where
    T: SystemDef,
{
    stores: T,
    bundles: IndexMap<BundleId, <T::JoinItem as Joined>::Indexer>,
}

impl<T> SystemData<T>
where
    T: SystemDef,
{
    #[inline]
    pub fn context(&self) -> core::Context<<T as SystemDef>::JoinItem> {
        core::Context::new(self.stores.as_joined(), &self.bundles)
    }
}

pub struct SystemEntry<T>
where
    T: System,
{
    system: T,
    data: SystemData<T::Data>,
}


pub trait SystemRuntime {
    fn run(&mut self, entities: EntityStore);
    fn add_entity(&mut self, entity: &Entity);
    fn remove_entity(&mut self, id: EntityId);
    fn update_entity_bundle(&mut self, entity: &Entity);
    fn add_bundle(&mut self, bundle: support::BundleDef);
    fn remove_bundle(&mut self, id: BundleId);
    fn get_required_components(&self) -> Vec<ComponentId>;
}

impl<T> SystemRuntime for SystemEntry<T>
where
    T: System,
{
    #[inline]
    fn run(&mut self, entities: EntityStore) {
        self.system.run(&self.data, entities);
    }

    #[inline]
    fn add_entity(&mut self, entity: &Entity) {
        self.data.entity_map.insert(entity.id, entity.bundle_id);
    }

    #[inline]
    fn remove_entity(&mut self, id: EntityId) {
        self.data.entity_map.remove(&id);
    }

    #[inline]
    fn update_entity_bundle(&mut self, entity: &Entity) {
        self.data.entity_map.insert(entity.id, entity.bundle_id);
    }

    #[inline]
    fn add_bundle(&mut self, bundle: support::BundleDef) {
        let data_bundle = support::DataBundle::new(bundle);
        self.data.bundles.insert(data_bundle.bundle_id(), data_bundle);
    }

    #[inline]
    fn remove_bundle(&mut self, id: BundleId) {
        self.data.bundles.remove(&id);
    }

    #[inline]
    fn get_required_components(&self) -> Vec<ComponentId> {
        T::Data::get_comp_ids()
    }
}

pub trait Indexable {
    type Item;

    fn index(&self, idx: usize) -> Self::Item;
}

pub trait Query {
    type DataPtr: Indexable;

    fn len(&self, section: usize) -> usize;
    fn unwrap(&self, section: usize) -> Self::DataPtr;
    fn null() -> Self::DataPtr;
}

pub trait Store {
    type QueryItem: Query;
    type DataType;

    fn get_query(&self) -> Self::QueryItem;
}

pub trait IndexablePtrTup {
    type ItemTup;

    fn index(&self, idx: usize) -> Self::ItemTup;
}

pub mod store {
    use super::{Indexable, Query, Store};
    use crate::component::ComponentStore;
    use crate::sync::{ReadGuard, RwCell, RwGuard};
    use std::marker::PhantomData;
    use std::ptr;
    use std::sync::Arc;

    #[repr(transparent)]
    pub struct SharedConst<'a, T>(*const T, PhantomData<&'a ()>);

    impl<'a, T> SharedConst<'a, T> {
        #[inline]
        fn new(ptr: *const T) -> SharedConst<'a, T> {
            SharedConst(ptr, PhantomData)
        }
    }

    #[repr(transparent)]
    pub struct SharedMut<'a, T>(*mut T, PhantomData<&'a ()>);

    impl<'a, T> SharedMut<'a, T> {
        #[inline]
        fn new(ptr: *mut T) -> SharedMut<'a, T> {
            SharedMut(ptr, PhantomData)
        }
    }

    impl<'a, T: 'a> Indexable for SharedConst<'a, T> {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            unsafe { &*self.0.add(idx) }
        }
    }

    impl<'a, T: 'a> Indexable for SharedMut<'a, T> {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            unsafe { &mut *self.0.add(idx) }
        }
    }

    #[repr(transparent)]
    pub struct ReadQuery<'a, T> {
        store: ReadGuard<ComponentStore<T>>,
        _x: PhantomData<&'a T>,
    }

    #[repr(transparent)]
    pub struct WriteQuery<'a, T> {
        store: RwGuard<ComponentStore<T>>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> ReadQuery<'a, T> {
        #[inline]
        fn new(store: ReadGuard<ComponentStore<T>>) -> ReadQuery<'a, T> {
            ReadQuery { store, _x: PhantomData }
        }
    }

    impl<'a, T> WriteQuery<'a, T> {
        #[inline]
        fn new(store: RwGuard<ComponentStore<T>>) -> WriteQuery<'a, T> {
            WriteQuery { store, _x: PhantomData }
        }
    }

    impl<'a, T: 'a> Query for ReadQuery<'a, T> {
        type DataPtr = SharedConst<'a, T>;

        #[inline]
        fn len(&self, section: usize) -> usize {
            self.store.section_len(section)
        }

        #[inline]
        fn unwrap(&self, section: usize) -> SharedConst<'a, T> {
            SharedConst::new(self.store.get_data_ptr(section))
        }

        #[inline]
        fn null() -> SharedConst<'a, T> {
            SharedConst::new(ptr::null())
        }
    }

    impl<'a, T: 'a> Query for WriteQuery<'a, T> {
        type DataPtr = SharedMut<'a, T>;

        #[inline]
        fn len(&self, section: usize) -> usize {
            self.store.section_len(section)
        }

        #[inline]
        fn unwrap(&self, section: usize) -> SharedMut<'a, T> {
            SharedMut::new(self.store.get_data_mut_ptr(section))
        }

        #[inline]
        fn null() -> SharedMut<'a, T> {
            SharedMut::new(ptr::null_mut())
        }
    }

    pub struct Read<'a, T> {
        store: Arc<RwCell<ComponentStore<T>>>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> Store for Read<'a, T> {
        type QueryItem = ReadQuery<'a, T>;
        type DataType = T;

        #[inline]
        fn get_query(&self) -> ReadQuery<'a, T> {
            ReadQuery::new(self.store.read())
        }
    }

    pub struct Write<'a, T> {
        store: Arc<RwCell<ComponentStore<T>>>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> Store for Write<'a, T> {
        type QueryItem = WriteQuery<'a, T>;
        type DataType = T;

        #[inline]
        fn get_query(&self) -> WriteQuery<'a, T> {
            WriteQuery::new(self.store.write())
        }
    }
}

pub trait Joined {
    type PtrTup: IndexablePtrTup;
    type Indexer;

    fn get_ptr_tup(&self, idx: &Self::Indexer) -> (usize, Self::PtrTup);
    unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
}

pub trait SystemDef {
    type JoinItem: Joined;

    fn as_joined(&self) -> Self::JoinItem;
    fn get_comp_ids() -> Vec<ComponentId>;
}

pub mod join {
    use super::{ComponentId, Indexable, IndexablePtrTup, Joined, Query, Store, SystemDef};

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
                type Indexer = ($(_decl_system_replace_expr!($field_type usize)),*,);

                #[inline]
                fn get_ptr_tup(&self, idx: &($(_decl_system_replace_expr!($field_type usize)),*,)) -> (usize, ($($field_type::DataPtr),*,)) {
                    (self.0.len(idx.0), ($(self.$field_seq.unwrap(idx.$field_seq)),*,))
                }

                #[inline]
                unsafe fn get_zero_ptr_tup() -> ($($field_type::DataPtr),*,) {
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
        ($field_count:tt; $( $field_type:ident:$field_seq:tt ),*) => {
            impl<$($field_type),*> SystemDef for ($($field_type),*,)
            where
                $($field_type: Store),*,
                $($field_type::DataType: 'static),*
            {
                type JoinItem = ($($field_type::QueryItem),*,);

                #[inline]
                fn as_joined(&self) -> ($($field_type::QueryItem),*,) {
                    ($(self.$field_seq.get_query()),*,)
                }

                #[inline]
                fn get_comp_ids() -> Vec<ComponentId> {
                    vec![$(ComponentId::new::<$field_type::DataType>()),*]
                }

//                #[inline]
//                fn reify(bundle: &Vec<VoidPtr>) -> ($($field_type),*,) {
//                    match bundle.len() {
//                        $field_count => ($(bundle[$field_seq].into()),*,),
//                        len => panic!("Recieved bundle rank {}, expected {}", len, $field_count),
//                    }
//                }

//                #[inline]
//                fn get_by_index(&self, idx: usize) -> ($($field_type::Item),*,) {
//                    ($(self.$field_seq.index(idx)),*,)
//                }
            }
        };
    }

    system_def!(1; A:0);
    system_def!(2; A:0, B:1);
    system_def!(3; A:0, B:1, C:2);
    system_def!(4; A:0, B:1, C:2, D:3);
    system_def!(5; A:0, B:1, C:2, D:3, E:4);
    system_def!(6; A:0, B:1, C:2, D:3, E:4, F:5);
    system_def!(7; A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    system_def!(8; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
}

pub mod core {
    use super::{IndexMap, IndexablePtrTup, Joined};
    use crate::object::BundleId;
    use indexmap::map::Values;

    pub struct Context<'a, T>
    where
        T: Joined,
    {
        stores: T,
        bundles: &'a IndexMap<BundleId, T::Indexer>,
    }

    impl<'a, T> Context<'a, T>
    where
        T: Joined,
    {
        #[inline]
        pub fn new(stores: T, bundles: &'a IndexMap<BundleId, T::Indexer>) -> Context<'a, T> {
            Context { stores, bundles }
        }

        //        #[inline]
        //        pub fn get_by_id(&mut self, id: EntityId) -> T::ItemTup {
        //            let bundle_id = self.entity_map[&id];
        //            let bundle = &self.bundles[&bundle_id];
        //            bundle.get_by_id(id)
        //        }
    }

    impl<'a, T> IntoIterator for Context<'a, T>
    where
        T: 'a + Joined,
    {
        type Item = <T::PtrTup as IndexablePtrTup>::ItemTup;
        type IntoIter = ComponentIterator<'a, T>;

        #[inline]
        fn into_iter(self) -> ComponentIterator<'a, T> {
            let mut stream = self.bundles.values();

            unsafe {
                let (size, bundle) = match stream.next() {
                    Some(item) => self.stores.get_ptr_tup(item),
                    _ => (0usize, T::get_zero_ptr_tup()),
                };

                ComponentIterator {
                    stores: self.stores,
                    stream,
                    bundle,
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
        stores: T,
        stream: Values<'a, BundleId, T::Indexer>,
        bundle: T::PtrTup,
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
                    return Some(self.bundle.index(idx));
                }

                if let Some(item) = self.stream.next() {
                    let (size, bundle) = self.stores.get_ptr_tup(item);
                    self.bundle = bundle;
                    self.size = size;
                    self.counter = 0;
                } else {
                    return None;
                }
            }
        }
    }
}

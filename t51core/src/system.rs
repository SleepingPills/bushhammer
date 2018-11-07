use crate::component;
use crate::component::ComponentCoords;
use crate::entity::{Entity, EntityStore};
use crate::object::{BundleId, ComponentId, EntityId};
use crate::sync::RwCell;
use indexmap::IndexMap;
use hashbrown::HashMap;
use std::sync::Arc;

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
    entity_map: Arc<RwCell<HashMap<EntityId, Entity>>>,
    components: Vec<ComponentId>,
}

impl<T> SystemData<T>
where
    T: SystemDef,
{
    #[inline]
    pub fn context(&self) -> context::Context<<T as SystemDef>::JoinItem> {
        context::Context::new(
            self.stores.as_joined(),
            &self.bundles,
            self.entity_map.read(),
            &self.components,
        )
    }
}

pub struct SystemEntry<T>
where
    T: System,
{
    system: T,
    data: SystemData<T::Data>,
}

trait SystemRuntime {
    fn run(&mut self, entities: EntityStore);
    fn add_bundle(&mut self, bundle: &component::Bundle);
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
    fn add_bundle(&mut self, bundle: &component::Bundle) {
        let locs = bundle.get_locs(&self.data.components);
        unimplemented!()
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
    type Item;

    fn len(&self, section: usize) -> usize;
    fn get_by_coords(&self, coords: ComponentCoords) -> Self::Item;
    fn unwrap(&mut self, section: usize) -> Self::DataPtr;
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
    use super::{Arc, ComponentCoords, Indexable, Query, RwCell, Store};
    use crate::component::ComponentStore;
    use crate::sync::{ReadGuard, RwGuard};
    use std::marker::PhantomData;
    use std::ptr;

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
        type Item = &'a T;

        #[inline]
        fn len(&self, section: usize) -> usize {
            self.store.section_len(section)
        }

        #[inline]
        fn get_by_coords(&self, coords: ComponentCoords) -> &'a T {
            let (section, loc) = coords;
            let ptr = self.store.get_data_ptr(section);
            unsafe {
                &*ptr.add(loc)
            }
            // Can't do the safe thing here due to lack of generic associated types as the
            // lifetimes would clash.
            // self.store.get_item(coords)
        }

        #[inline]
        fn unwrap(&mut self, section: usize) -> SharedConst<'a, T> {
            SharedConst::new(self.store.get_data_ptr(section))
        }

        #[inline]
        fn null() -> SharedConst<'a, T> {
            SharedConst::new(ptr::null())
        }
    }

    impl<'a, T: 'a> Query for WriteQuery<'a, T> {
        type DataPtr = SharedMut<'a, T>;
        type Item = &'a mut T;

        #[inline]
        fn len(&self, section: usize) -> usize {
            self.store.section_len(section)
        }

        #[inline]
        fn get_by_coords(&self, coords: ComponentCoords) -> &'a mut T {
            let (section, loc) = coords;
            let ptr = self.store.get_data_ptr(section);
            unsafe {
                &mut *(ptr.add(loc) as *mut _)
            }
            // Can't do the safe thing here due to lack of generic associated types as the
            // lifetimes would clash.
            // self.store.get_item_mut(coords)
        }

        #[inline]
        fn unwrap(&mut self, section: usize) -> SharedMut<'a, T> {
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
    type ItemTup;
    type Indexer;

    fn get_ptr_tup(&mut self, idx: &Self::Indexer) -> (usize, Self::PtrTup);
    fn get_entity(&self, entity: &Entity, components: &Vec<ComponentId>) -> Self::ItemTup;
    unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
}

pub trait SystemDef {
    type JoinItem: Joined;

    fn as_joined(&self) -> Self::JoinItem;
    fn get_comp_ids() -> Vec<ComponentId>;
}

pub mod join {
    use super::{ComponentId, Entity, Indexable, IndexablePtrTup, Joined, Query, Store, SystemDef};

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
                fn get_ptr_tup(&mut self, idx: &($(_decl_system_replace_expr!($field_type usize)),*,)) -> (usize, ($($field_type::DataPtr),*,)) {
                    (self.0.len(idx.0), ($(self.$field_seq.unwrap(idx.$field_seq)),*,))
                }

                #[inline]
                fn get_entity(&self, entity: &Entity, components: &Vec<ComponentId>) -> ($($field_type::Item),*,) {
                    ($(self.$field_seq.get_by_coords(entity.get_coords(&components[$field_seq]))),*,)
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

//                #[inline]
//                fn reify(bundle: &Vec<>) -> ($($field_type),*,) {
//                    match bundle.len() {
//                        $field_count => ($(bundle[$field_seq].into()),*,),
//                        len => panic!("Recieved bundle rank {}, expected {}", len, $field_count),
//                    }
//                }

                #[inline]
                fn get_comp_ids() -> Vec<ComponentId> {
                    vec![$(ComponentId::new::<$field_type::DataType>()),*]
                }
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

pub mod context {
    use super::{BundleId, ComponentId, Entity, EntityId, HashMap, IndexMap, IndexablePtrTup, Joined};
    use crate::sync::ReadGuard;
    use indexmap::map::Values;

    pub struct Context<'a, T>
    where
        T: Joined,
    {
        stores: T,
        bundles: &'a IndexMap<BundleId, T::Indexer>,
        entity_map: ReadGuard<HashMap<EntityId, Entity>>,
        components: &'a Vec<ComponentId>,
    }

    impl<'a, T> Context<'a, T>
    where
        T: Joined,
    {
        #[inline]
        pub fn new(
            stores: T,
            bundles: &'a IndexMap<BundleId, T::Indexer>,
            entity_map: ReadGuard<HashMap<EntityId, Entity>>,
            components: &'a Vec<ComponentId>,
        ) -> Context<'a, T> {
            Context {
                stores,
                bundles,
                entity_map,
                components,
            }
        }

        #[inline]
        pub fn get_entity(&mut self, id: EntityId) -> Option<T::ItemTup> {
            if let Some(entity) = self.entity_map.get(&id) {
                Some(self.stores.get_entity(&entity, self.components))
            } else {
                None
            }
        }
    }

    impl<'a, T> IntoIterator for Context<'a, T>
    where
        T: 'a + Joined,
    {
        type Item = <T::PtrTup as IndexablePtrTup>::ItemTup;
        type IntoIter = ComponentIterator<'a, T>;

        #[inline]
        fn into_iter(mut self) -> ComponentIterator<'a, T> {
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

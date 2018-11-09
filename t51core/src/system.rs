use crate::component;
use crate::component::{ComponentCoords, ComponentStore};
use crate::entity::{Entity, EntityStore, Transaction};
use crate::object::{BundleId, ComponentId, EntityId};
use crate::registry::Registry;
use crate::sync::RwCell;
use hashbrown::HashMap;
use indexmap::IndexMap;
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

    fn run(&mut self, data: context::Context<<Self::Data as SystemDef>::JoinItem>, entities: EntityStore);
}

pub trait SystemRuntime {
    fn run(&mut self, entity_map: &HashMap<EntityId, Entity>);
    fn add_bundle(&mut self, bundle: &component::Bundle);
    fn remove_bundle(&mut self, id: BundleId);
    fn get_required_components(&self) -> &Vec<ComponentId>;
}

pub struct SystemData<T>
where
    T: SystemDef,
{
    stores: T,
    bundles: IndexMap<BundleId, <T::JoinItem as Joined>::Indexer>,
    components: Vec<ComponentId>,
}

impl<T> SystemData<T>
where
    T: SystemDef,
{
    #[inline]
    pub(crate) fn new(components: &Registry<ComponentId>) -> SystemData<T> {
        SystemData {
            stores: T::new(components),
            bundles: IndexMap::new(),
            components: T::get_comp_ids()
        }
    }

    #[inline]
    pub fn context<'a, 'b>(&'a self, entity_map: &'b HashMap<EntityId, Entity>) -> context::Context<<T as SystemDef>::JoinItem>
    where
        'b: 'a,
    {
        context::Context::new(self.stores.as_joined(), &self.bundles, entity_map, &self.components)
    }

    #[inline]
    fn add_bundle(&mut self, bundle: &component::Bundle) {
        self.bundles.insert(bundle.id, T::reify_bundle(&self.components, bundle));
    }

    #[inline]
    fn remove_bundle(&mut self, id: BundleId) {
        self.bundles.remove(&id);
    }
}

pub struct SystemEntry<T>
where
    T: System,
{
    system: T,
    data: SystemData<T::Data>,
    transactions: Vec<Transaction>,
}

impl<T> SystemEntry<T>
where
    T: System,
{
    #[inline]
    pub(crate) fn new(system: T, components: &Registry<ComponentId>) -> SystemEntry<T> {
        SystemEntry {
            system,
            data: SystemData::new(components),
            transactions: Vec::new()
        }
    }
}

impl<T> SystemRuntime for SystemEntry<T>
where
    T: System,
{
    #[inline]
    fn run(&mut self, entity_map: &HashMap<EntityId, Entity>) {
        self.system.run(
            self.data.context(entity_map),
            EntityStore::new(entity_map, &mut self.transactions),
        );
    }

    #[inline]
    fn add_bundle(&mut self, bundle: &component::Bundle) {
        self.data.add_bundle(bundle);
    }

    #[inline]
    fn remove_bundle(&mut self, id: BundleId) {
        self.data.remove_bundle(id);
    }

    #[inline]
    fn get_required_components(&self) -> &Vec<ComponentId> {
        &self.data.components
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

    fn new(store: Arc<RwCell<ComponentStore<Self::DataType>>>) -> Self;
    fn get_query(&self) -> Self::QueryItem;
}

pub trait IndexablePtrTup {
    type ItemTup;

    fn index(&self, idx: usize) -> Self::ItemTup;
}

pub mod store {
    use super::{Arc, ComponentCoords, ComponentStore, Indexable, Query, RwCell, Store};
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
        store: *const ComponentStore<T>,
        _x: PhantomData<&'a T>,
    }

    #[repr(transparent)]
    pub struct WriteQuery<'a, T> {
        store: *mut ComponentStore<T>,
        _x: PhantomData<&'a T>,
    }

    impl<'a, T> ReadQuery<'a, T> {
        #[inline]
        fn new(store: &RwCell<ComponentStore<T>>) -> ReadQuery<'a, T> {
            // No need for explicit guards as the scheduler guarantees to maintain the reference aliasing invariants.
            unsafe {
                ReadQuery {
                    store: store.get_ptr_raw(),
                    _x: PhantomData,
                }
            }
        }

        #[inline]
        fn store_ref(&self) -> &ComponentStore<T> {
            unsafe { &*self.store }
        }
    }

    impl<'a, T> WriteQuery<'a, T> {
        #[inline]
        fn new(store: &RwCell<ComponentStore<T>>) -> WriteQuery<'a, T> {
            // No need for explicit guards as the scheduler guarantees to maintain the reference aliasing invariants.
            unsafe {
                WriteQuery {
                    store: store.get_ptr_raw(),
                    _x: PhantomData,
                }
            }
        }

        #[inline]
        fn store_ref(&self) -> &ComponentStore<T> {
            unsafe { &*self.store }
        }

        #[inline]
        fn store_mut_ref(&mut self) -> &mut ComponentStore<T> {
            unsafe { &mut *self.store }
        }
    }

    impl<'a, T: 'a> Query for ReadQuery<'a, T> {
        type DataPtr = SharedConst<'a, T>;
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
        fn unwrap(&mut self, section: usize) -> SharedConst<'a, T> {
            SharedConst::new(self.store_ref().get_data_ptr(section))
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
        fn unwrap(&mut self, section: usize) -> SharedMut<'a, T> {
            SharedMut::new(self.store_mut_ref().get_data_mut_ptr(section))
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
        fn new(store: Arc<RwCell<ComponentStore<T>>>) -> Self {
            Read { store, _x: PhantomData }
        }

        #[inline]
        fn get_query(&self) -> ReadQuery<'a, T> {
            ReadQuery::new(&self.store)
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
        fn new(store: Arc<RwCell<ComponentStore<T>>>) -> Self {
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
    fn reify_bundle(sys_comps: &Vec<ComponentId>, bundle: &component::Bundle) -> <Self::JoinItem as Joined>::Indexer;
    fn get_comp_ids() -> Vec<ComponentId>;
    fn new(components: &Registry<ComponentId>) -> Self;
}

pub mod join {
    use super::{ComponentId, ComponentStore, Entity, Indexable, IndexablePtrTup, Joined, Query, Registry, Store, SystemDef};
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
                $($field_type: Store),*,
                $($field_type::DataType: 'static),*
            {
                type JoinItem = ($($field_type::QueryItem),*,);

                #[inline]
                fn as_joined(&self) -> ($($field_type::QueryItem),*,) {
                    ($(self.$field_seq.get_query()),*,)
                }

                #[inline]
                fn reify_bundle(sys_comps: &Vec<ComponentId>,
                                bundle: &component::Bundle) -> <Self::JoinItem as Joined>::Indexer {
                    ($(bundle.get_loc(sys_comps[$field_seq])),*,)
                }

                #[inline]
                fn get_comp_ids() -> Vec<ComponentId> {
                    vec![$(ComponentId::new::<$field_type::DataType>()),*]
                }

                #[inline]
                fn new(components: &Registry<ComponentId>) -> Self {
                    let comp_ids = Self::get_comp_ids();
                    ($($field_type::new(components.get::<ComponentStore<$field_type::DataType>>(&comp_ids[$field_seq]))),*,)
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
    use super::{BundleId, ComponentId, Entity, EntityId, HashMap, IndexMap, IndexablePtrTup, Joined};
    use indexmap::map::Values;

    pub struct Context<'a, T>
    where
        T: Joined,
    {
        stores: T,
        bundles: &'a IndexMap<BundleId, T::Indexer>,
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
            bundles: &'a IndexMap<BundleId, T::Indexer>,
            entity_map: &'a HashMap<EntityId, Entity>,
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

        #[inline]
        pub fn iter(&mut self) -> ComponentIterator<T> {
            let mut stream = self.bundles.values();

            unsafe {
                let (size, bundle) = match stream.next() {
                    Some(item) => self.stores.get_ptr_tup(item),
                    _ => (0usize, T::get_zero_ptr_tup()),
                };

                ComponentIterator {
                    stores: &mut self.stores,
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
        stores: &'a mut T,
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

use crate::entity::{Entity, EntityStore};
use crate::object::{ComponentId, EntityId, SystemId};
use crate::registry::Registry;
use crate::sync::{MultiBorrow, MultiLock};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::ptr::NonNull;

pub trait System {
    fn run(&mut self, entities: EntityStore);

    #[allow(unused_variables)]
    fn init(&mut self, components: &Registry<ComponentId>, systems: &Registry<SystemId>) {}
    #[allow(unused_variables)]
    fn entity_added(&mut self, entity: &Entity) {}
    #[allow(unused_variables)]
    fn entity_removed(&mut self, id: EntityId) {}
}

// TODO: Implement in macro
pub trait ManagedSystem: System {
    fn add_entity(&mut self, entity: &Entity);
    fn remove_entity(&mut self, id: EntityId);
}

// TODO: Implement in macro
pub trait BuildableSystem: ManagedSystem {
    fn new(components: &Registry<ComponentId>) -> Self;
    fn required_components() -> Vec<ComponentId>;
}

pub trait Indexable {
    type Item;

    fn index(&self, idx: usize) -> Self::Item;
}

pub trait Query {
    type DataPtr: Indexable;

    fn len(&self) -> usize;
    fn unwrap(&self) -> Self::DataPtr;
    fn null() -> Self::DataPtr;
}

pub trait IndexablePtrTup {
    type ItemTup;

    fn index(&self, idx: usize) -> Self::ItemTup;
}

pub mod runtime {
    use super::support::{BundleDef, Context, DataBundle};
    use super::{HashMap, IndexMap, Joined, MultiLock, EntityStore};
    use crate::object::{BundleId, EntityId};

    pub trait System {
        type Data: Joined;

        fn run(&mut self, data: Context<Self::Data>, entities: EntityStore);
    }

    pub struct SystemData<T>
    where
        T: Joined,
    {
        bundles: IndexMap<BundleId, DataBundle<T>>,
        entity_map: HashMap<EntityId, BundleId>,
        lock: MultiLock,
    }

    impl<T> SystemData<T>
    where
        T: Joined,
    {
        pub fn context(&self) -> Context<T> {
            Context::new(&self.bundles, &self.entity_map, self.lock.acquire())
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
        fn add_entity(&mut self, id: EntityId, bundle_id: BundleId);
        fn remove_entity(&mut self, id: EntityId);
        fn update_entity_bundle(&mut self, id: EntityId, bundle_id: BundleId);
        fn add_bundle(&mut self, bundle: BundleDef);
        fn remove_bundle(&mut self, id: BundleId);
    }

    impl<T> SystemRuntime for SystemEntry<T>
    where
        T: System,
    {
        #[inline]
        fn run(&mut self, entities: EntityStore) {
            self.system.run(self.data.context(), entities);
        }

        #[inline]
        fn add_entity(&mut self, id: EntityId, bundle_id: BundleId) {
            self.data.entity_map.insert(id, bundle_id);
        }

        #[inline]
        fn remove_entity(&mut self, id: EntityId) {
            self.data.entity_map.remove(&id);
        }

        #[inline]
        fn update_entity_bundle(&mut self, id: EntityId, bundle_id: BundleId) {
            self.data.entity_map.insert(id, bundle_id);
        }

        #[inline]
        fn add_bundle(&mut self, bundle: BundleDef) {
            let data_bundle = DataBundle::new(bundle);
            self.data.bundles.insert(data_bundle.bundle_id(), data_bundle);
        }

        #[inline]
        fn remove_bundle(&mut self, id: BundleId) {
            self.data.bundles.remove(&id);
        }
    }
}

pub mod store {
    use super::{Indexable, Query};
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
    pub struct Read<'a, T> {
        ptr: SharedConst<'a, Vec<T>>,
    }

    #[repr(transparent)]
    pub struct Write<'a, T> {
        ptr: SharedMut<'a, Vec<T>>,
    }

    impl<'a, T> Read<'a, T> {
        #[inline]
        fn new(ptr: *const ()) -> Read<'a, T> {
            Read {
                ptr: SharedConst::new(ptr as *const Vec<T>),
            }
        }

        #[inline]
        fn deref_ptr(&self) -> &'a Vec<T> {
            unsafe { &*(self.ptr).0 }
        }
    }

    impl<'a, T> Write<'a, T> {
        #[inline]
        fn new(ptr: *const ()) -> Write<'a, T> {
            Write {
                ptr: SharedMut::new(ptr as *mut Vec<T>),
            }
        }

        #[inline]
        fn deref_ptr(&self) -> &'a mut Vec<T> {
            unsafe { &mut *(self.ptr).0 }
        }
    }

    impl<'a, T> From<ptr::NonNull<()>> for Read<'a, T> {
        #[inline]
        fn from(ptr: ptr::NonNull<()>) -> Self {
            Read::new(ptr.as_ptr())
        }
    }

    impl<'a, T> From<ptr::NonNull<()>> for Write<'a, T> {
        #[inline]
        fn from(ptr: ptr::NonNull<()>) -> Self {
            Write::new(ptr.as_ptr())
        }
    }

    impl<'a, T: 'a> Indexable for Read<'a, T> {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            &self.deref_ptr()[idx]
        }
    }

    impl<'a, T: 'a> Indexable for Write<'a, T> {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            &mut (self.deref_ptr())[idx]
        }
    }

    impl<'a, T: 'a> Query for Read<'a, T> {
        type DataPtr = SharedConst<'a, T>;

        #[inline]
        fn len(&self) -> usize {
            self.deref_ptr().len()
        }

        #[inline]
        fn unwrap(&self) -> SharedConst<'a, T> {
            SharedConst::new(self.deref_ptr().as_ptr())
        }

        #[inline]
        fn null() -> SharedConst<'a, T> {
            SharedConst::new(ptr::null())
        }
    }

    impl<'a, T: 'a> Query for Write<'a, T> {
        type DataPtr = SharedMut<'a, T>;

        #[inline]
        fn len(&self) -> usize {
            self.deref_ptr().len()
        }

        #[inline]
        fn unwrap(&self) -> SharedMut<'a, T> {
            SharedMut::new(self.deref_ptr().as_mut_ptr())
        }

        #[inline]
        fn null() -> SharedMut<'a, T> {
            SharedMut::new(ptr::null_mut())
        }
    }
}

pub trait Joined {
    type ItemTup;
    type PtrTup: IndexablePtrTup;

    fn reify(bundle: &Vec<NonNull<()>>) -> Self;
    fn len(&self) -> usize;
    fn get_by_index(&self, idx: usize) -> Self::ItemTup;
    fn get_ptr_tup(&self) -> Self::PtrTup;
    unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
}

pub mod join {
    use super::{Indexable, IndexablePtrTup, Joined, Query};
    use std::ptr::NonNull;

    /// To macro_rules!
    impl<A, B, C> IndexablePtrTup for (A, B, C)
    where
        A: Indexable,
        B: Indexable,
        C: Indexable,
    {
        type ItemTup = (A::Item, B::Item, C::Item);

        #[inline]
        fn index(&self, idx: usize) -> (A::Item, B::Item, C::Item) {
            (self.0.index(idx), self.1.index(idx), self.2.index(idx))
        }
    }

    /// To macro_rules!
    impl<A, B, C> Joined for (A, B, C)
    where
        A: Query + Indexable + From<NonNull<()>>,
        B: Query + Indexable + From<NonNull<()>>,
        C: Query + Indexable + From<NonNull<()>>,
    {
        type ItemTup = (A::Item, B::Item, C::Item);
        type PtrTup = (A::DataPtr, B::DataPtr, C::DataPtr);

        #[inline]
        fn reify(bundle: &Vec<NonNull<()>>) -> (A, B, C) {
            match bundle.len() {
                3 => (bundle[0].into(), bundle[1].into(), bundle[2].into()),
                len => panic!("Recieved bundle rank {}, expected {}", len, 3),
            }
        }

        #[inline]
        fn len(&self) -> usize {
            self.0.len()
        }

        #[inline]
        fn get_by_index(&self, idx: usize) -> (A::Item, B::Item, C::Item) {
            (self.0.index(idx), self.1.index(idx), self.2.index(idx))
        }

        #[inline]
        fn get_ptr_tup(&self) -> (A::DataPtr, B::DataPtr, C::DataPtr) {
            (self.0.unwrap(), self.1.unwrap(), self.2.unwrap())
        }

        #[inline]
        unsafe fn get_zero_ptr_tup() -> (A::DataPtr, B::DataPtr, C::DataPtr) {
            (A::null(), B::null(), C::null())
        }
    }
}

pub mod support {
    use super::{HashMap, IndexMap, IndexablePtrTup, Joined, MultiBorrow, NonNull};
    use crate::object::{BundleId, EntityId};
    use indexmap::map::Values;

    pub struct BundleDef(BundleId, NonNull<HashMap<EntityId, usize>>, Vec<NonNull<()>>);

    pub struct DataBundle<T>
    where
        T: Joined,
    {
        id: BundleId,
        entities: NonNull<HashMap<EntityId, usize>>,
        data: T,
    }

    impl<T> DataBundle<T>
    where
        T: Joined,
    {
        #[inline]
        pub fn new(bundle: BundleDef) -> DataBundle<T> {
            DataBundle {
                id: bundle.0,
                entities: bundle.1,
                data: T::reify(&bundle.2),
            }
        }

        #[inline]
        pub fn bundle_id(&self) -> BundleId {
            self.id
        }

        #[inline]
        pub fn len(&self) -> usize {
            self.data.len()
        }

        #[inline]
        pub fn get_by_id(&self, id: EntityId) -> T::ItemTup {
            unsafe {
                let index = self.entities.as_ref()[&id];
                self.data.get_by_index(index)
            }
        }

        #[inline]
        pub fn get_ptr_tup(&self) -> T::PtrTup {
            self.data.get_ptr_tup()
        }
    }

    pub struct Context<'a, T>
    where
        T: Joined,
    {
        bundles: &'a IndexMap<BundleId, DataBundle<T>>,
        entity_map: &'a HashMap<EntityId, BundleId>,
        _borrow: MultiBorrow,
    }

    impl<'a, T> Context<'a, T>
    where
        T: Joined,
    {
        #[inline]
        pub fn new(
            bundles: &'a IndexMap<BundleId, DataBundle<T>>,
            entity_map: &'a HashMap<EntityId, BundleId>,
            _borrow: MultiBorrow,
        ) -> Context<'a, T> {
            Context {
                bundles,
                entity_map,
                _borrow,
            }
        }

        #[inline]
        pub fn get_by_id(&mut self, id: EntityId) -> T::ItemTup {
            let bundle_id = self.entity_map[&id];
            let bundle = &self.bundles[&bundle_id];
            bundle.get_by_id(id)
        }

        #[inline]
        pub fn iter(&self) -> ComponentIterator<T> {
            let mut stream = self.bundles.values();

            unsafe {
                let (size, bundle) = match stream.next() {
                    Some(item) => (item.len(), item.get_ptr_tup()),
                    _ => (0usize, T::get_zero_ptr_tup()),
                };

                ComponentIterator {
                    stream,
                    bundle,
                    size,
                    counter: 0,
                    _borrow: &self._borrow,
                }
            }
        }
    }

    pub struct ComponentIterator<'a, T>
    where
        T: Joined,
    {
        stream: Values<'a, BundleId, DataBundle<T>>,
        bundle: T::PtrTup,
        size: usize,
        counter: usize,
        _borrow: &'a MultiBorrow,
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

                if let Some(new_bundle) = self.stream.next() {
                    self.bundle = new_bundle.get_ptr_tup();
                    self.size = new_bundle.len();
                    self.counter = 0;
                } else {
                    return None;
                }
            }
        }
    }
}

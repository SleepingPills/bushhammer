#![allow(unused_imports, dead_code, unused_variables)]
#![feature(nll)]
#![feature(specialization)]
//use t51core::system::SystemData;
//use t51core_proc::{make_system};
use std::collections::HashMap;
use t51core::entity::EntityId;

//#[make_system]
//struct MySys<'a> {
//    data: SystemData<(EntityId, &'a i32, &'a u64, &'a mut u64)>,
//    plod: i32,
//    glod: &'a str,
//}
//
//fn test(sys: &mut MySys) {
//    let ctx = sys.data.get_ctx();
//    for (id, a, b, c) in ctx.iter() {
//        sys.plod = 4;
//    };
//}

fn main() {}

pub mod refactor {
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::rc::Rc;
    use std::slice::Iter;
    use t51core::entity::EntityId;

    type BundleId = usize;

    /* ########## MultiLock ########## */
    pub struct MultiLock;

    impl MultiLock {
        pub fn acquire(&self) -> Borrow {
            unimplemented!()
        }
    }

    pub struct Borrow;

    pub trait Indexed<'a> {
        type Item;

        fn index(&self, idx: usize) -> Self::Item;
    }

    pub struct Reader<'a, T>(*const Vec<T>, PhantomData<&'a ()>);

    pub struct Writer<'a, T>(*mut Vec<T>, PhantomData<&'a ()>);

    impl<'a, T> Reader<'a, T> {
        #[inline]
        fn deref_ptr(&self) -> &'a Vec<T> {
            unsafe { &*self.0 }
        }
    }

    impl<'a, T> Writer<'a, T> {
        #[inline]
        fn deref_ptr(&self) -> &'a mut Vec<T> {
            unsafe { &mut *self.0 }
        }
    }

    impl<'a, T> From<*const ()> for Reader<'a, T> {
        #[inline]
        fn from(ptr: *const ()) -> Self {
            Reader(ptr as *const Vec<T>, PhantomData)
        }
    }

    impl<'a, T> From<*const ()> for Writer<'a, T> {
        #[inline]
        fn from(ptr: *const ()) -> Self {
            Writer(ptr as *mut Vec<T>, PhantomData)
        }
    }

    impl<'a, T: 'a> Indexed<'a> for Reader<'a, T> {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            &self.deref_ptr()[idx]
        }
    }

    impl<'a, T: 'a> Indexed<'a> for Writer<'a, T> {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            &mut (self.deref_ptr())[idx]
        }
    }

    pub trait Query<'a> {
        type DataPtr: Indexed<'a> + Copy;

        fn len(&self) -> usize;
        fn unwrap(&self) -> Self::DataPtr;
    }

    impl<'a, T: 'a> Query<'a> for Reader<'a, T> {
        type DataPtr = *const T;

        #[inline]
        fn len(&self) -> usize {
            self.deref_ptr().len()
        }

        #[inline]
        fn unwrap(&self) -> *const T {
            self.deref_ptr().as_ptr()
        }
    }

    impl<'a, T: 'a> Query<'a> for Writer<'a, T> {
        type DataPtr = *mut T;

        #[inline]
        fn len(&self) -> usize {
            self.deref_ptr().len()
        }

        #[inline]
        fn unwrap(&self) -> *mut T {
            self.deref_ptr().as_mut_ptr()
        }
    }

    impl<'a, T: 'a> Indexed<'a> for *const T {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            unsafe { &*self.add(idx) }
        }
    }

    impl<'a, T: 'a> Indexed<'a> for *mut T {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            unsafe { &mut *self.add(idx) }
        }
    }

    trait Bundled<'a, T> {
        fn reify(bundle: &Vec<*const ()>) -> T;
        fn get_ctx(&self) -> Context<T>;
    }

    pub struct SystemCore<'a, T> {
        lock: MultiLock,
        bundles: Vec<T>,
        entities: HashMap<EntityId, (BundleId, usize)>,
        bundle_map: HashMap<BundleId, usize>,
        _x: PhantomData<&'a ()>,
    }

    impl<'a, T> SystemCore<'a, T> {
        pub fn ingest(&mut self, id: BundleId, bundle: &Vec<*const ()>) {
            let bundle_loc = self.bundles.len();
            self.bundles.push(Self::reify(bundle));
            self.bundle_map.insert(id, bundle_loc);
        }
    }

    impl<'a, T> Bundled<'a, T> for SystemCore<'a, T> {
        default fn reify(bundle: &Vec<*const ()>) -> T {
            panic!("System bundle must be a tuple of components")
        }

        default fn get_ctx(&self) -> Context<T> {
            panic!("System bundle must be a tuple of components")
        }
    }

    impl<'a, A, B> Bundled<'a, (A, B)> for SystemCore<'a, (A, B)>
    where
        A: From<*const ()>,
        B: From<*const ()>,
    {
        #[inline]
        fn reify(bundle: &Vec<*const ()>) -> (A, B) {
            match bundle.len() {
                2 => (bundle[0].into(), bundle[1].into()),
                len => panic!("Recieved bundle rank {}, expected {}", len, 2),
            }
        }

        #[inline]
        fn get_ctx(&self) -> Context<(A, B)> {
            Context {
                bundles: &self.bundles,
                entities: &self.entities,
                bundle_map: &self.bundle_map,
                _borrow: Rc::new(self.lock.acquire()),
            }
        }
    }

    trait BundleContext<'a, T> {
        type Item;
        type PtrTup;

        fn iter(&self) -> BundleIterator<T, Self::PtrTup>;
        fn get_by_id(&self, id: EntityId) -> Self::Item;
    }

    pub struct Context<'a, T> {
        bundles: &'a Vec<T>,
        entities: &'a HashMap<EntityId, (BundleId, usize)>,
        bundle_map: &'a HashMap<BundleId, usize>,
        _borrow: Rc<Borrow>,
    }

    impl<'a, A, B> BundleContext<'a, (A, B)> for Context<'a, (A, B)>
    where
        A: Query<'a> + Indexed<'a>,
        B: Query<'a> + Indexed<'a>,
    {
        type Item = (A::Item, B::Item);
        type PtrTup = (A::DataPtr, B::DataPtr);

        #[inline]
        fn iter(&self) -> BundleIterator<(A, B), Self::PtrTup> {
            let mut iter = self.bundles.iter();

            let (size, cache) = match iter.next() {
                Some((a, b)) => (a.len(), (a.unwrap(), b.unwrap())),
                _ => panic!("Boo"), // TODO: (0usize, (0 as A::DataPtr, 0 as B::DataPtr))
            };

            BundleIterator {
                bundles: iter,
                size,
                counter: 0,
                cache,
                _borrow: self._borrow.clone(),
            }
        }

        #[inline]
        fn get_by_id(&self, id: usize) -> (A::Item, B::Item) {
            let (bundle_id, entity_index) = self.entities[&id];
            let bundle_index = self.bundle_map[&bundle_id];
            let (a, b) = &self.bundles[bundle_index];
            (a.index(id), b.index(id))
        }
    }

    pub struct BundleIterator<'a, T, I> {
        bundles: Iter<'a, T>,
        size: usize,
        counter: usize,
        cache: I,
        _borrow: Rc<Borrow>,
    }

    impl<'a, A, B> Iterator for BundleIterator<'a, (A, B), (A::DataPtr, B::DataPtr)>
    where
        A: Query<'a>,
        B: Query<'a>,
    {
        type Item = (
            <<A as Query<'a>>::DataPtr as Indexed<'a>>::Item,
            <<B as Query<'a>>::DataPtr as Indexed<'a>>::Item,
        );

        fn next(
            &mut self,
        ) -> Option<(
            <<A as Query<'a>>::DataPtr as Indexed<'a>>::Item,
            <<B as Query<'a>>::DataPtr as Indexed<'a>>::Item,
        )> {
            loop {
                if self.counter < self.size {
                    let (a, b) = self.cache;
                    let result = Some((a.index(self.counter), b.index(self.counter)));
                    self.counter += 1;
                    return result;
                }

                if let Some((a, b)) = self.bundles.next() {
                    self.size = a.len();
                    self.cache = (a.unwrap(), b.unwrap())
                } else {
                    return None;
                }
            }
        }
    }

    fn goop(core: SystemCore<(Reader<i32>, Writer<u64>)>) {
        let x = Vec::<i32>::new().iter();
        let ctx: Context<(Reader<i32>, Writer<u64>)> = core.get_ctx();

        for (a, b) in ctx.iter() {
            let (c, d) = ctx.get_by_id(5);
        }
    }
}

pub mod refactor0 {
    use indexmap::IndexMap;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::ptr;
    use std::ptr::NonNull;
    use std::rc::Rc;
    use std::slice::Iter;
    use t51core::entity::Entity;
    use t51core::entity::EntityId;
    use indexmap::map::Values;

    type BundleId = usize;

    /* ########## MultiLock ########## */
    pub struct MultiLock;

    impl MultiLock {
        pub fn acquire(&self) -> Borrow {
            unimplemented!()
        }
    }

    pub struct Borrow;

    pub trait Indexable {
        type Item;

        fn index(&self, idx: usize) -> Self::Item;
    }

    #[repr(transparent)]
    pub struct Reader<'a, T> {
        ptr: SharedConst<'a, Vec<T>>,
    }

    #[repr(transparent)]
    pub struct Writer<'a, T> {
        ptr: SharedMut<'a, Vec<T>>,
    }

    impl<'a, T> Reader<'a, T> {
        #[inline]
        fn new(ptr: *const ()) -> Reader<'a, T> {
            Reader {
                ptr: SharedConst::new(ptr as *const Vec<T>),
            }
        }

        #[inline]
        fn deref_ptr(&self) -> &'a Vec<T> {
            unsafe { &*(self.ptr).0 }
        }
    }

    impl<'a, T> Writer<'a, T> {
        #[inline]
        fn new(ptr: *const ()) -> Writer<'a, T> {
            Writer {
                ptr: SharedMut::new(ptr as *mut Vec<T>),
            }
        }

        #[inline]
        fn deref_ptr(&self) -> &'a mut Vec<T> {
            unsafe { &mut *(self.ptr).0 }
        }
    }

    impl<'a, T> From<NonNull<()>> for Reader<'a, T> {
        #[inline]
        fn from(ptr: NonNull<()>) -> Self {
            Reader::new(ptr.as_ptr())
        }
    }

    impl<'a, T> From<NonNull<()>> for Writer<'a, T> {
        #[inline]
        fn from(ptr: NonNull<()>) -> Self {
            Writer::new(ptr.as_ptr())
        }
    }

    impl<'a, T: 'a> Indexable for Reader<'a, T> {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            &self.deref_ptr()[idx]
        }
    }

    impl<'a, T: 'a> Indexable for Writer<'a, T> {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            &mut (self.deref_ptr())[idx]
        }
    }

    pub trait Query {
        type DataPtr: Indexable;

        fn len(&self) -> usize;
        fn unwrap(&self) -> Self::DataPtr;
        fn null() -> Self::DataPtr;
    }

    impl<'a, T: 'a> Query for Reader<'a, T> {
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

    impl<'a, T: 'a> Query for Writer<'a, T> {
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

    pub trait IndexablePtrTup {
        type ItemTup;

        fn index(&self, idx: usize) -> Self::ItemTup;
    }

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

    pub trait Joined {
        type ItemTup;
        type PtrTup: IndexablePtrTup;

        fn reify(bundle: &Vec<NonNull<()>>) -> Self;
        fn len(&self) -> usize;
        fn get_by_index(&self, idx: usize) -> Self::ItemTup;
        fn get_ptr_tup(&self) -> Self::PtrTup;
        unsafe fn get_zero_ptr_tup() -> Self::PtrTup;
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

    pub struct BundleIterator<T>
    where
        T: IndexablePtrTup,
    {
        size: usize,
        counter: usize,
        components: T,
    }

    pub struct BundleDef(BundleId, NonNull<HashMap<EntityId, usize>>, Vec<NonNull<()>>);

    pub struct DataBundle<T>
    where
        T: Joined,
    {
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
                entities: bundle.1,
                data: T::reify(&bundle.2),
            }
        }

        #[inline]
        fn len(&self) -> usize {
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

    pub struct SystemCore<T>
    where
        T: Joined,
    {
        bundles: IndexMap<BundleId, DataBundle<T>>,
        entity_map: HashMap<EntityId, BundleId>,
        lock: MultiLock,
    }

    impl<T> SystemCore<T>
    where
        T: Joined,
    {
        pub fn get_ctx(&self) -> Context<T> {
            Context {
                bundles: &self.bundles,
                entity_map: &self.entity_map,
                _borrow: Rc::new(self.lock.acquire()),
            }
        }

        #[inline]
        pub fn add_entity(&mut self, id: EntityId, bundle_id: BundleId) {
            self.entity_map.insert(id, bundle_id);
        }

        #[inline]
        pub fn remove_entity(&mut self, id: EntityId) {
            self.entity_map.remove(&id);
        }

        #[inline]
        pub fn update_entity_bundle(&mut self, id: EntityId, bundle_id: BundleId) {
            self.entity_map.insert(id, bundle_id);
        }

        #[inline]
        pub fn add_bundle(&mut self, bundle: BundleDef) {
            self.bundles.insert(bundle.0, DataBundle::new(bundle));
        }

        #[inline]
        pub fn remove_bundle(&mut self, id: BundleId) {
            self.bundles.remove(&id);
        }
    }

    pub struct Context<'a, T>
    where
        T: Joined,
    {
        bundles: &'a IndexMap<BundleId, DataBundle<T>>,
        entity_map: &'a HashMap<EntityId, BundleId>,
        _borrow: Rc<Borrow>,
    }

    impl<'a, T> Context<'a, T>
    where
        T: Joined,
    {
        #[inline]
        pub fn get_by_id(&self, id: EntityId) -> T::ItemTup {
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
                    _borrow: self._borrow.clone()
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
        _borrow: Rc<Borrow>
    }

    impl<'a, T> Iterator for ComponentIterator<'a, T> where T: Joined {
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
                    return None
                }
            }
        }
    }

    pub struct Goof<'a> {
        sys: SystemCore<(Reader<'a, EntityId>, Reader<'a, i32>, Writer<'a, u64>)>,
        coll: Vec<&'a i32>
    }

    impl<'a> Goof<'a> {
        fn moof(&mut self) {
            for (a, b, c) in self.sys.get_ctx().iter() {
                self.coll.push(b);
            }
        }
    }


    fn poof(sys: SystemCore<(Reader<EntityId>, Reader<i32>, Writer<u64>)>) {
        let ctx = sys.get_ctx();

        for (a, b, c) in ctx.iter() {
            *c = 5;
        }
    }
}

pub mod refactor2 {
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::ptr;
    use std::ptr::NonNull;
    use std::rc::Rc;
    use std::slice::Iter;
    use t51core::entity::Entity;
    use t51core::entity::EntityId;

    type BundleId = usize;

    /* ########## MultiLock ########## */
    pub struct MultiLock;

    impl MultiLock {
        pub fn acquire(&self) -> Borrow {
            unimplemented!()
        }
    }

    pub struct Borrow;

    pub trait Indexable<'a> {
        type Item;

        fn index(&self, idx: usize) -> Self::Item;
    }

    pub struct Reader<'a, T>(*const Vec<T>, PhantomData<&'a ()>);

    pub struct Writer<'a, T>(*mut Vec<T>, PhantomData<&'a ()>);

    impl<'a, T> Reader<'a, T> {
        #[inline]
        fn deref_ptr(&self) -> &'a Vec<T> {
            unsafe { &*self.0 }
        }
    }

    impl<'a, T> Writer<'a, T> {
        #[inline]
        fn deref_ptr(&self) -> &'a mut Vec<T> {
            unsafe { &mut *self.0 }
        }
    }

    impl<'a, T> From<*const ()> for Reader<'a, T> {
        #[inline]
        fn from(ptr: *const ()) -> Self {
            Reader(ptr as *const Vec<T>, PhantomData)
        }
    }

    impl<'a, T> From<*const ()> for Writer<'a, T> {
        #[inline]
        fn from(ptr: *const ()) -> Self {
            Writer(ptr as *mut Vec<T>, PhantomData)
        }
    }

    impl<'a, T: 'a> Indexable<'a> for Reader<'a, T> {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            &self.deref_ptr()[idx]
        }
    }

    impl<'a, T: 'a> Indexable<'a> for Writer<'a, T> {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            &mut (self.deref_ptr())[idx]
        }
    }

    pub trait Query<'a> {
        type DataPtr: Indexable<'a> + Copy;

        fn len(&self) -> usize;
        fn unwrap(&self) -> Self::DataPtr;
    }

    impl<'a, T: 'a> Query<'a> for Reader<'a, T> {
        type DataPtr = *const T;

        #[inline]
        fn len(&self) -> usize {
            self.deref_ptr().len()
        }

        #[inline]
        fn unwrap(&self) -> *const T {
            self.deref_ptr().as_ptr()
        }
    }

    impl<'a, T: 'a> Query<'a> for Writer<'a, T> {
        type DataPtr = *mut T;

        #[inline]
        fn len(&self) -> usize {
            self.deref_ptr().len()
        }

        #[inline]
        fn unwrap(&self) -> *mut T {
            self.deref_ptr().as_mut_ptr()
        }
    }

    impl<'a, T: 'a> Indexable<'a> for *const T {
        type Item = &'a T;

        #[inline]
        fn index(&self, idx: usize) -> &'a T {
            unsafe { &*self.add(idx) }
        }
    }

    impl<'a, T: 'a> Indexable<'a> for *mut T {
        type Item = &'a mut T;

        #[inline]
        fn index(&self, idx: usize) -> &'a mut T {
            unsafe { &mut *self.add(idx) }
        }
    }

    pub trait Joined<'a>: Sized {
        type ItemTup;

        fn reify(bundle: &Vec<*const ()>) -> Self;
        fn get_by_id(&self, idx: usize) -> Self::ItemTup;
    }

    /// To macro_rules!
    impl<'a, A, B, C> Joined<'a> for (A, B, C)
    where
        A: Indexable<'a> + From<*const ()>,
        B: Indexable<'a> + From<*const ()>,
        C: Indexable<'a> + From<*const ()>,
    {
        type ItemTup = (A::Item, B::Item, C::Item);

        #[inline]
        fn reify(bundle: &Vec<*const ()>) -> (A, B, C) {
            match bundle.len() {
                3 => (bundle[0].into(), bundle[1].into(), bundle[2].into()),
                len => panic!("Recieved bundle rank {}, expected {}", len, 3),
            }
        }

        #[inline]
        fn get_by_id(&self, idx: usize) -> (A::Item, B::Item, C::Item) {
            (self.0.index(idx), self.1.index(idx), self.2.index(idx))
        }
    }

    pub trait BundleVec<'a, T>
    where
        T: Joined<'a>,
    {
        fn reify(bundle: &Vec<*const ()>) -> T;
    }

    impl<'a, T> BundleVec<'a, T> for Vec<T>
    where
        T: Joined<'a>,
    {
        #[inline]
        fn reify(bundle: &Vec<*const ()>) -> T {
            T::reify(bundle)
        }
    }

    pub struct BundleDef {
        data: Vec<NonNull<()>>,
        entities: NonNull<HashMap<EntityId, usize>>,
    }

    pub struct DataBundle<'a, T>
    where
        T: Joined<'a>,
    {
        data: T,
        entities: NonNull<HashMap<EntityId, usize>>,
        _x: PhantomData<&'a ()>,
    }

    impl<'a, T> DataBundle<'a, T>
    where
        T: Joined<'a>,
    {
        pub fn get_entity_by_id(&self, id: EntityId) -> T::ItemTup {
            self.data.get_by_id(id)
        }
    }

    pub struct SystemData<'a> {
        bundles: Vec<(Reader<'a, EntityId>, Reader<'a, i32>, Writer<'a, u64>)>,
        entity_map: HashMap<EntityId, (BundleId, usize)>,
        bundle_map: HashMap<BundleId, usize>,
        lock: MultiLock,
    }

    /*
    If a systemdata specifies EntityId in its component list, it will simply become part of the component queries.
    This will work seamlessly, because BundleRegistries will always keep track of the entity id for each entry they keep.
    The entity id will be just another array in the bundle, same as the components.
    
    !!! Adjust the proc macro such that it tests that the user doesn't want writeable EntityIds, but there's no need
    to do anything else, the above method will just implicitly work. It fits into place neatly. !!!
    */
    impl<'a> SystemData<'a> {
        #[inline]
        pub fn ingest(&mut self, id: BundleId, bundle: Vec<*const ()>) {
            let bundle_loc = self.bundles.len();
            self.bundles.push(Vec::reify(&bundle));
        }

        pub fn activate(&mut self, id: BundleId) {}

        pub fn passivate(&mut self, id: BundleId) {
            let bundle_loc = self.bundle_map.remove(&id).expect("Bundle not found");
        }

        pub fn add_ent(&mut self, entity: &Entity) {}

        pub fn remove_ent(&mut self, id: EntityId, swapped_id: EntityId) {
            let coords = self.entity_map.remove(&id).expect("Entity missing");
            self.entity_map.insert(swapped_id, coords);
        }

        #[inline]
        pub fn get_ctx(&self) -> Context {
            Context {
                bundles: &self.bundles,
                entity_map: &self.entity_map,
                bundle_map: &self.bundle_map,
                _borrow: Rc::new(self.lock.acquire()),
            }
        }

        #[inline]
        pub fn iter(&self) -> BundleIterator {
            self.get_ctx().into_iter()
        }
    }

    pub struct Context<'a> {
        bundles: &'a Vec<(Reader<'a, EntityId>, Reader<'a, i32>, Writer<'a, u64>)>,
        entity_map: &'a HashMap<EntityId, (BundleId, usize)>,
        bundle_map: &'a HashMap<BundleId, usize>,
        _borrow: Rc<Borrow>,
    }

    impl<'a> Context<'a> {
        #[inline]
        pub unsafe fn get_by_id(&self, id: usize) -> (&EntityId, &i32, &mut u64) {
            let (bundle_id, entity_index) = self.entity_map[&id];
            let bundle_index = self.bundle_map[&bundle_id];
            let bundle = &self.bundles[bundle_index];
            bundle.get_by_id(entity_index)
        }

        #[inline]
        fn iter(&self) -> BundleIterator<'a> {
            let mut iter = self.bundles.iter();

            let (size, data_ptrs) = match iter.next() {
                Some((comp_0, comp_1, comp_2)) => (comp_0.len(), (comp_0.unwrap(), comp_1.unwrap(), comp_2.unwrap())),
                _ => (0usize, (ptr::null::<EntityId>(), ptr::null::<i32>(), ptr::null_mut::<u64>())),
            };

            BundleIterator {
                bundles: iter,
                size,
                counter: 0,
                ptr_0: data_ptrs.0,
                ptr_1: data_ptrs.1,
                ptr_2: data_ptrs.2,
                _borrow: self._borrow.clone(),
            }
        }
    }

    impl<'a> IntoIterator for Context<'a> {
        type Item = (&'a EntityId, &'a i32, &'a mut u64);
        type IntoIter = BundleIterator<'a>;

        #[inline]
        fn into_iter(self) -> BundleIterator<'a> {
            self.iter()
        }
    }

    pub struct BundleIterator<'a> {
        bundles: Iter<'a, (Reader<'a, EntityId>, Reader<'a, i32>, Writer<'a, u64>)>,
        size: usize,
        counter: usize,
        ptr_0: *const EntityId,
        ptr_1: *const i32,
        ptr_2: *mut u64,
        _borrow: Rc<Borrow>,
    }

    impl<'a> Iterator for BundleIterator<'a> {
        type Item = (&'a EntityId, &'a i32, &'a mut u64);

        #[inline]
        fn next(&mut self) -> Option<((&'a EntityId, &'a i32, &'a mut u64))> {
            loop {
                if self.counter < self.size {
                    let offset = self.counter;
                    self.counter += 1;
                    Some((self.ptr_0.index(offset), self.ptr_1.index(offset), self.ptr_2.index(offset)));
                }

                if let Some((comp_0, comp_1, comp_2)) = self.bundles.next() {
                    self.size = comp_0.len();
                    self.counter = 0;
                    self.ptr_0 = comp_0.unwrap();
                    self.ptr_1 = comp_1.unwrap();
                    self.ptr_2 = comp_2.unwrap();
                } else {
                    return None;
                }
            }
        }
    }
}

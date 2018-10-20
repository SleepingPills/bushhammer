use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use std::hash::Hash;

use anymap::AnyMap;
use indexmap::IndexMap;

use sync::RwCell;


pub struct Bundle {
    guard: Arc<AtomicI64>,
    mapping: AnyMap,
}

impl Bundle {
    #[inline]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.mapping.get::<T>()
    }

    #[inline]
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.mapping.insert(value);
    }
}

pub struct Registry<K> where K: Eq + Hash {
    pub(crate) data: IndexMap<K, Bundle>
}

impl<K> Registry<K> where K: Eq + Hash {
    pub fn get<T: 'static>(&self, key: &K) -> Option<Arc<RwCell<T>>> {
        if let Some(bundle) = self.data.get(key) {
            if let Some(item) = bundle.get::<Arc<RwCell<T>>>() {
                Some(item.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_trait<T: 'static>(&self, key: &K) -> Option<Arc<RwCell<Box<T>>>> {
        self.get::<Box<T>>(key)
    }

    pub fn register<T: 'static>(&mut self, key: K, value: T) {
        // Construct a new shared guard
        let guard = Arc::new(AtomicI64::new(0));
        // Construct new entry for the value
        let entry = Arc::new(RwCell::new(value, guard.clone()));
        // Construct type mapping
        let mut mapping: AnyMap = AnyMap::new();
        mapping.insert(entry);
        // Stash away new bundle
        self.data.insert(key, Bundle { guard, mapping });
    }

    pub fn register_trait<R: 'static, T: 'static, F>(&mut self, key: &K, casting: F)
        where F: Fn(Box<R>) -> Box<T> {
        let mut bundle = self.data.get_mut(key).unwrap();

        // Construct boxed trait object as a duplicate of the root.
        let trait_obj = {
            let root = bundle.get::<Arc<RwCell<R>>>().unwrap();

            // Duplicate the pointer to the root object into a new box. The casting function
            // will convert it into a trait object.
            unsafe {
                let ptr_root = root.get_ptr();
                casting(Box::new(ptr_root.read()))
            }
        };
        // Use the shared guard of the bundle.
        let guard = bundle.guard.clone();
        bundle.insert(RwCell::new(trait_obj, guard));
    }
}

struct A;

trait B {}

impl B for A {}


fn poo(registry: &mut Registry<i32>) {
    let mut a = A {};
    let b = &mut a as *mut A;
    unsafe {
        let c = Box::from_raw(b);
        let d = Box::from_raw(b);

        let e: Box<B> = c;
    }
}

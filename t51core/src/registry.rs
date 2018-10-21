use std::hash::Hash;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use anymap::AnyMap;
use indexmap::IndexMap;

use std::marker::Unsize;
use std::ops::Deref;
use std::ops::DerefMut;
use sync::{ReadGuard, RwCell, RwGuard};

/// Dynamically typed registry for shared ownership access to objects and traits they implement.
/// Vanilla trait objects in rust take full ownership of the underlying instance, making it
/// difficult for shared access to the various traits and direct methods of the object.
///
/// The registry also allows enumerating all registered instances implementing a specific trait.
///
/// Note: Traits have to be registered explicitly. The registry does not attempt to discover
/// traits an object implements.
pub struct Registry<K>
where
    K: Eq + Hash,
{
    data: IndexMap<K, Bundle>,
}

impl<K> Registry<K>
where
    K: Eq + Hash,
{
    pub fn new() -> Registry<K> {
        Registry::<K> {
            data: IndexMap::new(),
        }
    }

    /// Get the root object associated with the given key.
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

    /// Get a trait object associated with the given key. The trait has to be registered
    /// first. The registry does not attempt to discover all traits an object implements.
    pub fn get_trait<T: 'static>(&self, key: &K) -> Option<Arc<RwCell<WeakBox<T>>>>
    where
        T: 'static + ?Sized,
    {
        self.get::<WeakBox<T>>(key)
    }

    /// Register a new object under the given key.
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

    /// Register a new trait `T` for the root object `R` and the given key.
    pub fn register_trait<R, T>(&mut self, key: &K)
    where
        R: 'static + Unsize<T>,
        T: 'static + ?Sized,
    {
        let bundle = self.data.get_mut(key).unwrap();

        // Construct boxed trait object as a duplicate of the root
        let trait_obj = {
            let root = bundle.get::<Arc<RwCell<R>>>().unwrap();

            // Duplicate the pointer to the root object into a new box
            unsafe {
                // Extract root pointer
                let ptr_root = root.get_ptr();
                // Make new "unique" box
                let val = Box::new(ptr_root.read());
                // Create trait object
                let boxed: Box<T> = val;
                // Pass into WeakBox
                WeakBox::new(boxed)
            }
        };
        // Use the shared guard of the bundle and stash away the entry
        let guard = bundle.guard.clone();
        bundle.insert(Arc::new(RwCell::new(trait_obj, guard)));
    }

    /// Iterate over all registered instances with the supplied trait
    pub fn iter<T>(&self) -> impl Iterator<Item = (&K, ReadGuard<WeakBox<T>>)>
    where
        T: 'static + ?Sized,
    {
        self.data.iter().filter_map(
            |(key, bundle)| match bundle.get::<Arc<RwCell<WeakBox<T>>>>() {
                Some(item) => Some((key, item.read())),
                _ => None,
            },
        )
    }

    /// Mutably iterate over all registered instances with the supplied trait
    pub fn iter_mut<T>(&self) -> impl Iterator<Item = (&K, RwGuard<WeakBox<T>>)>
    where
        T: 'static + ?Sized,
    {
        self.data.iter().filter_map(
            |(key, bundle)| match bundle.get::<Arc<RwCell<WeakBox<T>>>>() {
                Some(item) => Some((key, item.write())),
                _ => None,
            },
        )
    }
}

/// Umbrella object for a single "instance" in a registry. The instance may be accessed
/// using the root object itself, or the various traits it implements that have been registered.
struct Bundle {
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

/// A wrapper around `Box<T>` that deliberately leaks the contents of the inner box.
/// Used as a crutch to avoid double free-ing memory pointed to by trait objects, as these
/// normally assume that they fully own both the data and the vtable. When used by the registry,
/// the leak is acceptable since items in the registry are never meant to be deleted and live
/// until the end of the program.
pub struct WeakBox<T: ?Sized> {
    item: Option<Box<T>>,
}

impl<T: ?Sized> WeakBox<T> {
    pub fn new(boxed: Box<T>) -> WeakBox<T> {
        WeakBox { item: Some(boxed) }
    }
}

impl<T: ?Sized> Deref for WeakBox<T> {
    type Target = Box<T>;

    fn deref(&self) -> &Box<T> {
        match self.item.as_ref() {
            Some(item) => item,
            _ => panic!("Attempted use after free"),
        }
    }
}

impl<T: ?Sized> DerefMut for WeakBox<T> {
    fn deref_mut(&mut self) -> &mut Box<T> {
        match self.item.as_mut() {
            Some(item) => item,
            _ => panic!("Attempted use after free"),
        }
    }
}

impl<T: ?Sized> Drop for WeakBox<T> {
    fn drop(&mut self) {
        // Leak the inner box since it does not actually own the data.
        if let Some(item) = self.item.take() {
            Box::leak(item);
        } else {
            panic!("Attempted double free")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Foo {
        x: i32,
    }

    impl Foo {
        fn get_x(&self) -> i32 {
            self.x
        }
    }

    trait FooTrait {
        fn get_x_times_two(&self) -> i32;
        fn add_one(&mut self);
    }

    impl FooTrait for Foo {
        fn get_x_times_two(&self) -> i32 {
            self.x * 2
        }

        fn add_one(&mut self) {
            self.x += 1
        }
    }

    #[test]
    fn test_register_root() {
        let mut registry = Registry::<i32>::new();
        registry.register(123, Foo { x: 2 });

        {
            let foo = registry.get::<Foo>(&123).unwrap().read();

            assert_eq!(foo.x, 2);
            assert_eq!(foo.get_x(), 2);

            // Ensure that the type is not available under another key
            assert!(registry.get::<Foo>(&5).is_none())
        }

        {
            let mut foo = registry.get::<Foo>(&123).unwrap().write();

            foo.x = 10;
            assert_eq!(foo.x, 10);
            assert_eq!(foo.get_x(), 10);
        }
    }

    #[test]
    fn register_trait() {
        let mut registry = Registry::<i32>::new();
        registry.register(123, Foo { x: 2 });
        registry.register_trait::<Foo, FooTrait>(&123);

        {
            let foo_trait = registry.get_trait::<FooTrait>(&123).unwrap().read();

            assert_eq!(foo_trait.get_x_times_two(), 4);

            // Ensure that the type is not available under another key
            assert!(registry.get_trait::<FooTrait>(&5).is_none())
        }

        {
            let mut foo_trait = registry.get_trait::<FooTrait>(&123).unwrap().write();

            foo_trait.add_one();

            assert_eq!(foo_trait.get_x_times_two(), 6);
        }
    }

    #[test]
    fn allow_multiple_readers() {
        let mut registry = Registry::<i32>::new();
        registry.register(123, Foo { x: 2 });

        let foo1 = registry.get::<Foo>(&123).unwrap().read();
        let foo2 = registry.get::<Foo>(&123).unwrap().read();
        let foo3 = registry.get::<Foo>(&123).unwrap().read();

        assert_eq!(foo1.x, 2);
        assert_eq!(foo2.x, 2);
        assert_eq!(foo3.x, 2);
    }

    #[test]
    #[should_panic(
        expected = "Attempted to acquire a write lock while another lock is already in effect"
    )]
    fn fail_read_write_conflict() {
        let mut registry = Registry::<i32>::new();
        registry.register(123, Foo { x: 2 });

        let _foo1 = registry.get::<Foo>(&123).unwrap().read();
        let _foo2 = registry.get::<Foo>(&123).unwrap().write();
    }

    #[test]
    #[should_panic(
        expected = "Attempted to acquire read lock when a write lock is already in effect"
    )]
    fn fail_write_read_conflict() {
        let mut registry = Registry::<i32>::new();
        registry.register(123, Foo { x: 2 });

        let _foo1 = registry.get::<Foo>(&123).unwrap().write();
        let _foo2 = registry.get::<Foo>(&123).unwrap().read();
    }

    #[test]
    #[should_panic(
        expected = "Attempted to acquire a write lock while another lock is already in effect"
    )]
    fn fail_write_write_conflict() {
        let mut registry = Registry::<i32>::new();
        registry.register(123, Foo { x: 2 });

        let _foo1 = registry.get::<Foo>(&123).unwrap().write();
        let _foo2 = registry.get::<Foo>(&123).unwrap().write();
    }

    #[test]
    fn iter_contents() {
        let mut registry = Registry::<i32>::new();

        // Populate the registry with instances and traits
        let ids = vec![1, 2, 3];
        for &id in ids.iter() {
            registry.register(id, Foo { x: id });
            registry.register_trait::<Foo, FooTrait>(&id);
        }

        // Add another instance without the trait
        registry.register(4, Foo { x: 4 });

        for (i, (&id, inst)) in registry.iter::<FooTrait>().enumerate() {
            assert_eq!(inst.get_x_times_two(), ids[i] * 2);
            assert_eq!(id, ids[i]);
        }
    }

    #[test]
    fn iter_mut_contents() {
        let mut registry = Registry::<i32>::new();

        // Populate the registry with instances and traits
        let ids = vec![1, 2, 3];
        for &id in ids.iter() {
            registry.register(id, Foo { x: id });
            registry.register_trait::<Foo, FooTrait>(&id);
        }

        // Add another instance without the trait
        registry.register(4, Foo { x: 4 });

        for (i, (&id, mut inst)) in registry.iter_mut::<FooTrait>().enumerate() {
            assert_eq!(inst.get_x_times_two(), ids[i] * 2);
            inst.add_one();
            assert_eq!(inst.get_x_times_two(), (ids[i] + 1) * 2);
            assert_eq!(id, ids[i]);
        }
    }
}

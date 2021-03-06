use crate::alloc::DynPtr;
use crate::entity::{EntityId, ShardDef};
use crate::alloc::{DynVec, DynVecOps};
use crate::identity::{ComponentClass, ShardKey};
use hashbrown::HashMap;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[macro_export]
macro_rules! component_init {
    ($name: ident) => {
        $crate::custom_type_id_init!($name, ComponentClass, Component, get_class);

        $crate::identity::paste::item! {
            #[allow(non_upper_case_globals)]
            #[allow(non_snake_case)]
            #[$crate::identity::ctor::ctor]
            fn [<_ $name _component_init>]() {
                // Get lock
                let _lock = ComponentClass::id_gen_lock();

                // Initialize the class
                $name::custom_id_type_init();

                // Set up component builders
                unsafe {
                    $crate::component::COMP_VEC_BUILDERS.push(Box::new(|| Box::new(Vec::<$name>::new())))
                }
            }
        }
    };
}

pub static mut COMP_VEC_BUILDERS: Vec<Box<Fn() -> Box<ComponentVec>>> = Vec::new();
pub static mut COMP_DEF_BUILDERS: Vec<Box<Fn() -> CompDefVec>> = Vec::new();

pub trait ComponentClassAux {
    fn comp_vec_builder(&self) -> &'static Box<Fn() -> Box<ComponentVec>>;
    fn comp_def_builder(&self) -> &'static Box<Fn() -> CompDefVec>;
}

impl ComponentClassAux for ComponentClass {
    fn comp_vec_builder(&self) -> &'static Box<Fn() -> Box<ComponentVec>> {
        unsafe {
            COMP_VEC_BUILDERS.get_unchecked(self.indexer())
        }
    }

    fn comp_def_builder(&self) -> &'static Box<Fn() -> CompDefVec> {
        unsafe {
            COMP_DEF_BUILDERS.get_unchecked(self.indexer())
        }
    }
}

pub(crate) type ComponentCoords = (ShardKey, usize);

pub trait Component: DeserializeOwned + Debug {
    fn get_class() -> ComponentClass;

    #[inline]
    fn get_type_indexer() -> usize {
        Self::get_class().indexer()
    }

    #[inline]
    fn get_type_name() -> &'static str {
        Self::get_class().name()
    }
}

pub trait ComponentVec {
    fn append(&mut self, data: &mut CompDefVec);
    fn remove(&mut self, loc: usize);
    fn len(&self) -> usize;
    unsafe fn get_ptr(&self) -> DynPtr;
}

impl<T> ComponentVec for Vec<T>
where
    T: 'static + Component,
{
    #[inline]
    fn append(&mut self, data: &mut CompDefVec) {
        let data_vec = data.cast_mut_vector::<T>();
        self.append(data_vec);
    }

    #[inline]
    fn remove(&mut self, loc: usize) {
        self.swap_remove(loc);
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    unsafe fn get_ptr(&self) -> DynPtr {
        DynPtr::new_unchecked(self as *const Vec<T>)
    }
}

pub trait CompDef: DynVecOps + Debug {
    fn push_json(&mut self, json: &str);
    fn clone_box(&self) -> Box<CompDef>;
}

impl<T> CompDef for Vec<T>
    where
        T: 'static + Component,
{
    #[inline]
    fn push_json(&mut self, json: &str) {
        self.push(serde_json::from_str(json).expect("Error deserializing component"));
    }

    #[inline]
    fn clone_box(&self) -> Box<CompDef> {
        Box::new(Vec::<T>::new())
    }
}

pub type CompDefVec = DynVec<CompDef>;

impl CompDefVec {
    #[inline]
    pub fn push<T>(&mut self, item: T)
        where
            T: 'static + Component,
    {
        self.cast_mut_vector::<T>().push(item);
    }

    // Quite nasty hack to allow internal mutability
    #[allow(clippy::mut_from_ref)]
    #[inline]
    pub unsafe fn cast_mut_unchecked<T>(&self) -> &mut Vec<T>
        where
            T: 'static + Component,
    {
        &mut *(self.get_inner_ptr().cast_checked_raw())
    }
}

#[allow(clippy::box_vec)]
pub struct Shard {
    pub(crate) key: ShardKey,
    // The pointer to the vec itself needs to be stable, hence the box.
    entities: Box<Vec<EntityId>>,
    store: HashMap<ComponentClass, Box<ComponentVec>>,
}

impl Shard {
    pub fn new(key: ShardKey, store: HashMap<ComponentClass, Box<ComponentVec>>) -> Shard {
        Shard {
            key,
            entities: Box::new(Vec::new()),
            store,
        }
    }

    pub fn new_with_ents(
        key: ShardKey,
        entities: Vec<EntityId>,
        store: HashMap<ComponentClass, Box<ComponentVec>>,
    ) -> Shard {
        Shard {
            key,
            entities: Box::new(entities),
            store,
        }
    }

    pub fn ingest(&mut self, shard_def: &mut ShardDef) -> usize {
        if shard_def.entity_ids.is_empty() {
            panic!("No entities to ingest");
        }

        for (id, data) in shard_def.components.iter_mut() {
            self.store.get_mut(id).unwrap().append(data);
        }

        let loc_start = self.entities.len();

        self.entities.extend(&shard_def.entity_ids);

        loc_start
    }

    #[inline]
    pub fn remove(&mut self, loc: usize) -> Option<EntityId> {
        self.entities.swap_remove(loc);

        for data in self.store.values_mut() {
            data.remove(loc);
        }

        self.entities.get(loc).and_then(|eid| Some(*eid))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    #[inline]
    pub fn data_ptr<T>(&self) -> *const Vec<T>
    where
        T: 'static + Component,
    {
        if T::get_class() == EntityId::get_class() {
            unsafe { self.entities.get_ptr().cast_checked_raw() }
        } else {
            unsafe {
                self.store
                    .get(&T::get_class())
                    .unwrap()
                    .get_ptr()
                    .cast_checked_raw()
            }
        }
    }

    #[inline]
    pub fn data_mut_ptr<T>(&self) -> *mut Vec<T>
    where
        T: 'static + Component,
    {
        if T::get_class() == EntityId::get_class() {
            panic!("Entity ID vector is not writeable")
        }

        unsafe {
            self.store
                .get(&T::get_class())
                .unwrap()
                .get_ptr()
                .cast_checked_raw()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_init;
    use serde_derive::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    struct SomeComponent {
        x: i32,
        y: i32,
    }

    component_init!(SomeComponent);

    #[test]
    fn test_ingest() {
        let some_comp_cls = SomeComponent::get_class();

        let mut shard = Shard::new(ShardKey::empty(), HashMap::new());
        shard
            .store
            .insert(some_comp_cls, Box::new(Vec::<SomeComponent>::new()));

        let mut shard_def = ShardDef {
            entity_ids: vec![0.into(), 1.into(), 2.into()],
            components: HashMap::new(),
        };

        // Load some components
        let data = vec![
            SomeComponent { x: 0, y: 0 },
            SomeComponent { x: 1, y: 1 },
            SomeComponent { x: 2, y: 2 },
        ];

        shard_def.components.insert(some_comp_cls, CompDefVec::new(data));

        assert_eq!(shard.ingest(&mut shard_def), 0);
        assert_eq!(shard.entities.len(), 3);
        assert_eq!(shard.store[&some_comp_cls].len(), 3);
    }

    #[test]
    fn test_remove() {
        let some_comp_cls = SomeComponent::get_class();

        let mut map: HashMap<_, Box<ComponentVec>> = HashMap::new();

        // Load some components
        let data = vec![
            SomeComponent { x: 0, y: 0 },
            SomeComponent { x: 1, y: 1 },
            SomeComponent { x: 2, y: 2 },
        ];

        map.insert(some_comp_cls, Box::new(data));

        let mut shard = Shard::new(ShardKey::empty(), map);

        // Add matching entity entries
        shard.entities.push(0.into());
        shard.entities.push(1.into());
        shard.entities.push(2.into());

        // Remove from front, swapping id 2 in
        assert_eq!(shard.remove(0).unwrap(), 2.into());
        assert_eq!(shard.entities.len(), 2);
        assert_eq!(shard.store[&some_comp_cls].len(), 2);

        // Remove the tail, no swapping
        assert!(shard.remove(1).is_none());
        assert_eq!(shard.entities.len(), 1);
        assert_eq!(shard.store[&some_comp_cls].len(), 1);

        // Remove last item, no swapping
        assert!(shard.remove(0).is_none());
        assert_eq!(shard.entities.len(), 0);
        assert_eq!(shard.store[&some_comp_cls].len(), 0);
    }

    #[test]
    fn test_data_ptr() {
        let mut map: HashMap<_, Box<ComponentVec>> = HashMap::new();
        map.insert(
            SomeComponent::get_class(),
            Box::new(Vec::<SomeComponent>::new()),
        );

        let shard = Shard::new(ShardKey::empty(), map);

        assert!(!shard.data_ptr::<EntityId>().is_null());
        assert!(!shard.data_ptr::<SomeComponent>().is_null());
    }

    #[test]
    fn test_data_mut_ptr() {
        let mut map: HashMap<_, Box<ComponentVec>> = HashMap::new();
        map.insert(
            SomeComponent::get_class(),
            Box::new(Vec::<SomeComponent>::new()),
        );

        let shard = Shard::new(ShardKey::empty(), map);

        assert!(!shard.data_mut_ptr::<SomeComponent>().is_null());
    }

    #[test]
    #[should_panic(expected = "Entity ID vector is not writeable")]
    fn test_entity_id_mut_ptr_fail() {
        let shard = Shard::new(ShardKey::empty(), HashMap::new());
        shard.data_mut_ptr::<EntityId>();
    }
}

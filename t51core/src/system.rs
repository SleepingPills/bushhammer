use crate::entity::{Entity, EntityId, EntityStore};

pub trait System {
    fn run(&mut self, entities: EntityStore);
    fn add_entity(&mut self, entity: &Entity);
    fn remove_entity(&mut self, id: EntityId);
}

/// Marker for designating the components required by the system and their mutability.
pub struct SystemData<T> {
    _x: T,
    _never: (),
}

impl<T> SystemData<T> {
    pub fn get_ctx(&self) -> Context<T> {
        unreachable!()
    }
}

pub struct Context<T> {
    _x: T,
    _never: (),
}

impl<T> Context<T> {
    pub fn iter(&self) -> SystemDataIter<T> {
        unreachable!()
    }

    #[allow(unused_variables)]
    pub unsafe fn get_by_id(&self, entity_id: EntityId) -> T {
        unreachable!()
    }
}

impl<T> IntoIterator for Context<T> {
    type Item = T;
    type IntoIter = SystemDataIter<T>;

    fn into_iter(self) -> SystemDataIter<T> {
        unreachable!()
    }
}

pub struct SystemDataIter<T> {
    _x: T,
    _never: (),
}

impl<T> Iterator for SystemDataIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        unreachable!()
    }
}

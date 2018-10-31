use crate::object::{ComponentId, SystemId};
use crate::entity::{Entity, EntityId, EntityStore};
use crate::registry::Registry;

pub trait System {
    fn run(&mut self, entities: EntityStore);

    #[allow(unused_variables)]
    fn init(&mut self, components: &Registry<ComponentId>, systems: &Registry<SystemId>) {}
    #[allow(unused_variables)]
    fn entity_added(&mut self, entity: &Entity) {}
    #[allow(unused_variables)]
    fn entity_removed(&mut self, id: EntityId) {}
}

pub trait ManagedSystem : System {
    fn add_entity(&mut self, entity: &Entity);
    fn remove_entity(&mut self, id: EntityId);
}

pub trait BuildableSystem : ManagedSystem {
    fn new(components: &Registry<ComponentId>) -> Self;
    fn required_components() -> Vec<ComponentId>;
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

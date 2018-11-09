use std::any::TypeId;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::intrinsics::type_name;

#[macro_export]
macro_rules! object_id {
    ($name: ident) => {
        #[derive(Copy, Clone, Debug)]
        pub struct $name {
            id: TypeId,
            pub name: &'static str,
        }

        impl $name {
            #[inline(always)]
            pub fn get_id(&self) -> TypeId {
                self.id
            }

            #[inline(always)]
            pub fn new_type(id: TypeId, name: &'static str) -> $name {
                $name { id, name }
            }

            #[inline(always)]
            pub fn new<T: 'static>() -> $name {
                unsafe {
                    $name {
                        id: TypeId::of::<T>(),
                        name: type_name::<T>(),
                    }
                }
            }
        }

        impl Eq for $name {}

        impl PartialEq for $name {
            #[inline(always)]
            fn eq(&self, other: &$name) -> bool {
                self.id == other.id
            }
        }

        impl Hash for $name {
            #[inline(always)]
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.id.hash(state)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({:?}, {})", stringify!($name), self.id, self.name)
            }
        }
    };
}

pub type EntityId = usize;
pub type ShardId = usize;
object_id!(SystemId);
object_id!(ComponentId);

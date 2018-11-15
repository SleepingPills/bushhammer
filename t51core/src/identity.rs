use std::mem;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::intrinsics::type_name;
use std::cmp::Ordering;
use serde_derive::{Deserialize, Serialize};

#[macro_export]
macro_rules! object_id {
    ($name: ident, $type: ty) => {
        #[derive(Copy, Clone, Debug)]
        pub struct $name {
            pub id: $type,
            pub name: &'static str,
        }

        impl $name {
            /// Creates a new instance. Unique ids are distinguished by a bitmask, there is thus a limit to the
            /// total number of unique ids. E.g. in case of `u64`, it is 64. Trying to go over this limit will
            /// cause the method to panic. This is to enable efficient set operations and membership tests on
            /// groups of ids.
            #[inline(always)]
            pub fn new<T:'static>(cur_count: usize) -> $name {
                let name = unsafe { type_name::<T>() };

                let limit = mem::size_of::<IdType>() * 8;
                let power = cur_count;

                if (power + 1) >= limit {
                    panic!("{} limit {} exceeded", name, limit)
                }

                $name {
                    id: (1 as IdType) << power,
                    name,
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

        impl PartialOrd for $name {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                self.id.partial_cmp(&other.id)
            }
        }

        impl Ord for $name {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> Ordering {
                self.id.cmp(&other.id)
            }
        }

        impl fmt::Display for $name {
            #[inline(always)]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({:?}, {})", stringify!($name), self.id, self.name)
            }
        }
    };
}

pub(crate) type IdType = u64;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct EntityId(IdType);

impl From<u64> for EntityId {
    #[inline]
    fn from(id: u64) -> Self {
        EntityId(id)
    }
}

impl Into<u64> for EntityId {
    #[inline]
    fn into(self) -> u64 {
        self.0
    }
}

impl From<usize> for EntityId {
    #[inline]
    fn from(id: usize) -> Self {
        if mem::size_of::<usize>() > 8 {
            panic!("Casting `usize` to `Id` will lead to precision loss.")
        }
        EntityId(id as u64)
    }
}

impl Into<usize> for EntityId {
    #[inline]
    fn into(self) -> usize {
        if mem::size_of::<usize>() < 8 {
            panic!("Casting `Id` to `usize` will lead to precision loss.")
        }

        self.0 as usize
    }
}

impl From<i32> for EntityId {
    #[inline]
    fn from(id: i32) -> Self {
        EntityId(id as IdType)
    }
}

pub type ShardId = IdType;

object_id!(SystemId, IdType);
object_id!(ComponentId, IdType);

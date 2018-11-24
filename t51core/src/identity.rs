use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::intrinsics::type_name;
use std::mem;
use hashbrown::HashMap;
use std::any::TypeId;

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
            pub fn new<T: 'static>(cur_count: usize) -> $name {
                let name = unsafe { type_name::<T>() };

                let limit = mem::size_of::<$type>() * 8;
                let power = cur_count;

                if (power + 1) >= limit {
                    panic!("{} limit {} exceeded", name, limit)
                }

                $name {
                    id: (1 as $type) << power,
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

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct EntityId(u32);

impl Into<usize> for EntityId {
    #[inline]
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for EntityId {
    #[inline]
    fn from(id: u32) -> Self {
        EntityId(id)
    }
}

impl Into<u32> for EntityId {
    #[inline]
    fn into(self) -> u32 {
        self.0
    }
}

impl From<i32> for EntityId {
    #[inline]
    fn from(id: i32) -> Self {
        EntityId(id as u32)
    }
}

impl Into<i32> for EntityId {
    #[inline]
    fn into(self) -> i32 {
        self.0 as i32
    }
}

pub(crate) type BitSetIdType = u64;
pub type ShardId = BitSetIdType;

object_id!(SystemId, BitSetIdType);
object_id!(ComponentId, BitSetIdType);

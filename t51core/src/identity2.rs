use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::intrinsics::type_name;
use std::mem;

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

trait BitFlagIndexer {
    fn indexer(&self) -> usize;
}

#[macro_export]
macro_rules! bitflag_type_id {
    ($name: ident, $type: ty) => {
        #[derive(Copy, Clone, Debug)]
        #[repr(transparent)]
        pub struct $name {
            pub id: $type,
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
                }
            }
        }

        impl BitFlagIndexer for $name {
            #[inline]
            fn indexer(&self) -> usize {
                self.id.leading_zeros() as usize
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
                write!(f, "{}({:?})", stringify!($name), self.id)
            }
        }
    };
}

pub(crate) type BitSetIdType = u64;

// TODO: Drop the ShardId and just use ShardKey as the unique identifier of a shard. It can be directly constructed
// from vectors or tuples of component Ids or even generic types anywhere.
pub type ShardId = BitSetIdType;

bitflag_type_id!(SystemId, BitSetIdType);
bitflag_type_id!(ComponentId, BitSetIdType);

static mut COMPONENT_NAMES: Vec<&'static str> = Vec::new();

trait ComponentTypeIdentity {
    fn acquire_type_id();
    fn get_type_id() -> ComponentId;

    #[inline]
    fn get_type_indexer() -> usize {
        Self::get_type_id().indexer()
    }

    #[inline]
    fn get_type_name() -> &'static str {
        unsafe { COMPONENT_NAMES[Self::get_type_indexer()] }
    }
}

/*
TODO
TODO: Use count leading zeros on u64 since it is a single machine instruction and gets a unique index for the component.
TODO: Replace all hashmap usage with ComponentIds with a simple Vec and then use the get_type_indexer to index into it.

- TypeIdRegistry struct
    - Global ID Counter
    - Global TypeId -> CustomTypeId map

trait CustomTypeIdentity {
    fn acquire_type_id();
    fn get_type_id();
}

- The trait will be implemented for each component by a proc macro.
- A static variable will be added by the proc macro.
- acquire_type_id() will be run by the world instances when registering components.
    - It will use a Once thingy
    - The Once will use the length of COMPONENT_NAMES as the counter
    - It will generate a new Id
    - It will then set the name based on the
    - It will acquire a mutex on the IdGenerator.
    - It will then ask the IdGenerator for an Id, passing in it's own TypeId
    - The IdGenerator will check if this type already has a type id, and if not, it will increment
      the internal counter and return the id
    - The function will then set the static custom id
- get_type_id() will just return the id value

*/

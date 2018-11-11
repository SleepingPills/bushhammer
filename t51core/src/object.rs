use std::mem;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::intrinsics::type_name;
use std::cmp::Ordering;

#[macro_export]
macro_rules! object_id {
    ($name: ident, $type: ty) => {
        #[derive(Copy, Clone, Debug)]
        pub struct $name {
            pub id: $type,
            pub name: &'static str,
        }

        impl $name {
            #[inline(always)]
            pub fn new<T:'static>(cur_count: usize) -> $name {
                let name = unsafe { type_name::<T>() };

                let limit = mem::size_of::<IdType>() * 8;
                let power = cur_count;

                if (power + 1) >= limit {
                    panic!("{} limit {} exceeded", name, limit)
                }

                let id = 2u64.pow(power as u32);

                $name {
                    id,
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

pub type EntityId = usize;
pub type ShardId = usize;

pub(crate) type IdType = u64;

object_id!(SystemId, IdType);
object_id!(ComponentId, IdType);

use std::fmt;
use std::intrinsics::type_name;
use std::mem;
use std::ops;

#[macro_export]
macro_rules! custom_type_id {
    ($name: ident, $type: ty, $name_vec: ident, $id_vec: ident) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        #[repr(transparent)]
        pub struct $name {
            pub id: $type,
        }

        impl $name {
            #[inline]
            pub unsafe fn get_name_vec() -> &'static mut Vec<&'static str> {
                &mut $name_vec
            }

            #[inline]
            pub unsafe fn get_id_vec() -> &'static mut Vec<$name> {
                &mut $id_vec
            }

            #[inline]
            pub unsafe fn reset_static() {
                Self::get_name_vec().clear();
                Self::get_id_vec().clear();
            }
        }

        impl fmt::Display for $name {
            #[inline(always)]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.id)
            }
        }

        static mut $name_vec: Vec<&'static str> = Vec::new();
        static mut $id_vec: Vec<$name> = Vec::new();
    };
}

#[macro_export]
macro_rules! bitflag_type_id {
    ($name: ident, $type: ty, $name_vec: ident, $id_vec: ident, $composite_key: ident) => {
        custom_type_id!($name, $type, $name_vec, $id_vec);

        impl $name {
            /// Creates a new instance. Unique ids are distinguished by a bitmask, there is thus a limit to the
            /// total number of unique ids. E.g. in case of `u64`, it is 64. Trying to go over this limit will
            /// cause the method to panic. This is to enable efficient set operations and membership tests on
            /// groups of ids.
            #[inline(always)]
            pub fn new<T: 'static>(cur_count: usize) -> $name {
                let name = unsafe { type_name::<T>() };

                let power = cur_count;

                if (power + 1) >= ID_BIT_LENGTH {
                    panic!("{} limit {} exceeded", name, ID_BIT_LENGTH)
                }

                $name {
                    id: (1 as $type) << power,
                }
            }

            #[inline]
            pub fn indexer(&self) -> usize {
                self.id.trailing_zeros() as usize
            }
        }

        #[repr(transparent)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $composite_key($type);

        impl $composite_key {
            #[inline]
            pub fn empty() -> $composite_key {
                $composite_key(0)
            }

            #[inline]
            pub fn from_iter<'a>(keys: impl Iterator<Item = &'a $name>) -> $composite_key {
                $composite_key(keys.fold(0, |acc, cid| acc | cid.id))
            }

            #[inline]
            pub fn count(&self) -> u32 {
                self.0.count_ones()
            }

            #[inline]
            pub fn decompose(&self) -> impl Iterator<Item = $name> {
                let mut field = self.0;

                let first_index = self.0.leading_zeros() as usize;
                let last_index = ID_BIT_LENGTH - self.0.leading_zeros() as usize;

                (first_index..last_index).filter_map(move |index| unsafe {
                    let result = match field & 1 {
                        1 => Some($id_vec[index]),
                        _ => None,
                    };
                    field >>= 1;
                    result
                })
            }

            #[inline]
            pub fn contains_key(&self, other: $composite_key) -> bool {
                (self.0 & other.0) == other.0
            }

            #[inline]
            pub fn contains_id(&self, other: $name) -> bool {
                (self.0 & other.id) == other.id
            }
        }

        impl From<$name> for $composite_key {
            fn from(id: $name) -> Self {
                $composite_key(id.id)
            }
        }

        impl ops::BitOr for $name {
            type Output = $composite_key;

            #[inline]
            fn bitor(self, rhs: $name) -> Self::Output {
                $composite_key(self.id | rhs.id)
            }
        }

        impl ops::BitOr<$composite_key> for $name {
            type Output = $composite_key;

            #[inline]
            fn bitor(self, rhs: $composite_key) -> Self::Output {
                $composite_key(self.id | rhs.0)
            }
        }

        impl ops::BitOr<$name> for $composite_key {
            type Output = $composite_key;

            #[inline]
            fn bitor(self, rhs: $name) -> Self::Output {
                $composite_key(self.0 | rhs.id)
            }
        }

        impl ops::Add for $name {
            type Output = $composite_key;

            #[inline]
            fn add(self, rhs: $name) -> Self::Output {
                $composite_key(self.id | rhs.id)
            }
        }

        impl ops::Add<$composite_key> for $name {
            type Output = $composite_key;

            #[inline]
            fn add(self, rhs: $composite_key) -> Self::Output {
                $composite_key(self.id | rhs.0)
            }
        }

        impl ops::Add<$name> for $composite_key {
            type Output = $composite_key;

            #[inline]
            fn add(self, rhs: $name) -> Self::Output {
                $composite_key(self.0 | rhs.id)
            }
        }

        impl ops::AddAssign<$name> for $composite_key {
            fn add_assign(&mut self, rhs: $name) {
                self.0 |= rhs.id;
            }
        }

        impl ops::Sub<$name> for $composite_key {
            type Output = $composite_key;

            #[inline]
            fn sub(self, rhs: $name) -> Self::Output {
                $composite_key(self.0 & (!rhs.id))
            }
        }

        impl ops::SubAssign<$name> for $composite_key {
            #[inline]
            fn sub_assign(&mut self, rhs: $name) {
                self.0 &= !rhs.id;
            }
        }
    };
}

pub(crate) type BitFlagId = u64;
const ID_BIT_LENGTH: usize = mem::size_of::<BitFlagId>() * 8;

bitflag_type_id!(ComponentId, BitFlagId, COMP_NAME_VEC, COMP_ID_VEC, ShardKey);
bitflag_type_id!(SystemId, BitFlagId, SYS_NAME_VEC, SYS_ID_VEC, BundleKey);
custom_type_id!(TopicId, i16, TOPIC_NAME_VEC, TOPIC_ID_VEC);
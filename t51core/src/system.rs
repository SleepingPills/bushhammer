use component::ComponentStore;
use indexmap::map::Iter;
use indexmap::IndexMap;
use std::marker::PhantomData;

pub mod indexing {
    use super::*;
    use sync::ReadGuard;
    use sync::RwGuard;

    pub trait Indexer {
        type Item;

        fn get(&self, index: usize) -> Self::Item;
    }

    pub struct ReadIndexer<'a, T>
    where
        T: 'a,
    {
        pub(crate) ptr: *const T,
        pub(crate) _borrow: ReadGuard<ComponentStore<T>>,
        pub(crate) _x: PhantomData<&'a T>,
    }

    impl<'a, T> Indexer for ReadIndexer<'a, T>
    where
        T: 'a,
    {
        type Item = &'a T;

        #[inline(always)]
        fn get(&self, index: usize) -> &'a T {
            unsafe { &*self.ptr.add(index) }
        }
    }

    pub struct WriteIndexer<'a, T>
    where
        T: 'a,
    {
        pub(crate) ptr: *mut T,
        pub(crate) _borrow: RwGuard<ComponentStore<T>>,
        pub(crate) _x: PhantomData<&'a T>,
    }

    impl<'a, T> Indexer for WriteIndexer<'a, T>
    where
        T: 'a,
    {
        type Item = &'a mut T;

        #[inline(always)]
        fn get(&self, index: usize) -> &'a mut T {
            unsafe { &mut *self.ptr.add(index) }
        }
    }
}

pub mod storage {
    use super::indexing::*;
    use super::*;
    use std::sync::Arc;
    use sync::RwCell;

    pub trait Store {
        type Indexer;

        fn as_indexer(&self) -> Self::Indexer;
    }

    pub struct ReadStore<'a, T>
    where
        T: 'a,
    {
        pub(crate) data: Arc<RwCell<ComponentStore<T>>>,
        pub(crate) _x: PhantomData<&'a T>,
    }

    impl<'a, T> ReadStore<'a, T> {
        #[inline(always)]
        pub fn new(data: Arc<RwCell<ComponentStore<T>>>) -> Self {
            ReadStore {
                data,
                _x: PhantomData,
            }
        }
    }

    impl<'a, T> Store for ReadStore<'a, T>
    where
        T: 'a,
    {
        type Indexer = ReadIndexer<'a, T>;

        #[inline(always)]
        fn as_indexer(&self) -> ReadIndexer<'a, T> {
            unsafe {
                let guard = self.data.read();
                ReadIndexer::<T> {
                    ptr: guard.pool.get_store_ptr(),
                    _borrow: guard,
                    _x: PhantomData,
                }
            }
        }
    }

    pub struct WriteStore<'a, T>
    where
        T: 'a,
    {
        pub(crate) data: Arc<RwCell<ComponentStore<T>>>,
        pub(crate) _x: PhantomData<&'a T>,
    }

    impl<'a, T> WriteStore<'a, T> {
        #[inline(always)]
        pub fn new(data: Arc<RwCell<ComponentStore<T>>>) -> Self {
            WriteStore {
                data,
                _x: PhantomData,
            }
        }
    }

    impl<'a, T> Store for WriteStore<'a, T>
    where
        T: 'a,
    {
        type Indexer = WriteIndexer<'a, T>;

        #[inline(always)]
        fn as_indexer(&self) -> WriteIndexer<'a, T> {
            unsafe {
                let mut guard = self.data.write();
                WriteIndexer::<T> {
                    ptr: guard.pool.get_store_mut_ptr(),
                    _borrow: guard,
                    _x: PhantomData,
                }
            }
        }
    }
}

pub mod join {
    use super::indexing::*;
    use super::storage::*;
    use super::*;

    macro_rules! _decl_system_replace_expr {
        ($_t:tt $sub:ty) => {
            $sub
        };
    }

    macro_rules! joiniter {
        ($iname:ident; $( $field_name:ident:$field_type:ident ),*) => {
            pub struct $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                mapiter: Iter<'a, usize, ($(_decl_system_replace_expr!($field_name usize)),*,)>,
                $($field_name: $field_type),*
            }

            impl<'a, $($field_type),*> Iterator for $iname<'a, $($field_type),*>
                where $($field_type: Indexer),* {
                type Item = (usize, $($field_type::Item),*);

                #[inline(always)]
                fn next(&mut self) -> Option<(usize, $($field_type::Item),*)> {
                    match self.mapiter.next() {
                        Some((&id, &($($field_name),*,))) => Some((id, $(self.$field_name.get($field_name)),*)),
                        _ => None
                    }
                }

                #[inline(always)]
                fn size_hint(&self) -> (usize, Option<usize>) {
                    self.mapiter.size_hint()
                }
            }
        }
    }

    joiniter!(Join1Iterator; a:A);
    joiniter!(Join2Iterator; a:A, b:B);
    joiniter!(Join3Iterator; a:A, b:B, c:C);
    joiniter!(Join4Iterator; a:A, b:B, c:C, d:D);
    joiniter!(Join5Iterator; a:A, b:B, c:C, d:D, e:E);

    macro_rules! join {
        ($iname:ident; $itertype:ident; $( $field_name:ident:$field_type:ident ),*) => {
            pub struct $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                mapping: &'a IndexMap<usize, ($(_decl_system_replace_expr!($field_name usize)),*,)>,
                $($field_name: $field_type),*
            }

            impl<'a, $($field_type),*> $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                #[inline(always)]
                pub fn get(&self, id: usize) -> ($($field_type::Item),*) {
                    let &($($field_name),*,) = self.mapping.get(&id).unwrap();
                    ($(self.$field_name.get($field_name)),*)
                }
            }

            impl<'a, $($field_type),*> IntoIterator for $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                type Item = (usize, $($field_type::Item),*);
                type IntoIter = $itertype<'a, $($field_type),*>;

                #[inline(always)]
                fn into_iter(self) -> $itertype<'a, $($field_type),*> {
                    $itertype { mapiter: self.mapping.iter(), $($field_name: self.$field_name),*}
                }
            }
        }
    }

    join!(Join1; Join1Iterator; a:A);
    join!(Join2; Join2Iterator; a:A, b:B);
    join!(Join3; Join3Iterator; a:A, b:B, c:C);
    join!(Join4; Join4Iterator; a:A, b:B, c:C, d:D);
    join!(Join5; Join5Iterator; a:A, b:B, c:C, d:D, e:E);

    macro_rules! joinable {
        ($iname:ident; $jointype:ident; $( $field_name:ident:$field_type:ident:$field_seq:tt ),*) => {
            pub trait $iname<$($field_type),*>
                where $($field_type: Store),*,$($field_type::Indexer: Indexer),* {
                fn join(&self) -> $jointype<$($field_type::Indexer),*>;
            }

            impl<$($field_type),*> $iname<$($field_type),*>
                for (IndexMap<usize, ($(_decl_system_replace_expr!($field_name usize)),*,)>, $($field_type),*)
                where $($field_type: Store),*,$($field_type::Indexer: Indexer),* {

                #[inline(always)]
                fn join(&self) -> $jointype<$($field_type::Indexer),*> {
                    $jointype { mapping: &self.0, $($field_name: self.$field_seq.as_indexer()),* }
                }
            }
        }
    }

    joinable!(Joinable1; Join1; a:A:1);
    joinable!(Joinable2; Join2; a:A:1, b:B:2);
    joinable!(Joinable3; Join3; a:A:1, b:B:2, c:C:3);
    joinable!(Joinable4; Join4; a:A:1, b:B:2, c:C:3, d:D:4);
    joinable!(Joinable5; Join5; a:A:1, b:B:2, c:C:3, d:D:4, e:E:5);
}

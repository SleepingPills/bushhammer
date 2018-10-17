use indexmap::IndexMap;
use indexmap::map::Iter;
use std::marker::PhantomData;
use std::cell::RefCell;


pub mod indexing {
    use super::*;

    pub trait Indexer: Copy {
        type Item;

        fn get(&self, index: usize) -> Self::Item;
    }

    pub struct ReadIndexer<'a, T> where T: 'a {
        pub(crate) ptr: *const T,
        pub(crate) _x: PhantomData<&'a T>,
    }

    impl<'a, T> Indexer for ReadIndexer<'a, T> where T: 'a {
        type Item = &'a T;

        fn get(&self, index: usize) -> &'a T {
            unsafe {
                &*self.ptr.offset(index as isize)
            }
        }
    }

    impl<'a, T> Copy for ReadIndexer<'a, T> where T: 'a {}

    impl<'a, T> Clone for ReadIndexer<'a, T> where T: 'a {
        fn clone(&self) -> Self {
            ReadIndexer { ptr: self.ptr, _x: PhantomData }
        }
    }

    pub struct WriteIndexer<'a, T> where T: 'a {
        pub(crate) ptr: *mut T,
        pub(crate) _x: PhantomData<&'a T>,
    }

    impl<'a, T> Indexer for WriteIndexer<'a, T> where T: 'a {
        type Item = &'a mut T;

        fn get(&self, index: usize) -> &'a mut T {
            unsafe {
                &mut *self.ptr.offset(index as isize)
            }
        }
    }

    impl<'a, T> Copy for WriteIndexer<'a, T> where T: 'a {}

    impl<'a, T> Clone for WriteIndexer<'a, T> where T: 'a {
        fn clone(&self) -> Self {
            WriteIndexer { ptr: self.ptr, _x: PhantomData }
        }
    }
}


pub mod storage {
    use super::*;
    use super::indexing::*;

    pub trait Store {
        type Indexer;

        fn as_indexer(&self) -> Self::Indexer;
    }


    pub struct ReadStore<'a, T> where T: 'a {
        pub(crate) data: &'a RefCell<Vec<T>>,
    }


    impl<'a, T> Store for ReadStore<'a, T>
        where T: 'a {
        type Indexer = ReadIndexer<'a, T>;

        fn as_indexer(&self) -> ReadIndexer<'a, T> {
            ReadIndexer::<T> { ptr: self.data.borrow().as_ptr(), _x: PhantomData }
        }
    }


    pub struct WriteStore<'a, T> where T: 'a {
        pub(crate) data: &'a RefCell<Vec<T>>,
    }


    impl<'a, T> Store for WriteStore<'a, T>
        where T: 'a {
        type Indexer = WriteIndexer<'a, T>;

        fn as_indexer(&self) -> WriteIndexer<'a, T> {
            WriteIndexer::<T> { ptr: self.data.borrow_mut().as_mut_ptr(), _x: PhantomData }
        }
    }
}


pub mod join {
    use super::*;
    use super::indexing::*;
    use super::storage::*;

    macro_rules! _decl_system_replace_expr {
        ($_t:tt $sub:ty) => {$sub};
    }

    macro_rules! joiniter{
        ($iname:ident; $( $field_name:ident:$field_type:ident ),*) => {
            pub struct $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                mapiter: Iter<'a, usize, ($(_decl_system_replace_expr!($field_name usize)),*,)>,
                $($field_name: $field_type),*
            }

            impl<'a, $($field_type),*> Iterator for $iname<'a, $($field_type),*>
                where $($field_type: Indexer),* {
                type Item = (usize, $($field_type::Item),*);

                fn next(&mut self) -> Option<(usize, $($field_type::Item),*)> {
                    match self.mapiter.next() {
                        Some((&id, &($($field_name),*,))) => Some((id, $(self.$field_name.get($field_name)),*)),
                        _ => None
                    }
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
                pub fn get(&self, id: usize) -> ($($field_type::Item),*) {
                    let &($($field_name),*,) = self.mapping.get(&id).unwrap();
                    ($(self.$field_name.get($field_name)),*)
                }
            }

            impl<'a, $($field_type),*> Copy for $iname<'a, $($field_type),*> where $($field_type: Indexer),* {}

            impl<'a, $($field_type),*> Clone for $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                fn clone(&self) -> Self {
                    $iname{mapping: self.mapping, $($field_name: self.$field_name),*}
                }
            }

            impl<'a, $($field_type),*> IntoIterator for $iname<'a, $($field_type),*> where $($field_type: Indexer),* {
                type Item = (usize, $($field_type::Item),*);
                type IntoIter = $itertype<'a, $($field_type),*>;

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


/*
pub mod join {
    use super::*;
    use super::indexing::*;
    use super::storage::*;

    pub struct Join2Iterator<'a, A, B> where A: Indexer, B: Indexer {
        mapiter: Iter<'a, usize, (usize, usize)>,
        a: A,
        b: B,
    }

    impl<'a, A, B> Iterator for Join2Iterator<'a, A, B> where A: Indexer, B: Indexer {
        type Item = (usize, A::Item, B::Item);

        fn next(&mut self) -> Option<(usize, A::Item, B::Item)> {
            match self.mapiter.next() {
                Some((&id, &(a, b))) => Some((id, self.a.get(a), self.b.get(b))),
                _ => None
            }
        }
    }

    pub struct Join2<'a, A, B> where A: Indexer, B: Indexer {
        mapping: &'a IndexMap<usize, (usize, usize)>,
        a: A,
        b: B,
    }

    impl<'a, A, B> Join2<'a, A, B> where A: Indexer, B: Indexer {
        pub fn get(&self, id: usize) -> (A::Item, B::Item) {
            let &(a, b) = self.mapping.get(&id).unwrap();
            (self.a.get(a), self.b.get(b))
        }
    }

    impl<'a, A, B> Copy for Join2<'a, A, B> where A: Indexer, B: Indexer {
    }

    impl<'a, A, B> Clone for Join2<'a, A, B> where A: Indexer, B: Indexer {
        fn clone(&self) -> Self {
            Join2{mapping: self.mapping, a: self.a, b: self.b}
        }
    }

    impl<'a, A, B> IntoIterator for Join2<'a, A, B> where A: Indexer, B: Indexer {
        type Item = (usize, A::Item, B::Item);
        type IntoIter = Join2Iterator<'a, A, B>;

        fn into_iter(self) -> Join2Iterator<'a, A, B> {
            Join2Iterator { mapiter: self.mapping.iter(), a: self.a, b: self.b }
        }
    }

    pub trait Joinable2<A, B>
        where A: Store, B: Store, A::Indexer: Indexer, B::Indexer: Indexer {
        fn join(&self) -> Join2<A::Indexer, B::Indexer>;
    }


    impl<A, B> Joinable2<A, B> for (IndexMap<usize, (usize, usize)>, A, B)
        where A: Store, B: Store, A::Indexer: Indexer, B::Indexer: Indexer {
        fn join(&self) -> Join2<A::Indexer, B::Indexer> {
            Join2 { mapping: &self.0, a: self.1.as_indexer(), b: self.2.as_indexer() }
        }
    }
}
*/

use alloc::VecPool;

pub struct ComponentStore<T> {
    pub(crate) pool: VecPool<T>,
}

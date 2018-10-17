use alloc::VecPool;


pub struct ComponentStore<T> {
    pub pool: VecPool<T>
}

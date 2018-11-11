use std::ops::Deref;
use std::ops::DerefMut;

#[repr(transparent)]
pub struct Take<T> {
    data: Option<T>,
}

/// Sentinel for values that may be temporarily moved out of their field.
impl<T> Take<T> {
    #[inline]
    pub fn new(data: T) -> Self {
        Take { data: Some(data) }
    }

    /// Take the value out from the sentinel. The sentinel won't be usable until a value is put back in.
    #[inline]
    pub fn take(&mut self) -> T {
        self.data.take().expect("Data already taken")
    }

    /// Put a value in the sentinel. Any old value will be lost.
    #[inline]
    pub fn put(&mut self, data: T) {
        self.data = data.into()
    }

    /// Returns `true` if the sentinel is empty.
    #[inline]
    pub fn is_taken(&self) -> bool {
        self.data.is_none()
    }
}

impl<T> Deref for Take<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.data.as_ref().expect("Data already taken")
    }
}

impl<T> DerefMut for Take<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.data.as_mut().expect("Data already taken")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deref() {
        let sentinel = Take::new(5);

        assert_eq!(*sentinel, 5);
    }

    #[test]
    fn test_deref_mut() {
        let mut sentinel = Take::new(5);

        *sentinel = 1;

        assert_eq!(*sentinel, 1);
    }

    #[test]
    fn test_take_put() {
        let mut sentinel = Take::new(5);

        let value = sentinel.take();

        assert_eq!(value, 5);
        assert!(sentinel.is_taken());

        sentinel.put(1);

        assert_eq!(*sentinel, 1)
    }

    #[test]
    #[should_panic(expected = "Data already taken")]
    fn test_panic_deref_when_taken() {
        let mut sentinel = Take::new(5);
        sentinel.take();
        let result = *sentinel;
    }

    #[test]
    #[should_panic(expected = "Data already taken")]
    fn test_panic_deref_mut_when_taken() {
        let mut sentinel = Take::new(5);
        sentinel.take();
        *sentinel = 1;
    }

    #[test]
    #[should_panic(expected = "Data already taken")]
    fn test_panic_take_when_taken() {
        let mut sentinel = Take::new(5);
        sentinel.take();
        sentinel.take();
    }
}

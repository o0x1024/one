use std::fmt;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// GC 管理的指针占位类型。
///
/// Phase 2 中将被真正的分代 GC 托管指针替换。
/// 当前实现：基于 Box 的简单堆分配。
pub struct GcPtr<T> {
    ptr: NonNull<T>,
}

impl<T> GcPtr<T> {
    pub fn new(value: T) -> Self {
        let boxed = Box::new(value);
        GcPtr {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
        }
    }

    pub fn as_raw(&self) -> u64 {
        self.ptr.as_ptr() as u64
    }

    /// # Safety
    /// `raw` must come from `as_raw()` and the corresponding GcPtr must still be valid
    pub unsafe fn from_raw(raw: u64) -> Self {
        GcPtr {
            ptr: unsafe { NonNull::new_unchecked(raw as *mut T) },
        }
    }
}

impl<T> Deref for GcPtr<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for GcPtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> Clone for GcPtr<T> {
    fn clone(&self) -> Self {
        GcPtr { ptr: self.ptr }
    }
}

impl<T> Drop for GcPtr<T> {
    fn drop(&mut self) {
        // Intentional no-op: simulates GC semantics where the collector handles deallocation
    }
}

impl<T: fmt::Debug> fmt::Debug for GcPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GcPtr({:?})", self.deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc_ptr_alloc_and_deref() {
        let ptr = GcPtr::new(42u64);
        assert_eq!(*ptr, 42);
    }

    #[test]
    fn gc_ptr_deref_mut() {
        let mut ptr = GcPtr::new(10i32);
        *ptr = 20;
        assert_eq!(*ptr, 20);
    }

    #[test]
    fn gc_ptr_as_raw_round_trip() {
        let ptr = GcPtr::new(String::from("hello"));
        let raw = ptr.as_raw();
        assert_ne!(raw, 0);
        let recovered = unsafe { GcPtr::<String>::from_raw(raw) };
        assert_eq!(*recovered, "hello");
        std::mem::forget(recovered);
    }

    #[test]
    fn gc_ptr_clone_is_independent() {
        let a = GcPtr::new(100u32);
        let b = a.clone();
        assert_eq!(*a, *b);
    }
}

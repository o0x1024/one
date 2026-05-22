use std::fmt;
use std::ops::Deref;

use crate::trace::{Trace, Tracer};

/// A GC-managed pointer. Wraps a raw pointer to a heap-allocated value.
/// In this Phase 2 version, it's a thin wrapper. Full root tracking comes later.
#[derive(Clone, Copy)]
pub struct Gc<T: Trace + 'static> {
    ptr: *mut T,
}

impl<T: Trace + 'static> Gc<T> {
    /// Create from a raw pointer returned by Heap::alloc
    #[allow(dead_code)]
    pub(crate) fn from_raw(ptr: *mut T) -> Self {
        Gc { ptr }
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    pub fn as_raw(&self) -> u64 {
        self.ptr as u64
    }
}

impl<T: Trace + 'static> Deref for Gc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: Trace + fmt::Debug + 'static> fmt::Debug for Gc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gc({:?})", self.deref())
    }
}

impl<T: Trace + 'static> Trace for Gc<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark(self.ptr as *const u8);
    }
}

impl<T: Trace + PartialEq + 'static> PartialEq for Gc<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.ptr, other.ptr)
    }
}

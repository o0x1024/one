use std::cell::{Cell, RefCell};
use std::mem;

pub struct Arena {
    chunks: RefCell<Vec<Vec<u8>>>,
    current: Cell<*mut u8>,
    end: Cell<*mut u8>,
}

const CHUNK_SIZE: usize = 4096;

impl Arena {
    pub fn new() -> Self {
        let arena = Arena {
            chunks: RefCell::new(Vec::new()),
            current: Cell::new(std::ptr::null_mut()),
            end: Cell::new(std::ptr::null_mut()),
        };
        arena.grow(CHUNK_SIZE);
        arena
    }

    /// Allocate a value in the arena, returning a reference with arena lifetime
    pub fn alloc<T>(&self, value: T) -> &T {
        let layout = std::alloc::Layout::new::<T>();
        let ptr = self.alloc_raw(layout);
        unsafe {
            ptr.cast::<T>().write(value);
            &*ptr.cast::<T>()
        }
    }

    /// Allocate a slice in the arena
    pub fn alloc_vec<T>(&self, items: Vec<T>) -> &[T] {
        if items.is_empty() {
            return &[];
        }
        let layout = std::alloc::Layout::array::<T>(items.len()).expect("invalid slice layout");
        let len = items.len();
        let ptr = self.alloc_raw(layout);
        unsafe {
            ptr.cast::<T>().copy_from_nonoverlapping(items.as_ptr(), len);
            mem::forget(items);
            std::slice::from_raw_parts(ptr.cast::<T>(), len)
        }
    }

    fn alloc_raw(&self, layout: std::alloc::Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let ptr = self.alloc_bytes(size, align);
        debug_assert_eq!(ptr as usize % align, 0);
        ptr
    }

    fn alloc_bytes(&self, size: usize, align: usize) -> *mut u8 {
        let mut current = self.current.get() as usize;
        let aligned = align_up(current, align);
        let needed = aligned - current + size;

        if aligned + size > self.end.get() as usize {
            self.grow(needed.max(CHUNK_SIZE));
            current = self.current.get() as usize;
        }

        let ptr = align_up(current, align) as *mut u8;
        self.current.set(unsafe { ptr.add(size) });
        ptr
    }

    fn grow(&self, needed: usize) {
        let chunk_size = needed.max(CHUNK_SIZE);
        let mut chunk = vec![0u8; chunk_size];
        let start = chunk.as_mut_ptr();
        let end = unsafe { start.add(chunk_size) };
        self.chunks.borrow_mut().push(chunk);
        self.current.set(start);
        self.end.set(end);
    }
}

fn align_up(ptr: usize, align: usize) -> usize {
    (ptr + align - 1) & !(align - 1)
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_alloc_and_read() {
        let arena = Arena::new();
        let val = arena.alloc(42u64);
        assert_eq!(*val, 42);
    }

    #[test]
    fn arena_multiple_allocs() {
        let arena = Arena::new();
        let a = arena.alloc(1u32);
        let b = arena.alloc(2u32);
        let c = arena.alloc(3u32);
        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
        assert_eq!(*c, 3);
    }

    #[test]
    fn arena_alloc_different_types() {
        let arena = Arena::new();
        let n = arena.alloc(42u64);
        let s = arena.alloc(String::from("hello"));
        let b = arena.alloc(true);
        assert_eq!(*n, 42);
        assert_eq!(s.as_str(), "hello");
        assert_eq!(*b, true);
    }

    #[test]
    fn arena_alloc_vec() {
        let arena = Arena::new();
        let items = vec![1, 2, 3, 4, 5];
        let slice = arena.alloc_vec(items);
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn arena_large_allocation() {
        let arena = Arena::new();
        for i in 0..10000u64 {
            let val = arena.alloc(i);
            assert_eq!(*val, i);
        }
    }
}

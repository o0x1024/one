use std::alloc::{Layout, alloc, dealloc};
use std::any::TypeId;
use std::collections::HashMap;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use crate::header::{GcHeader, GcVtable};
use crate::trace::{Trace, Tracer};

static VTABLES: OnceLock<Mutex<HashMap<TypeId, &'static GcVtable>>> = OnceLock::new();

pub struct Heap {
    /// Linked list of all allocated objects
    head: Option<*mut GcHeader>,
    /// Total bytes allocated
    bytes_allocated: usize,
    /// Threshold to trigger GC
    gc_threshold: usize,
    /// Number of allocations
    alloc_count: usize,
}

impl Heap {
    pub fn new() -> Self {
        Heap {
            head: None,
            bytes_allocated: 0,
            gc_threshold: 1024 * 1024, // 1MB initial threshold
            alloc_count: 0,
        }
    }

    /// Allocate a value on the GC heap. Returns a raw pointer to the value.
    pub fn alloc<T: Trace + 'static>(&mut self, value: T) -> *mut T {
        let size = std::mem::size_of::<T>();
        let total_size = std::mem::size_of::<GcHeader>() + size;
        let align = std::mem::align_of::<GcHeader>().max(std::mem::align_of::<T>());
        let layout = Layout::from_size_align(total_size, align).unwrap();

        unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            let header_ptr = ptr as *mut GcHeader;
            let data_ptr = ptr.add(std::mem::size_of::<GcHeader>()) as *mut T;

            // Write value
            ptr::write(data_ptr, value);

            // Write header
            ptr::write(
                header_ptr,
                GcHeader {
                    marked: false,
                    finalized: false,
                    size: size as u32,
                    type_id: TypeId::of::<T>(),
                    vtable: Self::vtable::<T>(),
                    next: self.head,
                },
            );

            self.head = Some(header_ptr);
            self.bytes_allocated += total_size;
            self.alloc_count += 1;

            data_ptr
        }
    }

    /// Get the GcHeader for a data pointer.
    ///
    /// # Safety
    ///
    /// `data_ptr` must point to the payload immediately following a valid `GcHeader`
    /// allocated by this heap.
    pub unsafe fn header_of<T>(data_ptr: *const T) -> *mut GcHeader {
        unsafe {
            (data_ptr as *const u8).sub(std::mem::size_of::<GcHeader>()) as *mut GcHeader
        }
    }

    /// Run a mark-sweep GC cycle
    pub fn collect(&mut self, roots: &[*const u8]) {
        // Phase 1: Unmark all
        self.unmark_all();

        // Phase 2: Mark from roots
        let mut marker = MarkTracer;
        for &root in roots {
            if !root.is_null() {
                marker.mark(root);
            }
        }

        // Phase 3: Sweep unmarked
        self.sweep();
    }

    fn unmark_all(&mut self) {
        let mut current = self.head;
        while let Some(ptr) = current {
            unsafe {
                (*ptr).marked = false;
                current = (*ptr).next;
            }
        }
    }

    fn sweep(&mut self) {
        let mut prev: Option<*mut GcHeader> = None;
        let mut current = self.head;

        while let Some(ptr) = current {
            unsafe {
                let next = (*ptr).next;
                if !(*ptr).marked {
                    // Remove from list
                    if let Some(prev_ptr) = prev {
                        (*prev_ptr).next = next;
                    } else {
                        self.head = next;
                    }
                    // Drop and dealloc
                    self.free(ptr);
                } else {
                    prev = Some(ptr);
                }
                current = next;
            }
        }
    }

    unsafe fn free(&mut self, header: *mut GcHeader) {
        unsafe {
            let header_ref = &*header;
            let total_size = std::mem::size_of::<GcHeader>() + header_ref.size as usize;
            let align = std::mem::align_of::<GcHeader>();
            let data_ptr = (header as *mut u8).add(std::mem::size_of::<GcHeader>());

            // Drop the value
            (header_ref.vtable.drop_fn)(data_ptr);

            // Dealloc memory
            let layout = Layout::from_size_align_unchecked(total_size, align);
            self.bytes_allocated -= total_size;
            self.alloc_count -= 1;
            dealloc(header as *mut u8, layout);
        }
    }

    fn vtable<T: Trace + 'static>() -> &'static GcVtable {
        let cache = VTABLES.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().unwrap();
        guard.entry(TypeId::of::<T>()).or_insert_with(|| {
            Box::leak(Box::new(GcVtable {
                trace_fn: |ptr, tracer| unsafe { &*(ptr as *const T) }.trace(tracer),
                drop_fn: |ptr| unsafe {
                    ptr::drop_in_place(ptr as *mut T);
                },
                type_name: std::any::type_name::<T>(),
            }))
        })
    }

    pub fn should_collect(&self) -> bool {
        self.bytes_allocated >= self.gc_threshold
    }

    pub fn grow_threshold(&mut self) {
        self.gc_threshold = (self.bytes_allocated * 2).max(1024 * 1024);
    }

    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.gc_threshold = threshold;
    }

    pub fn stats(&self) -> HeapStats {
        HeapStats {
            bytes_allocated: self.bytes_allocated,
            alloc_count: self.alloc_count,
            gc_threshold: self.gc_threshold,
        }
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        // Free all remaining objects
        let mut current = self.head;
        while let Some(ptr) = current {
            unsafe {
                current = (*ptr).next;
                self.free(ptr);
            }
        }
    }
}

struct MarkTracer;

impl Tracer for MarkTracer {
    fn mark(&mut self, ptr: *const u8) {
        unsafe {
            let header = Heap::header_of(ptr);
            if !(*header).marked {
                (*header).marked = true;
                // Trace children
                ((*header).vtable.trace_fn)(ptr, self);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeapStats {
    pub bytes_allocated: usize,
    pub alloc_count: usize,
    pub gc_threshold: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::Trace;

    #[derive(Debug)]
    struct TestObj {
        value: i32,
    }
    impl Trace for TestObj {
        fn trace(&self, _: &mut dyn Tracer) {}
    }

    #[test]
    fn alloc_and_read() {
        let mut heap = Heap::new();
        let ptr = heap.alloc(TestObj { value: 42 });
        assert_eq!(unsafe { &*ptr }.value, 42);
    }

    #[test]
    fn multiple_allocs() {
        let mut heap = Heap::new();
        let a = heap.alloc(TestObj { value: 1 });
        let b = heap.alloc(TestObj { value: 2 });
        let c = heap.alloc(TestObj { value: 3 });
        unsafe {
            assert_eq!((*a).value, 1);
            assert_eq!((*b).value, 2);
            assert_eq!((*c).value, 3);
        }
        assert_eq!(heap.stats().alloc_count, 3);
    }

    #[test]
    fn gc_collects_unreachable() {
        let mut heap = Heap::new();
        let _a = heap.alloc(TestObj { value: 1 });
        let b = heap.alloc(TestObj { value: 2 });
        assert_eq!(heap.stats().alloc_count, 2);
        // Only b is a root
        heap.collect(&[b as *const u8]);
        assert_eq!(heap.stats().alloc_count, 1);
        assert_eq!(unsafe { &*b }.value, 2);
    }

    #[test]
    fn gc_keeps_reachable() {
        let mut heap = Heap::new();
        let a = heap.alloc(TestObj { value: 1 });
        let b = heap.alloc(TestObj { value: 2 });
        heap.collect(&[a as *const u8, b as *const u8]);
        assert_eq!(heap.stats().alloc_count, 2);
    }

    #[test]
    fn gc_collects_all_when_no_roots() {
        let mut heap = Heap::new();
        heap.alloc(TestObj { value: 1 });
        heap.alloc(TestObj { value: 2 });
        heap.alloc(TestObj { value: 3 });
        assert_eq!(heap.stats().alloc_count, 3);
        heap.collect(&[]);
        assert_eq!(heap.stats().alloc_count, 0);
    }

    #[test]
    fn heap_drop_cleans_up() {
        let mut heap = Heap::new();
        heap.alloc(String::from("hello"));
        heap.alloc(String::from("world"));
        assert_eq!(heap.stats().alloc_count, 2);
        drop(heap);
        // No memory leak (would show up in sanitizers)
    }

    #[test]
    fn alloc_string() {
        let mut heap = Heap::new();
        let ptr = heap.alloc(String::from("hello world"));
        let s = unsafe { &*ptr };
        assert_eq!(s.as_str(), "hello world");
    }
}

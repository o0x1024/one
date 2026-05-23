use std::alloc::{Layout, alloc, dealloc};
use std::any::TypeId;
use std::collections::HashMap;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use crate::header::{GcHeader, GcVtable, PROMOTION_AGE};
use crate::trace::{Trace, Tracer};

static VTABLES: OnceLock<Mutex<HashMap<TypeId, &'static GcVtable>>> = OnceLock::new();

const DEFAULT_NURSERY_CAPACITY: usize = 256 * 1024;
const DEFAULT_OLD_GEN_THRESHOLD: usize = 1024 * 1024;

pub struct Heap {
    /// Linked list of all allocated objects
    head: Option<*mut GcHeader>,
    /// Total bytes allocated (young + old)
    bytes_allocated: usize,
    /// Bytes in the young generation
    nursery_size: usize,
    /// Nursery capacity before triggering minor GC
    nursery_capacity: usize,
    /// Bytes in the old generation
    old_gen_size: usize,
    /// Old generation threshold before triggering major GC
    old_gen_threshold: usize,
    /// Number of allocations
    alloc_count: usize,
    /// Total bytes reclaimed by GC
    total_collected: usize,
    /// Total GC cycles
    gc_count: u32,
    minor_gc_count: u32,
    major_gc_count: u32,
}

impl Heap {
    pub fn new() -> Self {
        Heap {
            head: None,
            bytes_allocated: 0,
            nursery_size: 0,
            nursery_capacity: DEFAULT_NURSERY_CAPACITY,
            old_gen_size: 0,
            old_gen_threshold: DEFAULT_OLD_GEN_THRESHOLD,
            alloc_count: 0,
            total_collected: 0,
            gc_count: 0,
            minor_gc_count: 0,
            major_gc_count: 0,
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
                    generation: 0,
                    age: 0,
                    size: size as u32,
                    type_id: TypeId::of::<T>(),
                    vtable: Self::vtable::<T>(),
                    next: self.head,
                },
            );

            self.head = Some(header_ptr);
            self.bytes_allocated += total_size;
            self.nursery_size += total_size;
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

    /// Run a full mark-sweep GC cycle over all generations.
    pub fn collect(&mut self, roots: &[*const u8]) {
        self.gc_count += 1;
        self.full_mark_sweep(roots);
    }

    /// Minor GC — collect young generation only, promote long-lived survivors.
    pub fn minor_collect(&mut self, roots: &[*const u8]) {
        self.minor_gc_count += 1;
        self.gc_count += 1;

        self.unmark_all();

        let mut marker = MarkTracer;
        for &root in roots {
            if !root.is_null() {
                marker.mark(root);
            }
        }

        self.sweep_young();
    }

    /// Major GC — full mark-sweep over all generations.
    pub fn major_collect(&mut self, roots: &[*const u8]) {
        self.major_gc_count += 1;
        self.gc_count += 1;
        self.full_mark_sweep(roots);
    }

    fn full_mark_sweep(&mut self, roots: &[*const u8]) {
        self.unmark_all();

        let mut marker = MarkTracer;
        for &root in roots {
            if !root.is_null() {
                marker.mark(root);
            }
        }

        self.sweep_all();
    }

    /// Visit the data pointer of every live object on the heap.
    pub fn visit_objects<F>(&self, mut visit: F)
    where
        F: FnMut(*mut u8),
    {
        let mut current = self.head;
        while let Some(header_ptr) = current {
            unsafe {
                let data_ptr = (header_ptr as *mut u8).add(std::mem::size_of::<GcHeader>());
                visit(data_ptr);
                current = (*header_ptr).next;
            }
        }
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

    fn sweep_young(&mut self) {
        let mut prev: Option<*mut GcHeader> = None;
        let mut current = self.head;

        while let Some(ptr) = current {
            unsafe {
                let next = (*ptr).next;
                let header = &mut *ptr;
                if header.generation == 0 {
                    if header.marked {
                        header.age = header.age.saturating_add(1);
                        if header.age >= PROMOTION_AGE {
                            let total_size =
                                std::mem::size_of::<GcHeader>() + header.size as usize;
                            header.generation = 1;
                            self.nursery_size = self.nursery_size.saturating_sub(total_size);
                            self.old_gen_size += total_size;
                        }
                        prev = Some(ptr);
                    } else {
                        if let Some(prev_ptr) = prev {
                            (*prev_ptr).next = next;
                        } else {
                            self.head = next;
                        }
                        self.free(ptr);
                    }
                } else {
                    prev = Some(ptr);
                }
                current = next;
            }
        }
    }

    fn sweep_all(&mut self) {
        let mut prev: Option<*mut GcHeader> = None;
        let mut current = self.head;

        while let Some(ptr) = current {
            unsafe {
                let next = (*ptr).next;
                if !(*ptr).marked {
                    if let Some(prev_ptr) = prev {
                        (*prev_ptr).next = next;
                    } else {
                        self.head = next;
                    }
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

            if header_ref.generation == 0 {
                self.nursery_size = self.nursery_size.saturating_sub(total_size);
            } else {
                self.old_gen_size = self.old_gen_size.saturating_sub(total_size);
            }
            self.total_collected += total_size;

            // Drop the value
            (header_ref.vtable.drop_fn)(data_ptr);

            // Dealloc memory
            let layout = Layout::from_size_align_unchecked(total_size, align);
            self.bytes_allocated -= total_size;
            self.alloc_count -= 1;
            dealloc(header as *mut u8, layout);
        }
    }

    /// Returns true if `data_ptr` points to a live object on this heap.
    pub fn contains_ptr(&self, data_ptr: *const u8) -> bool {
        let mut current = self.head;
        while let Some(header_ptr) = current {
            unsafe {
                let this_data = (header_ptr as *const u8).add(std::mem::size_of::<GcHeader>());
                if this_data == data_ptr {
                    return true;
                }
                current = (*header_ptr).next;
            }
        }
        false
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

    /// True when the nursery (young generation) is full.
    pub fn should_collect(&self) -> bool {
        self.nursery_size >= self.nursery_capacity
    }

    /// True when the old generation exceeds its threshold.
    pub fn should_major_collect(&self) -> bool {
        self.old_gen_size >= self.old_gen_threshold
    }

    pub fn grow_threshold(&mut self) {
        self.nursery_capacity = (self.nursery_size * 2).max(1024).max(self.nursery_capacity);
        self.old_gen_threshold = (self.old_gen_size * 2).max(1024 * 1024).max(self.old_gen_threshold);
    }

    pub fn set_gc_threshold(&mut self, threshold: usize) {
        self.nursery_capacity = threshold;
        self.old_gen_threshold = threshold.saturating_mul(4).max(threshold);
    }

    pub fn total_allocated(&self) -> usize {
        self.bytes_allocated
    }

    pub fn total_collected(&self) -> usize {
        self.total_collected
    }

    pub fn gc_count(&self) -> u32 {
        self.gc_count
    }

    pub fn minor_gc_count(&self) -> u32 {
        self.minor_gc_count
    }

    pub fn major_gc_count(&self) -> u32 {
        self.major_gc_count
    }

    pub fn nursery_size(&self) -> usize {
        self.nursery_size
    }

    pub fn old_gen_size(&self) -> usize {
        self.old_gen_size
    }

    pub fn stats(&self) -> HeapStats {
        HeapStats {
            bytes_allocated: self.bytes_allocated,
            alloc_count: self.alloc_count,
            nursery_capacity: self.nursery_capacity,
            nursery_size: self.nursery_size,
            old_gen_size: self.old_gen_size,
            old_gen_threshold: self.old_gen_threshold,
            total_collected: self.total_collected,
            gc_count: self.gc_count,
            minor_gc_count: self.minor_gc_count,
            major_gc_count: self.major_gc_count,
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
                ((*header).vtable.trace_fn)(ptr, self);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeapStats {
    pub bytes_allocated: usize,
    pub alloc_count: usize,
    pub nursery_capacity: usize,
    pub nursery_size: usize,
    pub old_gen_size: usize,
    pub old_gen_threshold: usize,
    pub total_collected: usize,
    pub gc_count: u32,
    pub minor_gc_count: u32,
    pub major_gc_count: u32,
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

    #[test]
    fn minor_gc_collects_young_unreachable() {
        let mut heap = Heap::new();
        heap.set_gc_threshold(1);
        let _dead = heap.alloc(TestObj { value: 1 });
        let live = heap.alloc(TestObj { value: 2 });
        assert_eq!(heap.stats().alloc_count, 2);
        heap.minor_collect(&[live as *const u8]);
        assert_eq!(heap.stats().alloc_count, 1);
        assert_eq!(unsafe { &*live }.value, 2);
    }

    #[test]
    fn minor_gc_promotes_after_age_threshold() {
        let mut heap = Heap::new();
        let ptr = heap.alloc(TestObj { value: 42 });
        let root = ptr as *const u8;

        for _ in 0..PROMOTION_AGE {
            heap.minor_collect(&[root]);
        }

        unsafe {
            let header = Heap::header_of(ptr);
            assert_eq!((*header).generation, 1);
        }
        assert!(heap.old_gen_size() > 0);
    }
}

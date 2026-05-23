use std::any::TypeId;

/// Number of minor GC cycles a young object must survive before promotion.
pub const PROMOTION_AGE: u8 = 3;

/// Header prepended to every GC-managed object.
#[repr(C, align(16))]
pub struct GcHeader {
    /// Mark bit for GC tracing
    pub marked: bool,
    /// Object has been finalized (prevent double-finalize)
    pub finalized: bool,
    /// Generation: 0 = young (nursery), 1 = old (tenured)
    pub generation: u8,
    /// Number of minor GC cycles this object has survived
    pub age: u8,
    /// Size of the object (excluding header) in bytes
    pub size: u32,
    /// Type ID for safe downcasting
    pub type_id: TypeId,
    /// Pointer to the vtable for trace/drop
    pub vtable: &'static GcVtable,
    /// Next object in the allocation list
    pub next: Option<*mut GcHeader>,
}

/// Virtual dispatch table for GC operations on a type
pub struct GcVtable {
    pub trace_fn: fn(*const u8, &mut dyn super::trace::Tracer),
    pub drop_fn: fn(*mut u8),
    pub type_name: &'static str,
}

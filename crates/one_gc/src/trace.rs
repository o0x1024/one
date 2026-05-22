/// Trait for GC-managed types. Every type on the GC heap must implement this.
pub trait Trace {
    /// Visit all GC pointers contained in this value.
    fn trace(&self, tracer: &mut dyn Tracer);
}

/// Callback interface for the GC to discover references.
pub trait Tracer {
    fn mark(&mut self, ptr: *const u8);
}

// Blanket impls for primitive types (no GC pointers to trace)
impl Trace for bool {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for u8 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for u16 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for u32 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for u64 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for i32 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for i64 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for f64 {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl Trace for String {
    fn trace(&self, _: &mut dyn Tracer) {}
}
impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        for item in self {
            item.trace(tracer);
        }
    }
}
impl<T: Trace> Trace for Option<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        if let Some(v) = self {
            v.trace(tracer);
        }
    }
}
impl<T: Trace> Trace for Box<T> {
    fn trace(&self, tracer: &mut dyn Tracer) {
        (**self).trace(tracer);
    }
}

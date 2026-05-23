#[derive(Debug, Clone, Default)]
pub struct RuntimeLimits {
    pub max_operations: Option<u64>,
    pub max_call_depth: Option<usize>,
    pub max_string_bytes: Option<usize>,
    pub max_array_length: Option<usize>,
    pub max_object_properties: Option<usize>,
}

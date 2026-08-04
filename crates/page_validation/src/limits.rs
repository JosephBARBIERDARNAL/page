#[derive(Clone, Debug)]
pub struct SafetyLimits {
    pub max_input_size: u64,
    pub max_decoded_stream_size: usize,
    pub max_object_count: usize,
    pub max_reference_depth: usize,
}

impl SafetyLimits {
    /// ISO 19005-1:2005, 6.1.12-7 permits at most this many indirect objects.
    pub const PDF_A1_MAX_INDIRECT_OBJECTS: usize = 8_388_607;
    pub const DEFAULT_MAX_INPUT_SIZE: u64 = 256 * 1024 * 1024;
    pub const DEFAULT_MAX_DECODED_STREAM_SIZE: usize = 32 * 1024 * 1024;
    pub const DEFAULT_MAX_OBJECT_COUNT: usize = 1_000_000;
    pub const DEFAULT_MAX_REFERENCE_DEPTH: usize = 128;
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_input_size: Self::DEFAULT_MAX_INPUT_SIZE,
            max_decoded_stream_size: Self::DEFAULT_MAX_DECODED_STREAM_SIZE,
            max_object_count: Self::DEFAULT_MAX_OBJECT_COUNT,
            max_reference_depth: Self::DEFAULT_MAX_REFERENCE_DEPTH,
        }
    }
}

/// Byte offset in source code
pub type BytePos = u32;

/// A span of source code
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: BytePos,
    pub end: BytePos,
}

impl Span {
    pub const fn new(start: BytePos, end: BytePos) -> Self {
        Span { start, end }
    }

    pub const fn empty() -> Self {
        Span { start: 0, end: 0 }
    }

    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn contains(&self, pos: BytePos) -> bool {
        pos >= self.start && pos < self.end
    }
}

impl Default for Span {
    fn default() -> Self {
        Self::empty()
    }
}

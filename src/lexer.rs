use crate::types::lookup;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    pub file_id: u32,
    pub line: u32,
    pub col: u32,
    pub byte_offset: u32,
    pub byte_len: u32,
}

impl Default for Loc {
    fn default() -> Self {
        Self {
            file_id: 0,
            line: 1,
            col: 1,
            byte_offset: 0,
            byte_len: 1,
        }
    }
}

impl Loc {
    pub fn new(file_id: u32, line: u32, col: u32, byte_offset: u32, byte_len: u32) -> Self {
        Self {
            file_id,
            line,
            col,
            byte_offset,
            byte_len,
        }
    }

    pub fn point(file_id: u32, line: u32, col: u32) -> Self {
        Self {
            file_id,
            line,
            col,
            byte_offset: 0,
            byte_len: 1,
        }
    }

    pub fn merge(self, other: Loc) -> Loc {
        if self.file_id != other.file_id {
            return self;
        }
        let (start_loc, end_loc) = if self.byte_offset <= other.byte_offset {
            (self, other)
        } else {
            (other, self)
        };
        let end_byte = (end_loc.byte_offset + end_loc.byte_len).max(start_loc.byte_offset + start_loc.byte_len);
        let byte_len = end_byte.saturating_sub(start_loc.byte_offset);
        Loc {
            file_id: start_loc.file_id,
            line: start_loc.line,
            col: start_loc.col,
            byte_offset: start_loc.byte_offset,
            byte_len,
        }
    }
}

impl std::fmt::Display for Loc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", lookup(self.file_id), self.line, self.col)
    }
}

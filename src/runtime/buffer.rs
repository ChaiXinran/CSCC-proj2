//! V10 ArrayBuffer, TypedArray, and DataView runtime substrate.

use super::StableId;

/// Stable handle for shared ArrayBuffer byte storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArrayBufferId(pub u32);
impl StableId for ArrayBufferId {
    fn from_u32(value: u32) -> Self {
        Self(value)
    }
    fn to_u32(self) -> u32 {
        self.0
    }
}

/// Shared byte storage used by future ArrayBuffer, TypedArray, and DataView builtins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayBufferRecord {
    pub bytes: Vec<u8>,
    pub detached: bool,
    pub max_byte_length: usize,
    pub resizable: bool,
    pub immutable: bool,
    pub shared: bool,
}

impl ArrayBufferRecord {
    #[must_use]
    pub fn new(byte_length: usize) -> Self {
        Self::with_options(byte_length, byte_length, false, false)
    }

    #[must_use]
    pub fn with_options(
        byte_length: usize,
        max_byte_length: usize,
        resizable: bool,
        immutable: bool,
    ) -> Self {
        Self::with_options_and_shared(byte_length, max_byte_length, resizable, immutable, false)
    }

    #[must_use]
    pub fn with_options_and_shared(
        byte_length: usize,
        max_byte_length: usize,
        resizable: bool,
        immutable: bool,
        shared: bool,
    ) -> Self {
        Self {
            bytes: vec![0; byte_length],
            detached: false,
            max_byte_length,
            resizable,
            immutable,
            shared,
        }
    }

    #[must_use]
    pub fn byte_length(&self) -> usize {
        if self.detached { 0 } else { self.bytes.len() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypedArrayViewId(pub u32);
impl StableId for TypedArrayViewId {
    fn from_u32(value: u32) -> Self {
        Self(value)
    }
    fn to_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataViewId(pub u32);
impl StableId for DataViewId {
    fn from_u32(value: u32) -> Self {
        Self(value)
    }
    fn to_u32(self) -> u32 {
        self.0
    }
}

/// Element kind and byte-width information for typed-array views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypedArrayElementKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float16,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

impl TypedArrayElementKind {
    #[must_use]
    pub const fn bytes_per_element(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 | Self::Uint8Clamped => 1,
            Self::Int16 | Self::Uint16 | Self::Float16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 | Self::BigInt64 | Self::BigUint64 => 8,
        }
    }

    #[must_use]
    pub const fn is_bigint(self) -> bool {
        matches!(self, Self::BigInt64 | Self::BigUint64)
    }
}

/// Runtime metadata for a typed-array view over an ArrayBuffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedArrayView {
    pub buffer: ArrayBufferId,
    pub byte_offset: usize,
    pub length: usize,
    pub length_tracking: bool,
    pub element_kind: TypedArrayElementKind,
}

impl TypedArrayView {
    #[must_use]
    pub fn fixed_byte_length(&self) -> Option<usize> {
        self.length
            .checked_mul(self.element_kind.bytes_per_element())
    }
}

/// Runtime metadata for a DataView over an ArrayBuffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataViewRecord {
    pub buffer: ArrayBufferId,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub length_tracking: bool,
}

//! Shared V9 iterator runtime records.

use super::JsValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IteratorMode {
    Key,
    Value,
    KeyAndValue,
}

/// Minimal iterator record used by runtime helpers before full Iterator
/// builtins are installed.
#[derive(Debug, Clone, PartialEq)]
pub struct IteratorRecord {
    pub(crate) kind: IteratorKind,
    pub(crate) done: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IteratorKind {
    Array {
        object: JsValue,
        index: usize,
        length: usize,
        mode: IteratorMode,
    },
    String {
        chars: Vec<String>,
        index: usize,
    },
    Js {
        iterator: JsValue,
    },
}

impl IteratorRecord {
    pub(crate) fn array(object: JsValue, length: usize) -> Self {
        Self::array_with_mode(object, length, IteratorMode::Value)
    }

    pub(crate) fn array_with_mode(object: JsValue, length: usize, mode: IteratorMode) -> Self {
        Self {
            kind: IteratorKind::Array {
                object,
                index: 0,
                length,
                mode,
            },
            done: false,
        }
    }

    pub(crate) fn string(value: String) -> Self {
        Self {
            kind: IteratorKind::String {
                chars: value.chars().map(String::from).collect(),
                index: 0,
            },
            done: false,
        }
    }

    pub(crate) fn js(iterator: JsValue) -> Self {
        Self {
            kind: IteratorKind::Js { iterator },
            done: false,
        }
    }

    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.done
    }
}

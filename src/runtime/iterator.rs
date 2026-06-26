//! Shared V9 iterator runtime records.

use super::JsValue;

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
        Self {
            kind: IteratorKind::Array {
                object,
                index: 0,
                length,
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

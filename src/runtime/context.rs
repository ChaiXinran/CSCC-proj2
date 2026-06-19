//! Persistent state shared by native execution and integration.

use super::Heap;

/// Per-isolate language state passed to the bytecode executor.
#[derive(Debug, Default)]
pub struct NativeContext {
    heap: Heap,
    strict: bool,
    output: Vec<String>,
}

impl NativeContext {
    #[must_use]
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }

    #[must_use]
    pub const fn strict(&self) -> bool {
        self.strict
    }

    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    pub fn push_output(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }
}

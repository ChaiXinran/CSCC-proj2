//! Garbage-collection boundary.

use super::Heap;

/// Statistics returned by one collection pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CollectionStats {
    pub objects_before: usize,
    pub objects_after: usize,
}

/// Collection entry point.
///
/// The initial arena retains all objects. A mark-and-sweep implementation can
/// replace this type without changing VM or object APIs.
#[derive(Debug, Default)]
pub struct Collector;

impl Collector {
    #[must_use]
    pub fn collect(&mut self, heap: &mut Heap) -> CollectionStats {
        let count = heap.object_count();
        CollectionStats {
            objects_before: count,
            objects_after: count,
        }
    }
}

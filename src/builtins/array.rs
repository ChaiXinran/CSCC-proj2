//! `Array` constructor and prototype bootstrap.

use crate::runtime::Heap;

pub fn install_array(_heap: &mut Heap) {
    // Array exotic behavior lands after ordinary property semantics.
}

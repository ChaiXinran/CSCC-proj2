//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

pub use array::install_array;
pub use function::install_function;
pub use object::install_object;

use crate::runtime::Heap;

/// Installs the foundational native constructors and prototypes.
pub fn install_foundation(heap: &mut Heap) {
    install_object(heap);
    install_function(heap);
    install_array(heap);
}

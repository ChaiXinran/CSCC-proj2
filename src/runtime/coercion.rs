//! ECMAScript type-coercion hint shared between the runtime and the VM.

/// Hint passed to `ToPrimitive` to influence which conversion method is tried first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredType {
    /// No preference — use `valueOf` then `toString` (same as `Number` for ordinary objects).
    Default,
    /// Prefer numeric conversion: try `valueOf` first, then `toString`.
    Number,
    /// Prefer string conversion: try `toString` first, then `valueOf`.
    String,
}

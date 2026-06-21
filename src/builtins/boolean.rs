//! Pure algorithms and metadata for the V6 `Boolean` builtins.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BooleanMethodSpec {
    pub name: &'static str,
    pub length: u8,
}

pub(crate) const BOOLEAN_PROTOTYPE_METHODS: &[BooleanMethodSpec] = &[
    BooleanMethodSpec {
        name: "toString",
        length: 0,
    },
    BooleanMethodSpec {
        name: "valueOf",
        length: 0,
    },
];

#[must_use]
pub(crate) fn boolean_call(value: bool) -> bool {
    value
}

#[must_use]
pub(crate) fn boolean_value_of(value: bool) -> bool {
    value
}

#[must_use]
pub(crate) fn boolean_to_string(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

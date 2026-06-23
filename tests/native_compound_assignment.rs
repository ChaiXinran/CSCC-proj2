//! Native execution coverage for compound assignment and computed object keys.

use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn compound_assignment_updates_identifier_and_returns_value() {
    assert_eq!(native_eval("var x = 5; var y = (x += 3); y + x;"), "16");
}

#[test]
fn compound_assignment_supports_numeric_operators() {
    assert_eq!(
        native_eval("var x = 20; x -= 3; x *= 2; x /= 4; x %= 5; x;"),
        "3.5"
    );
}

#[test]
fn compound_assignment_updates_static_member() {
    assert_eq!(native_eval("var o = { x: 2 }; o.x *= 5; o.x;"), "10");
}

#[test]
fn compound_assignment_updates_computed_member_once() {
    assert_eq!(
        native_eval(
            "var hits = 0; var o = { x: 2 }; \
             function key() { hits += 1; return 'x'; } \
             o[key()] += 3; hits + ':' + o.x;"
        ),
        "1:5"
    );
}

#[test]
fn object_literal_computed_key_uses_runtime_key() {
    assert_eq!(
        native_eval("var key = 'ab'; var o = { [key + 'c']: 7 }; o.abc;"),
        "7"
    );
}

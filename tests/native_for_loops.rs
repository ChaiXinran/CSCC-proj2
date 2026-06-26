//! Native front-end tests for `for`, `for-in`, and `++`/`--` update operators.

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

// ── Update operators ─────────────────────────────────────────────────────────

#[test]
fn postfix_increment_returns_old_value_then_updates() {
    assert_eq!(native_eval("var i = 5; var j = i++; j;"), "5");
    assert_eq!(native_eval("var i = 5; i++; i;"), "6");
}

#[test]
fn prefix_increment_returns_new_value() {
    assert_eq!(native_eval("var i = 5; var j = ++i; j;"), "6");
}

#[test]
fn decrement_operators() {
    assert_eq!(native_eval("var i = 5; i--; i;"), "4");
    assert_eq!(native_eval("var i = 5; var j = --i; j;"), "4");
}

#[test]
fn increment_coerces_to_number() {
    // ToNumber semantics: a numeric string is incremented as a number.
    assert_eq!(native_eval("var i = '5'; i++; i;"), "6");
}

#[test]
fn postfix_computed_member_update_returns_old_value() {
    assert_eq!(native_eval("var a = [1]; var old = a[0]++; old + a[0];"), "3");
}

#[test]
fn destructuring_assignment_updates_existing_bindings() {
    assert_eq!(native_eval("var a = 0; var b = 0; [a, b] = [1, 2]; a + b;"), "3");
    assert_eq!(
        native_eval("var a = 0; var b = 0; ({ x: a, y: b } = { x: 4, y: 5 }); a + b;"),
        "9"
    );
}

// ── C-style for ──────────────────────────────────────────────────────────────

#[test]
fn classic_for_accumulates_with_increment() {
    assert_eq!(
        native_eval("var s = 0; for (var i = 0; i < 5; i++) { s = s + i; } s;"),
        "10"
    );
}

#[test]
fn classic_for_with_let_binding() {
    assert_eq!(
        native_eval("var s = 0; for (let i = 0; i < 4; i++) { s = s + i; } s;"),
        "6"
    );
}

#[test]
fn for_break_exits_loop() {
    assert_eq!(
        native_eval(
            "var s = 0; for (var i = 0; i < 10; i++) { if (i === 5) { break; } s = s + i; } s;"
        ),
        "10"
    );
}

#[test]
fn for_continue_skips_to_update() {
    // `continue` must run the update clause, so this terminates and skips i==2.
    assert_eq!(
        native_eval(
            "var s = 0; for (var i = 0; i < 5; i++) { if (i === 2) { continue; } s = s + i; } s;"
        ),
        "8"
    );
}

#[test]
fn for_with_empty_clauses() {
    assert_eq!(native_eval("var i = 0; for (; i < 3;) { i++; } i;"), "3");
}

#[test]
fn nested_for_loops_count() {
    assert_eq!(
        native_eval(
            "var c = 0; for (var i = 0; i < 3; i++) { for (var j = 0; j < 3; j++) { c = c + 1; } } c;"
        ),
        "9"
    );
}

#[test]
fn for_init_expression_statement() {
    assert_eq!(
        native_eval("var i; var s = 0; for (i = 0; i < 3; i++) { s = s + i; } s;"),
        "3"
    );
}

// ── for-in ───────────────────────────────────────────────────────────────────

#[test]
fn for_in_iterates_object_keys_in_insertion_order() {
    assert_eq!(
        native_eval("var o = { a: 1, b: 2, c: 3 }; var r = ''; for (var k in o) { r = r + k; } r;"),
        "abc"
    );
}

#[test]
fn for_in_iterates_array_indices() {
    assert_eq!(
        native_eval("var a = [10, 20, 30]; var r = ''; for (var i in a) { r = r + i; } r;"),
        "012"
    );
}

#[test]
fn for_in_without_declaration_uses_existing_binding() {
    assert_eq!(
        native_eval("var o = { x: 1 }; var k; for (k in o) {} k;"),
        "x"
    );
}

#[test]
fn for_in_sums_values_via_member_access() {
    assert_eq!(
        native_eval(
            "var o = { a: 1, b: 2, c: 3 }; var s = 0; for (var k in o) { s = s + o[k]; } s;"
        ),
        "6"
    );
}

#[test]
fn for_in_break_and_continue() {
    assert_eq!(
        native_eval(
            "var o = { a: 1, b: 2, c: 3 }; var r = ''; \
             for (var k in o) { if (k === 'b') { continue; } r = r + k; } r;"
        ),
        "ac"
    );
}

#[test]
fn for_in_walks_prototype_chain() {
    // Object.create gives `child` an inherited enumerable `x`.
    assert_eq!(
        native_eval(
            "var parent = { x: 1 }; var child = Object.create(parent); child.y = 2; \
             var r = ''; for (var k in child) { r = r + k; } r;"
        ),
        "yx"
    );
}

// ── Regression: while loops still work alongside ++ ──────────────────────────

#[test]
fn while_loop_with_increment_still_works() {
    assert_eq!(native_eval("var i = 0; while (i < 3) { i++; } i;"), "3");
}

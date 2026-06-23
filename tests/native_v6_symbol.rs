//! Track B — Symbol infrastructure, ToPrimitive, and Symbol.toStringTag tests.

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

fn native_eval_err(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_err()
        .to_string()
}

// ── typeof ────────────────────────────────────────────────────────────────────

#[test]
fn typeof_symbol_is_symbol() {
    assert_eq!(native_eval("typeof Symbol()"), "symbol");
    assert_eq!(native_eval("typeof Symbol('x')"), "symbol");
}

// ── identity / equality ───────────────────────────────────────────────────────

#[test]
fn two_symbol_calls_produce_distinct_values() {
    assert_eq!(native_eval("Symbol() === Symbol()"), "false");
    assert_eq!(native_eval("Symbol('a') === Symbol('a')"), "false");
}

#[test]
fn same_symbol_reference_is_strictly_equal_to_itself() {
    assert_eq!(native_eval("var s = Symbol(); s === s"), "true");
}

// ── description property ──────────────────────────────────────────────────────

#[test]
fn symbol_description_returns_the_provided_string() {
    assert_eq!(native_eval("Symbol('hello').description"), "hello");
}

#[test]
fn symbol_description_is_undefined_when_no_argument() {
    assert_eq!(native_eval("Symbol().description"), "undefined");
}

// ── Symbol.prototype.toString ─────────────────────────────────────────────────

#[test]
fn symbol_to_string_includes_description() {
    assert_eq!(native_eval("Symbol('foo').toString()"), "Symbol(foo)");
}

#[test]
fn symbol_to_string_with_no_description() {
    assert_eq!(native_eval("Symbol().toString()"), "Symbol()");
}

// ── coercion errors ───────────────────────────────────────────────────────────

#[test]
fn symbol_to_number_throws_typeerror() {
    let e = native_eval_err("var s = Symbol(); +s");
    assert!(e.contains("TypeError") || e.contains("Symbol"), "got: {e}");
}

#[test]
fn symbol_implicit_string_concat_throws_typeerror() {
    let e = native_eval_err("'' + Symbol()");
    assert!(e.contains("TypeError") || e.contains("Symbol"), "got: {e}");
}

// ── well-known symbols on Symbol constructor ──────────────────────────────────

#[test]
fn symbol_to_primitive_is_a_symbol() {
    assert_eq!(native_eval("typeof Symbol.toPrimitive"), "symbol");
}

#[test]
fn symbol_to_string_tag_is_a_symbol() {
    assert_eq!(native_eval("typeof Symbol.toStringTag"), "symbol");
}

#[test]
fn symbol_iterator_is_a_symbol() {
    assert_eq!(native_eval("typeof Symbol.iterator"), "symbol");
}

#[test]
fn well_known_symbols_are_distinct_from_each_other() {
    assert_eq!(
        native_eval("Symbol.toPrimitive === Symbol.toStringTag"),
        "false"
    );
    assert_eq!(
        native_eval("Symbol.iterator === Symbol.hasInstance"),
        "false"
    );
}

// ── Object.prototype.toString with Symbol.toStringTag ────────────────────────

#[test]
fn object_to_string_for_plain_object() {
    assert_eq!(
        native_eval("Object.prototype.toString.call({})"),
        "[object Object]"
    );
}

#[test]
fn object_to_string_for_array() {
    assert_eq!(
        native_eval("Object.prototype.toString.call([])"),
        "[object Array]"
    );
}

#[test]
fn object_to_string_for_number_wrapper() {
    assert_eq!(
        native_eval("Object.prototype.toString.call(new Number(0))"),
        "[object Number]"
    );
}

#[test]
fn object_to_string_for_boolean_wrapper() {
    assert_eq!(
        native_eval("Object.prototype.toString.call(new Boolean(false))"),
        "[object Boolean]"
    );
}

#[test]
fn object_to_string_for_string_wrapper() {
    assert_eq!(
        native_eval("Object.prototype.toString.call(new String(''))"),
        "[object String]"
    );
}

#[test]
fn object_to_string_for_null() {
    assert_eq!(
        native_eval("Object.prototype.toString.call(null)"),
        "[object Null]"
    );
}

#[test]
fn object_to_string_for_undefined() {
    assert_eq!(
        native_eval("Object.prototype.toString.call(undefined)"),
        "[object Undefined]"
    );
}

// ── custom Symbol.toStringTag ─────────────────────────────────────────────────

#[test]
fn custom_to_string_tag_overrides_object_tag() {
    let r = native_eval(
        r#"var obj = {};
Object.defineProperty(obj, Symbol.toStringTag, { value: 'MyType', writable: false, enumerable: false, configurable: false });
Object.prototype.toString.call(obj)"#,
    );
    assert_eq!(r, "[object MyType]");
}

// ── JSON.stringify ignores Symbol values ──────────────────────────────────────

#[test]
fn json_stringify_symbol_value_produces_undefined() {
    assert_eq!(native_eval("JSON.stringify(Symbol('x'))"), "undefined");
}

#[test]
fn json_stringify_object_with_symbol_value_omits_the_entry() {
    assert_eq!(native_eval("JSON.stringify({ a: Symbol() })"), "{}");
}

// ── new Symbol() is not allowed ───────────────────────────────────────────────

#[test]
fn new_symbol_throws_typeerror() {
    let e = native_eval_err("new Symbol()");
    assert!(e.contains("TypeError") || e.contains("constructor"), "got: {e}");
}

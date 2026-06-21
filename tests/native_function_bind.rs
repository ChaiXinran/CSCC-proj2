//! Tests for `Function.prototype.bind`, including the `call.bind(method)`
//! pattern that Test262's `propertyHelper.js` relies on.

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
fn bind_sets_this() {
    assert_eq!(
        native_eval("function f() { return this.x; } var o = { x: 42 }; var g = f.bind(o); g();"),
        "42"
    );
}

#[test]
fn bind_prepends_bound_arguments() {
    assert_eq!(
        native_eval("function add(a, b) { return a + b; } var add5 = add.bind(null, 5); add5(3);"),
        "8"
    );
}

#[test]
fn bind_prepends_then_appends_call_arguments() {
    assert_eq!(
        native_eval(
            "function join3(a, b, c) { return a + b + c; } \
             var f = join3.bind(null, 1, 2); f(3);"
        ),
        "6"
    );
}

#[test]
fn call_bind_join_pattern() {
    // The propertyHelper.js idiom: borrow a prototype method via call.bind.
    assert_eq!(
        native_eval(
            "var join = Function.prototype.call.bind(Array.prototype.join); join([1, 2, 3], '-');"
        ),
        "1-2-3"
    );
}

#[test]
fn call_bind_hasownproperty_pattern() {
    assert_eq!(
        native_eval(
            "var hop = Function.prototype.call.bind(Object.prototype.hasOwnProperty); \
             var o = { a: 1 }; hop(o, 'a');"
        ),
        "true"
    );
    assert_eq!(
        native_eval(
            "var hop = Function.prototype.call.bind(Object.prototype.hasOwnProperty); \
             var o = { a: 1 }; hop(o, 'b');"
        ),
        "false"
    );
}

#[test]
fn bound_function_used_as_constructor() {
    assert_eq!(
        native_eval(
            "function Point(x) { this.x = x; } var P = Point.bind(null); var p = new P(7); p.x;"
        ),
        "7"
    );
}

#[test]
fn bind_is_a_function_property_only() {
    // bind lives on Function.prototype; ordinary functions inherit it.
    assert_eq!(native_eval("typeof (function () {}).bind;"), "function");
}

#[test]
fn property_helper_verify_property_runs() {
    // End-to-end: define and use a verifyProperty-style check that exercises
    // call.bind, hasOwnProperty, and Object.getOwnPropertyDescriptor together.
    assert_eq!(
        native_eval(
            "var hasOwn = Function.prototype.call.bind(Object.prototype.hasOwnProperty); \
             var o = {}; \
             Object.defineProperty(o, 'k', { value: 1, writable: false, enumerable: false, configurable: false }); \
             var d = Object.getOwnPropertyDescriptor(o, 'k'); \
             var ok = hasOwn(o, 'k') && d.value === 1 && d.writable === false && d.enumerable === false; \
             ok;"
        ),
        "true"
    );
}

//! Tests for the `arguments` exotic object inside user-defined functions.
//!
//! These cover the patterns exercised by Test262's propertyHelper.js and the
//! bulk of the ~280 argument-blocked cases: length, indexed access, rest-like
//! spreading, and forwarding arguments between callers.

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

// ── Basic length and indexed access ──────────────────────────────────────────

#[test]
fn arguments_length_reflects_actual_call_arg_count() {
    assert_eq!(
        native_eval("function f(a, b, c) { return arguments.length; } f(1, 2);"),
        "2"
    );
}

#[test]
fn arguments_zero_length_when_called_with_no_args() {
    assert_eq!(
        native_eval("function f() { return arguments.length; } f();"),
        "0"
    );
}

#[test]
fn arguments_indexed_access_returns_correct_values() {
    assert_eq!(
        native_eval("function f() { return arguments[0] + arguments[1]; } f(3, 4);"),
        "7"
    );
}

#[test]
fn arguments_extra_args_beyond_params_are_accessible() {
    assert_eq!(
        native_eval("function f(a) { return arguments[1]; } f('x', 'y');"),
        "y"
    );
}

#[test]
fn arguments_length_counts_extra_args() {
    assert_eq!(
        native_eval("function f(a, b) { return arguments.length; } f(1, 2, 3, 4);"),
        "4"
    );
}

// ── arguments in nested calls ─────────────────────────────────────────────────

#[test]
fn arguments_is_local_to_each_function() {
    assert_eq!(
        native_eval(
            "function inner() { return arguments[0]; }
             function outer(x) { return inner(x + 1); }
             outer(10);"
        ),
        "11"
    );
}

#[test]
fn arguments_forwarding_to_another_function() {
    assert_eq!(
        native_eval(
            "function sum() {
                 var total = 0;
                 for (var i = 0; i < arguments.length; i++) {
                     total = total + arguments[i];
                 }
                 return total;
             }
             sum(1, 2, 3, 4, 5);"
        ),
        "15"
    );
}

// ── typeof and hasOwnProperty ─────────────────────────────────────────────────

#[test]
fn typeof_arguments_is_object() {
    assert_eq!(
        native_eval("function f() { return typeof arguments; } f();"),
        "object"
    );
}

#[test]
fn arguments_has_own_property_for_each_index() {
    assert_eq!(
        native_eval(
            "function f() {
                 return Object.prototype.hasOwnProperty.call(arguments, '0') &&
                        Object.prototype.hasOwnProperty.call(arguments, '1');
             }
             f('a', 'b');"
        ),
        "true"
    );
}

#[test]
fn arguments_has_own_property_length() {
    assert_eq!(
        native_eval(
            "function f() {
                 return Object.prototype.hasOwnProperty.call(arguments, 'length');
             }
             f(1);"
        ),
        "true"
    );
}

// ── propertyHelper.js-style verifyProperty pattern ───────────────────────────

#[test]
fn verify_property_pattern_with_arguments_length() {
    // Mimics what propertyHelper.js does after bind was fixed:
    // use call.bind + hasOwnProperty to check a property descriptor.
    assert_eq!(
        native_eval(
            "var hasOwn = Function.prototype.call.bind(Object.prototype.hasOwnProperty);
             function f(a, b) {
                 return hasOwn(arguments, 'length') && arguments.length === 2;
             }
             f(1, 2);"
        ),
        "true"
    );
}

// ── arguments with varargs patterns ──────────────────────────────────────────

#[test]
fn variadic_max_using_arguments() {
    assert_eq!(
        native_eval(
            "function max() {
                 var result = -Infinity;
                 for (var i = 0; i < arguments.length; i++) {
                     if (arguments[i] > result) result = arguments[i];
                 }
                 return result;
             }
             max(3, 1, 4, 1, 5, 9, 2, 6);"
        ),
        "9"
    );
}

#[test]
fn arguments_object_is_not_an_array() {
    // arguments is not an Array instance — it doesn't have Array.prototype methods
    // directly; typeof is "object" but it's not === Array.prototype.
    assert_eq!(
        native_eval("function f() { return Array.isArray(arguments); } f(1, 2, 3);"),
        "false"
    );
}

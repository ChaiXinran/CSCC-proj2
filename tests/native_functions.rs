use agentjs::{BackendKind, Engine, ExecutionOptions, FailureKind, RuntimeConfig};

fn native_engine() -> Engine {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
}

fn eval(source: &str) -> String {
    native_engine()
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("V3 source should execute: {error}"))
        .value
}

#[test]
fn executes_functions_parameters_and_returns() {
    assert_eq!(eval("function add(a, b) { return a + b; } add(1, 2);"), "3");
    assert_eq!(eval("function id(x) { return x; } id('agent');"), "agent");
    assert_eq!(eval("function f() { return; } f();"), "undefined");
    assert_eq!(eval("function f() { var x = 1; return x + 2; } f();"), "3");
}

#[test]
fn isolates_locals_and_executes_basic_closures() {
    assert_eq!(
        eval("var x = 1; function f() { var x = 2; return x; } f() + x;"),
        "3"
    );
    assert_eq!(
        eval(
            "function outer(x) { \
             function inner(y) { return x + y; } \
             return inner(2); \
             } outer(1);"
        ),
        "3"
    );
}

#[test]
fn executes_objects_arrays_and_member_assignment() {
    assert_eq!(eval("var obj = { a: 1, b: 2 }; obj.a + obj['b'];"), "3");
    assert_eq!(eval("var arr = [1, 2, 3]; arr[0] + arr.length;"), "4");
    assert_eq!(eval("var obj = { x: 1 }; obj.x = 5; obj['x'];"), "5");
    assert_eq!(eval("var arr = [1]; arr[0] = 9; arr[0];"), "9");
}

#[test]
fn preserves_this_for_member_calls() {
    assert_eq!(
        eval(
            "var obj = { \
             value: 7, \
             get: function () { return this.value; } \
             }; obj.get();"
        ),
        "7"
    );
}

#[test]
fn direct_eval_resolves_function_locals() {
    assert_eq!(
        eval("function f() { var x = 5; return eval('x + 2'); } f();"),
        "7"
    );
    assert_eq!(
        eval(
            "function f() { \
             function Y() { return 1; } \
             var ia = ['Y']; \
             return eval(ia[0] + '()'); \
             } f();"
        ),
        "1"
    );
    assert_eq!(
        eval("function f() { eval('var y = 11'); return y; } f();"),
        "11"
    );
}

#[test]
fn function_name_does_not_overwrite_parameters_or_arguments() {
    assert_eq!(eval("function f(x, f) { return x * f; } f(3, 4);"), "12");
    assert_eq!(
        eval("function arguments() { return typeof arguments + ':' + arguments.length; } arguments(1, 2);"),
        "object:2"
    );
}

#[test]
fn executes_checked_in_v3_example() {
    assert_eq!(eval(include_str!("../examples/v3.js")), "5");
}

#[test]
fn rejects_invalid_calls_and_limits_recursion() {
    let not_callable = native_engine()
        .execute("var f = 1; f();", ExecutionOptions::default())
        .unwrap_err();
    assert_eq!(not_callable.kind, FailureKind::Type);

    let engine = Engine::with_backend(
        BackendKind::Native,
        RuntimeConfig {
            recursion_limit: 8,
            ..RuntimeConfig::default()
        },
    );
    let recursion = engine
        .execute(
            "function recurse() { return recurse(); } recurse();",
            ExecutionOptions::default(),
        )
        .unwrap_err();
    assert_eq!(recursion.kind, FailureKind::RuntimeLimit);
}

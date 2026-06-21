use agentjs::{BackendKind, Engine, ExecutionOptions, RuntimeConfig};

fn eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("V4 source should execute: {error}"))
        .value
}

#[test]
fn executes_delete_in_and_prototype_lookup() {
    assert_eq!(
        eval(
            "var base = { x: 1 }; \
             var child = { __proto__: base, y: 2 }; \
             var deleted = delete child.y; \
             deleted && !(\"y\" in child) && (\"x\" in child);"
        ),
        "true"
    );
}

#[test]
fn executes_instanceof_for_user_constructors() {
    assert_eq!(
        eval(
            "function Point(x) { this.x = x; } \
             var point = new Point(3); \
             point instanceof Point;"
        ),
        "true"
    );
}

#[test]
fn executes_sparse_array_holes() {
    assert_eq!(
        eval(
            "var array = [1, , 3]; \
             array.length === 3 && !(1 in array) && array[2] === 3;"
        ),
        "true"
    );
}

#[test]
fn executes_array_constructor_and_minimal_array_methods() {
    assert_eq!(
        eval(
            "var array = Array(1, 2); \
             var pushed = array.push(3); \
             var popped = array.pop(); \
             Array.isArray(array) && pushed === 3 && popped === 3 && array.length === 2;"
        ),
        "true"
    );
}

#[test]
fn executes_array_constructor_length_form_with_holes() {
    assert_eq!(
        eval(
            "var array = new Array(3); \
             Array.isArray(array) && array.length === 3 && !(0 in array);"
        ),
        "true"
    );
}

#[test]
fn executes_object_literal_accessors() {
    assert_eq!(
        eval(
            "var object = { \
               get x() { return 7; }, \
               set x(value) { this.saved = value; } \
             }; \
             object.x = 4; \
             object.x + object.saved;"
        ),
        "11"
    );
}

#[test]
fn executes_function_prototype_call() {
    assert_eq!(
        eval(
            "function read() { return this.x; } \
             var object = { x: 7 }; \
             read.call(object);"
        ),
        "7"
    );
}

#[test]
fn exposes_function_and_array_prototype_relationships() {
    assert_eq!(
        eval(
            "function Point() {} \
             var point = new Point(); \
             var array = []; \
             (point instanceof Point) && \
             Point.prototype.constructor === Point && \
             Object.getPrototypeOf(Point) === Function.prototype && \
             Object.getPrototypeOf(array) === Array.prototype;"
        ),
        "true"
    );
}

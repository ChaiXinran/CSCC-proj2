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

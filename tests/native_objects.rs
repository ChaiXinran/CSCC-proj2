use agentjs::{BackendKind, Engine, ExecutionOptions, FailureKind, RuntimeConfig};

fn eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("V4 source should execute: {error}"))
        .value
}

fn eval_error(source: &str) -> FailureKind {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect_err("V4 source should fail")
        .kind
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
fn class_field_named_get_is_not_parsed_as_getter() {
    assert_eq!(
        eval(
            "class P { \
             get = () => '123'; \
             static() { return 42; } \
             } \
             var p = new P(); \
             p.get() + ':' + p.static();"
        ),
        "123:42"
    );
}

#[test]
fn object_is_uses_same_value_semantics() {
    assert_eq!(
        eval("Object.is(NaN, NaN) + ':' + Object.is(0, -0) + ':' + Object.is(undefined, undefined);"),
        "true:false:true"
    );
}

#[test]
fn delete_property_coerces_primitives_but_rejects_nullish() {
    assert_eq!(eval("delete 'abc'[100];"), "true");
    assert_eq!(eval("delete 'abc'[0];"), "false");
    assert_eq!(
        eval("var ok = false; try { delete null.a; } catch (e) { ok = e instanceof TypeError; } ok;"),
        "true"
    );
}

#[test]
fn delete_super_property_throws_reference_error() {
    assert_eq!(
        eval("var ok = false; var a = { f() { delete super.a; } }; try { a.f(); } catch (e) { ok = e instanceof ReferenceError; } ok;"),
        "true"
    );
}

#[test]
fn accessor_function_names_include_get_or_set_prefix() {
    assert_eq!(
        eval("class C { get y() { return 1; } } Object.getOwnPropertyDescriptor(C.prototype, 'y').get.name;"),
        "get y"
    );
    assert_eq!(
        eval("var o = { get x() { return 1; } }; Object.getOwnPropertyDescriptor(o, 'x').get.name;"),
        "get x"
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
fn object_define_property_obeys_descriptor_edges() {
    assert_eq!(
        eval(
            "var object = {}; \
             var called = 0; \
             var descriptor = { get value() { called = 1; return 10; } }; \
             Object.defineProperty(object, 'x', descriptor); \
             called + object.x;"
        ),
        "11"
    );

    assert_eq!(
        eval_error(
            "var object = {}; Object.defineProperty(object, 'x', { value: 1, get: Object });"
        ),
        FailureKind::Type
    );
    assert_eq!(
        eval_error(
            "var object = {}; \
             function first() { return 1; } \
             function second() { return 2; } \
             Object.defineProperty(object, 'x', { get: first, configurable: false }); \
             Object.defineProperty(object, 'x', { get: second });"
        ),
        FailureKind::Type
    );
}

#[test]
fn object_create_uses_only_enumerable_descriptor_map_entries() {
    assert_eq!(
        eval(
            "var descriptors = {}; \
             Object.defineProperty(descriptors, 'hidden', { value: { value: 1 }, enumerable: false }); \
             var object = Object.create(null, descriptors); \
             !('hidden' in object);"
        ),
        "true"
    );
}

#[test]
fn array_descriptors_preserve_flags_and_length_writable() {
    assert_eq!(
        eval(
            "var array = []; \
             Object.defineProperty(array, '0', { value: 1, writable: false, enumerable: false, configurable: false }); \
             var descriptor = Object.getOwnPropertyDescriptor(array, '0'); \
             descriptor.value === 1 && \
             descriptor.writable === false && \
             descriptor.enumerable === false && \
             descriptor.configurable === false && \
             delete array[0] === false && \
             array[0] === 1;"
        ),
        "true"
    );

    assert_eq!(
        eval(
            "var array = [1, 2]; \
             Object.defineProperty(array, 'length', { writable: false }); \
             Object.getOwnPropertyDescriptor(array, 'length').writable === false && array.length === 2;"
        ),
        "true"
    );
    assert_eq!(
        eval_error(
            "var array = [1, 2]; \
             Object.defineProperty(array, 'length', { writable: false }); \
             array.push(3);"
        ),
        FailureKind::Type
    );
}

#[test]
fn saved_function_prototype_call_stays_callable_after_prototype_overwrite() {
    assert_eq!(
        eval(
            "function read() { return this.x; } \
             var saved = Function.prototype.call; \
             Function.prototype.call = Object; \
             read.saved = saved; \
             read.saved({ x: 9 });"
        ),
        "9"
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

#[test]
fn executes_class_static_blocks_with_class_this_and_source_order() {
    assert_eq!(
        eval("var value; class C { static { value = this; } } value === C;"),
        "true"
    );
    assert_eq!(
        eval(
            "var seq = ''; \
             class C { \
               static x = seq = seq + 'a'; \
               static { seq = seq + 'b'; } \
               static y = seq = seq + 'c'; \
             } \
             seq;"
        ),
        "abc"
    );
}

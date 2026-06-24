//! V6 runtime foundation tests: PrimitiveWrapper, Intrinsics V6 fields, coercion.

use agentjs::{
    backend::BackendKind,
    builtins::install_foundation,
    bytecode::{Chunk, Constant, Instruction},
    engine::{Engine, ExecutionOptions, RuntimeConfig},
    runtime::{JsValue, NativeContext, ObjectKind, PrimitiveValue},
    vm::Vm,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ctx() -> NativeContext {
    let mut c = NativeContext::default();
    install_foundation(&mut c);
    c
}

fn str_const(chunk: &mut Chunk, v: &str) -> u16 {
    chunk.add_constant(Constant::String(v.into())).unwrap()
}

fn run(chunk: Chunk, context: &mut NativeContext) -> JsValue {
    Vm::default()
        .execute_with_context(&chunk, context)
        .expect("chunk execution should succeed")
}

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|e| panic!("native eval failed: {e}"))
        .value
}

// ── Intrinsics ────────────────────────────────────────────────────────────────

#[test]
fn v6_intrinsics_are_populated_after_install_foundation() {
    let ctx = ctx();
    let intrinsics = ctx.intrinsics().expect("intrinsics must be present");
    assert!(
        ctx.number_prototype().is_some(),
        "number_prototype should be set"
    );
    assert!(
        ctx.boolean_prototype().is_some(),
        "boolean_prototype should be set"
    );
    assert!(
        ctx.string_prototype().is_some(),
        "string_prototype should be set"
    );
    assert!(
        ctx.error_prototype().is_some(),
        "error_prototype should be set"
    );
    // V6 intrinsics are distinct from each other.
    let np = intrinsics.number_prototype;
    let bp = intrinsics.boolean_prototype;
    let sp = intrinsics.string_prototype;
    let ep = intrinsics.error_prototype;
    assert_ne!(np, bp);
    assert_ne!(np, sp);
    assert_ne!(np, ep);
    assert_ne!(bp, sp);
}

// ── PrimitiveWrapper prototype kinds ─────────────────────────────────────────

#[test]
fn number_prototype_is_a_primitive_wrapper_with_default_zero() {
    let ctx = ctx();
    let proto_id = ctx.number_prototype().unwrap();
    let pv = ctx.primitive_value(proto_id);
    assert_eq!(pv, Some(&PrimitiveValue::Number(0.0)));
}

#[test]
fn boolean_prototype_is_a_primitive_wrapper_with_default_false() {
    let ctx = ctx();
    let proto_id = ctx.boolean_prototype().unwrap();
    let pv = ctx.primitive_value(proto_id);
    assert_eq!(pv, Some(&PrimitiveValue::Boolean(false)));
}

#[test]
fn string_prototype_is_a_primitive_wrapper_with_default_empty_string() {
    let ctx = ctx();
    let proto_id = ctx.string_prototype().unwrap();
    let pv = ctx.primitive_value(proto_id);
    assert_eq!(pv, Some(&PrimitiveValue::String(String::new())));
}

#[test]
fn error_prototype_is_an_ordinary_object_not_a_wrapper() {
    let ctx = ctx();
    let proto_id = ctx.error_prototype().unwrap();
    // error_prototype must NOT carry a PrimitiveValue.
    assert_eq!(ctx.primitive_value(proto_id), None);
    // It should have the ordinary Object kind.
    let obj = ctx.heap().object(proto_id).expect("object must exist");
    assert!(
        matches!(obj.kind, ObjectKind::Ordinary),
        "Error.prototype must be an ordinary object"
    );
}

// ── create_primitive_wrapper ──────────────────────────────────────────────────

#[test]
fn create_primitive_wrapper_stores_number_value_and_prototype() {
    let mut ctx = ctx();
    let proto = ctx.number_prototype().unwrap();
    let wrapper = ctx
        .create_primitive_wrapper(PrimitiveValue::Number(42.0), proto)
        .unwrap();
    let JsValue::Object(id) = wrapper else {
        panic!("expected Object variant");
    };
    assert_eq!(ctx.primitive_value(id), Some(&PrimitiveValue::Number(42.0)));
    assert_eq!(
        ctx.heap().object(id).unwrap().prototype,
        Some(proto),
        "prototype chain must point to Number.prototype"
    );
}

#[test]
fn create_primitive_wrapper_stores_boolean_value_and_prototype() {
    let mut ctx = ctx();
    let proto = ctx.boolean_prototype().unwrap();
    let wrapper = ctx
        .create_primitive_wrapper(PrimitiveValue::Boolean(true), proto)
        .unwrap();
    let JsValue::Object(id) = wrapper else {
        panic!("expected Object variant");
    };
    assert_eq!(
        ctx.primitive_value(id),
        Some(&PrimitiveValue::Boolean(true))
    );
    assert_eq!(ctx.heap().object(id).unwrap().prototype, Some(proto));
}

#[test]
fn create_primitive_wrapper_stores_string_value_and_prototype() {
    let mut ctx = ctx();
    let proto = ctx.string_prototype().unwrap();
    let wrapper = ctx
        .create_primitive_wrapper(PrimitiveValue::String("hello".into()), proto)
        .unwrap();
    let JsValue::Object(id) = wrapper else {
        panic!("expected Object variant");
    };
    assert_eq!(
        ctx.primitive_value(id),
        Some(&PrimitiveValue::String("hello".into()))
    );
    assert_eq!(ctx.heap().object(id).unwrap().prototype, Some(proto));
}

// ── new Number / Boolean / String construct ───────────────────────────────────

/// Build a chunk that runs `new <ctor_name>(arg)` and returns the result.
fn construct_chunk(ctor_name: &str, arg: Constant) -> (Chunk, NativeContext) {
    let ctx = ctx();
    let mut chunk = Chunk::default();
    let ctor = str_const(&mut chunk, ctor_name);
    let arg_idx = chunk.add_constant(arg).unwrap();
    chunk.emit(Instruction::LoadGlobal(ctor));
    chunk.emit(Instruction::Constant(arg_idx));
    chunk.emit(Instruction::Construct(1));
    chunk.emit(Instruction::Return);
    (chunk, ctx)
}

#[test]
fn number_construct_creates_primitive_wrapper_with_value() {
    let (chunk, mut ctx) = construct_chunk("Number", Constant::Number(99.0));
    let result = run(chunk, &mut ctx);
    let JsValue::Object(id) = result else {
        panic!("expected Object from new Number(99)");
    };
    assert_eq!(ctx.primitive_value(id), Some(&PrimitiveValue::Number(99.0)));
    // Prototype must be Number.prototype.
    let expected_proto = ctx.number_prototype().unwrap();
    assert_eq!(
        ctx.heap().object(id).unwrap().prototype,
        Some(expected_proto)
    );
}

#[test]
fn boolean_construct_creates_primitive_wrapper_with_value() {
    let (chunk, mut ctx) = construct_chunk("Boolean", Constant::Boolean(true));
    let result = run(chunk, &mut ctx);
    let JsValue::Object(id) = result else {
        panic!("expected Object from new Boolean(true)");
    };
    assert_eq!(
        ctx.primitive_value(id),
        Some(&PrimitiveValue::Boolean(true))
    );
    let expected_proto = ctx.boolean_prototype().unwrap();
    assert_eq!(
        ctx.heap().object(id).unwrap().prototype,
        Some(expected_proto)
    );
}

#[test]
fn string_construct_creates_primitive_wrapper_with_value() {
    let (chunk, mut ctx) = construct_chunk("String", Constant::String("js".into()));
    let result = run(chunk, &mut ctx);
    let JsValue::Object(id) = result else {
        panic!("expected Object from new String('js')");
    };
    assert_eq!(
        ctx.primitive_value(id),
        Some(&PrimitiveValue::String("js".into()))
    );
    let expected_proto = ctx.string_prototype().unwrap();
    assert_eq!(
        ctx.heap().object(id).unwrap().prototype,
        Some(expected_proto)
    );
}

#[test]
fn string_construct_distinguishes_missing_from_undefined_argument() {
    assert_eq!(native_eval("new String().valueOf()"), "");
    assert_eq!(native_eval("new String(undefined).valueOf()"), "undefined");
}

// ── valueOf on wrapper objects ────────────────────────────────────────────────

/// Build a chunk: `new <ctor>(arg).valueOf()`.
fn valueof_chunk(ctor_name: &str, arg: Constant) -> (Chunk, NativeContext) {
    let ctx = ctx();
    let mut chunk = Chunk::default();
    let ctor = str_const(&mut chunk, ctor_name);
    let arg_idx = chunk.add_constant(arg).unwrap();
    let valueof = str_const(&mut chunk, "valueOf");
    chunk.emit(Instruction::LoadGlobal(ctor));
    chunk.emit(Instruction::Constant(arg_idx));
    chunk.emit(Instruction::Construct(1));
    chunk.emit(Instruction::GetMethod(valueof));
    chunk.emit(Instruction::CallWithThis(0));
    chunk.emit(Instruction::Return);
    (chunk, ctx)
}

#[test]
fn number_prototype_valueof_unwraps_wrapped_number() {
    let (chunk, mut ctx) = valueof_chunk("Number", Constant::Number(7.0));
    assert_eq!(run(chunk, &mut ctx), JsValue::Number(7.0));
}

#[test]
fn boolean_prototype_valueof_unwraps_wrapped_boolean() {
    let (chunk, mut ctx) = valueof_chunk("Boolean", Constant::Boolean(true));
    assert_eq!(run(chunk, &mut ctx), JsValue::Boolean(true));
}

#[test]
fn string_prototype_valueof_unwraps_wrapped_string() {
    let (chunk, mut ctx) = valueof_chunk("String", Constant::String("world".into()));
    assert_eq!(run(chunk, &mut ctx), JsValue::String("world".into()));
}

// ── Number() call coercion (now via vm.to_number) ─────────────────────────────

/// Run `Number(<arg>)` and return the numeric result.
fn number_call_result(arg: Constant) -> f64 {
    let mut ctx = ctx();
    let mut chunk = Chunk::default();
    let number = str_const(&mut chunk, "Number");
    let arg_idx = chunk.add_constant(arg).unwrap();
    chunk.emit(Instruction::LoadGlobal(number));
    chunk.emit(Instruction::Constant(arg_idx));
    chunk.emit(Instruction::Call(1));
    chunk.emit(Instruction::Return);
    let result = run(chunk, &mut ctx);
    match result {
        JsValue::Number(n) => n,
        other => panic!("expected Number, got {other:?}"),
    }
}

#[test]
fn number_call_coerces_null_to_zero() {
    let mut ctx = ctx();
    let mut chunk = Chunk::default();
    let number = str_const(&mut chunk, "Number");
    chunk.emit(Instruction::LoadGlobal(number));
    let null_idx = chunk.add_constant(Constant::Null).unwrap();
    chunk.emit(Instruction::Constant(null_idx));
    chunk.emit(Instruction::Call(1));
    chunk.emit(Instruction::Return);
    assert_eq!(run(chunk, &mut ctx), JsValue::Number(0.0));
}

#[test]
fn number_call_coerces_bool_true_to_one() {
    assert_eq!(number_call_result(Constant::Boolean(true)), 1.0);
}

#[test]
fn number_call_coerces_bool_false_to_zero() {
    assert_eq!(number_call_result(Constant::Boolean(false)), 0.0);
}

#[test]
fn number_call_coerces_numeric_string() {
    assert_eq!(number_call_result(Constant::String("42".into())), 42.0);
}

#[test]
fn number_call_on_non_numeric_string_gives_nan() {
    assert!(number_call_result(Constant::String("abc".into())).is_nan());
}

// ── String() call coercion (now via vm.to_string_coerce) ──────────────────────

/// Run `String(<arg>)` and return the string result.
fn string_call_result(arg: Constant) -> String {
    let mut ctx = ctx();
    let mut chunk = Chunk::default();
    let string_fn = str_const(&mut chunk, "String");
    let arg_idx = chunk.add_constant(arg).unwrap();
    chunk.emit(Instruction::LoadGlobal(string_fn));
    chunk.emit(Instruction::Constant(arg_idx));
    chunk.emit(Instruction::Call(1));
    chunk.emit(Instruction::Return);
    let result = run(chunk, &mut ctx);
    match result {
        JsValue::String(s) => s,
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn string_call_coerces_number() {
    assert_eq!(string_call_result(Constant::Number(42.0)), "42");
}

#[test]
fn string_call_coerces_bool() {
    assert_eq!(string_call_result(Constant::Boolean(true)), "true");
    assert_eq!(string_call_result(Constant::Boolean(false)), "false");
}

// ── Full coercion chain: object with custom valueOf ───────────────────────────

#[test]
fn number_call_invokes_custom_value_of_on_object() {
    // `Number({valueOf: function() { return 99; }})` should return 99.
    assert_eq!(
        native_eval("Number({ valueOf: function() { return 99; } });"),
        "99"
    );
}

#[test]
fn number_call_falls_back_to_to_string_when_value_of_absent() {
    // Object with no valueOf but has toString returning a numeric string.
    assert_eq!(
        native_eval("Number({ toString: function() { return '7'; } });"),
        "7"
    );
}

#[test]
fn string_call_invokes_custom_to_string_on_object() {
    assert_eq!(
        native_eval("String({ toString: function() { return 'hi'; } });"),
        "hi"
    );
}

#[test]
fn string_call_with_value_of_only_returns_object_object() {
    // Object.prototype.toString is in the prototype chain and returns "[object Object]",
    // so ToPrimitive(String) uses toString first — valueOf is never reached.
    // This is correct ECMAScript behaviour.
    assert_eq!(
        native_eval(
            r#"
            var obj = { valueOf: function() { return 'unreachable'; } };
            String(obj);
            "#
        ),
        "[object Object]"
    );
}

// ── Number() edge cases ───────────────────────────────────────────────────────

#[test]
fn number_call_with_no_args_returns_zero() {
    assert_eq!(native_eval("Number();"), "0");
}

#[test]
fn number_call_on_empty_string_returns_zero() {
    assert_eq!(native_eval("Number('');"), "0");
}

#[test]
fn number_call_on_infinity_string_returns_infinity() {
    assert_eq!(native_eval("Number('Infinity');"), "Infinity");
}

#[test]
fn number_call_rejects_non_canonical_infinity_spelling() {
    assert_eq!(native_eval("Number('INFINITY');"), "NaN");
    assert_eq!(native_eval("Number('inf');"), "NaN");
}

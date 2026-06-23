use std::time::Duration;

use agentjs::{
    backend::BackendKind,
    bytecode::{Chunk, Constant, Instruction},
    engine::{Engine, ExecutionOptions, FailureKind, RuntimeConfig},
    runtime::{JsValue, NativeContext},
    vm::{Vm, VmErrorKind},
};

fn number_constant(chunk: &mut Chunk, value: f64) -> u16 {
    chunk.add_constant(Constant::Number(value)).unwrap()
}

#[test]
fn wall_clock_deadline_reports_runtime_limit() {
    let engine = Engine::with_backend(
        BackendKind::Native,
        RuntimeConfig {
            wall_clock_limit: Some(Duration::from_nanos(0)),
            ..RuntimeConfig::default()
        },
    );

    let error = engine
        .execute("1 + 1", ExecutionOptions::default())
        .unwrap_err();
    assert_eq!(error.kind, FailureKind::RuntimeLimit);
}

#[test]
fn heap_byte_limit_guards_large_heap_values() {
    let mut context = NativeContext::with_heap_limits(128, 4 * 1024);
    let error = context
        .create_array(vec![JsValue::String("x".repeat(32 * 1024))])
        .unwrap_err();

    assert_eq!(error.kind, VmErrorKind::RuntimeLimit);
}

#[test]
fn string_repeat_allocation_limit_is_runtime_limit() {
    let engine = Engine::with_backend(BackendKind::Native, RuntimeConfig::default());
    let error = engine
        .execute("'x'.repeat(9000000);", ExecutionOptions::default())
        .unwrap_err();

    assert_eq!(error.kind, FailureKind::RuntimeLimit);
}

#[test]
fn stack_budget_rejects_high_stack_chunks_before_execution() {
    let mut chunk = Chunk::default();
    for value in 0..5 {
        let constant = number_constant(&mut chunk, value as f64);
        chunk.emit(Instruction::Constant(constant));
    }
    for _ in 0..4 {
        chunk.emit(Instruction::Pop);
    }
    chunk.emit(Instruction::Return);

    let mut context = NativeContext::default();
    context.reset_stack_limit(4);
    let error = Vm::default()
        .execute_with_context(&chunk, &mut context)
        .unwrap_err();

    assert_eq!(error.kind, VmErrorKind::RuntimeLimit);
}

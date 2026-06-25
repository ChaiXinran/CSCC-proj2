use agentjs::{
    bytecode::{Chunk, Constant, Instruction},
    runtime::{JsValue, NativeContext},
    vm::{Vm, VmErrorKind},
};

fn string(chunk: &mut Chunk, value: &str) -> u16 {
    chunk.add_constant(Constant::String(value.into())).unwrap()
}

#[test]
fn lexical_environment_bindings_track_initialization_state() {
    let mut context = NativeContext::default();
    let environment = context.current_environment();

    context
        .create_mutable_binding(environment, "name".into(), false)
        .unwrap();
    let error = context.resolve_binding_value("name").unwrap_err();
    assert_eq!(error.kind, VmErrorKind::Reference);

    context
        .initialize_binding(environment, "name", JsValue::Number(5.0))
        .unwrap();
    assert_eq!(
        context.resolve_binding_value("name").unwrap(),
        Some((environment, JsValue::Number(5.0)))
    );
}

#[test]
fn immutable_bindings_reject_assignment_after_initialization() {
    let mut context = NativeContext::default();
    let environment = context.current_environment();

    context
        .create_immutable_binding(environment, "fixed".into())
        .unwrap();
    context
        .initialize_binding(environment, "fixed", JsValue::Number(1.0))
        .unwrap();

    let error = context
        .set_binding("fixed", JsValue::Number(2.0))
        .unwrap_err();
    assert_eq!(error.kind, VmErrorKind::Type);
}

#[test]
fn vm_executes_catch_binding_from_handler_metadata() {
    let mut chunk = Chunk::default();
    let thrown = chunk.add_constant(Constant::Number(9.0)).unwrap();
    let name = string(&mut chunk, "error");

    let protected_start = chunk.current_offset();
    chunk.emit(Instruction::Constant(thrown));
    chunk.emit(Instruction::Throw);
    let protected_end = chunk.current_offset();

    let catch_target = chunk.current_offset();
    chunk.emit(Instruction::CreateLexicalEnvironment);
    chunk.emit(Instruction::CreateMutableBinding(name));
    chunk.emit(Instruction::LoadException);
    chunk.emit(Instruction::InitializeBinding(name));
    chunk.emit(Instruction::LoadName(name));
    chunk.emit(Instruction::Return);
    chunk.handlers.push(agentjs::bytecode::ExceptionHandler {
        start: protected_start,
        end: protected_end,
        target: catch_target,
        kind: agentjs::bytecode::HandlerKind::Catch,
        stack_depth: 0,
        environment_depth: 0,
    });

    assert_eq!(Vm::default().execute(&chunk).unwrap(), JsValue::Number(9.0));
}

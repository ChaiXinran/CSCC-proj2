use agentjs::{
    Engine, ExecutionOptions,
    bytecode::{ChunkError, Compiler, Instruction, UpvalueSlot},
    contracts::{ChunkExecutor, NativeFrontend, ProgramCompiler, SourceParser},
    runtime::NativeContext,
    vm::Vm,
};

fn compile(source: &str) -> agentjs::bytecode::SharedChunk {
    let program = NativeFrontend.parse_source(source).unwrap();
    ProgramCompiler::compile_program(&mut Compiler::new(), &program).unwrap()
}

#[test]
fn lowers_parent_reads_and_writes_to_upvalue_slots() {
    let chunk = compile(
        "function outer(value) { return function inner(step) { value = value + step; return value; }; }",
    );
    let inner = &chunk.functions[0].chunk.functions[0];
    assert_eq!(inner.upvalue_layout.bindings.len(), 1);
    assert_eq!(inner.upvalue_layout.bindings[0].name, "value");
    assert_eq!(
        inner.upvalue_layout.bindings[0].descriptor.environment_hops,
        0
    );
    assert!(
        inner
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadUpvalue(_)))
    );
    assert!(
        inner
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::StoreUpvalue(_)))
    );
}

#[test]
fn ancestor_and_block_created_closures_use_fixed_hops() {
    let chunk = compile(
        "function first(value) { return function second() { { return function third() { return value; }; } }; }",
    );
    let second = &chunk.functions[0].chunk.functions[0];
    let third = &second.chunk.functions[0];
    assert_eq!(third.upvalue_layout.bindings.len(), 1);
    assert_eq!(third.upvalue_layout.bindings[0].name, "value");
    assert_eq!(
        third.upvalue_layout.bindings[0].descriptor.environment_hops,
        1
    );

    let report = Engine::default()
        .execute(
            "function first(value) { return function second() { { return function third() { return value; }; } }; } var f=first(7)(); f();",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "7");
}

#[test]
fn dynamic_scope_deoptimizes_descendant_upvalues() {
    let eval_chunk = compile(
        "function outer(value) { eval('value = 5'); return function inner() { return value; }; }",
    );
    let inner = &eval_chunk.functions[0].chunk.functions[0];
    assert!(inner.upvalue_layout.bindings.is_empty());
    assert!(
        inner
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadName(_)))
    );

    let report = Engine::default()
        .execute(
            "function outer(value, object) { with (object) { return function () { return value; }; } } outer(1, {value: 9})();",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "9");
}

#[test]
fn arrows_generators_and_loop_created_closures_preserve_values() {
    let report = Engine::default()
        .execute(
            "function outer(value) { var callbacks=[]; for(var i=0;i<2;i=i+1){ callbacks.push(()=>value+i); } return callbacks; } var c=outer(10); c[0]()+c[1]();",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "24");

    let report = Engine::default()
        .execute(
            "function outer(value) { return function* () { yield value; value=value+1; yield value; }; } var g=outer(3)(); g.next().value+g.next().value;",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "7");

    let chunk = compile(
        "function outer(value) { return async function () { await Promise.resolve(0); return value; }; }",
    );
    let inner = &chunk.functions[0].chunk.functions[0];
    let await_offset = inner
        .chunk
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::AwaitValue))
        .unwrap();
    assert!(
        inner.chunk.instructions[await_offset + 1..]
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadUpvalue(_)))
    );
}

#[test]
fn block_lexicals_shadow_activation_upvalues_and_async_functions_capture() {
    let chunk = compile(
        "function outer(value) { { let value=9; return async function () { return value; }; } }",
    );
    let inner = &chunk.functions[0].chunk.functions[0];
    assert!(inner.upvalue_layout.bindings.is_empty());
    assert!(
        inner
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadName(_)))
    );

    let chunk = compile("function outer(value) { return async function () { return value; }; }");
    let inner = &chunk.functions[0].chunk.functions[0];
    assert_eq!(inner.upvalue_layout.bindings[0].name, "value");
    assert!(
        inner
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadUpvalue(_)))
    );
}

#[test]
fn function_body_lexicals_shadow_ancestor_upvalues() {
    let chunk = compile(
        "function first(value) { return function second() { let value=8; return function third() { return value; }; }; }",
    );
    let second = &chunk.functions[0].chunk.functions[0];
    let third = &second.chunk.functions[0];
    assert!(third.upvalue_layout.bindings.is_empty());
    assert!(
        third
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadName(_)))
    );
    let report = Engine::default()
        .execute(
            "function first(value) { return function second() { let value=8; return function third() { return value; }; }; } first(1)()();",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "8");
}

#[test]
fn eligible_closure_accesses_are_at_least_seventy_percent_upvalues() {
    let chunk = compile(
        "function outer(value) { return function inner() { value=value+1; value=value+1; return value+value; }; } outer(0)();",
    );
    let inner = &chunk.functions[0].chunk.functions[0];
    let upvalue = inner
        .chunk
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(
                instruction,
                Instruction::LoadUpvalue(_) | Instruction::StoreUpvalue(_)
            )
        })
        .count();
    let names = inner
        .chunk
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(
                instruction,
                Instruction::LoadName(_) | Instruction::StoreName(_)
            )
        })
        .count();
    assert!(upvalue * 100 >= (upvalue + names) * 70);

    let mut context = NativeContext::default();
    context.reset_name_resolution_metrics();
    ChunkExecutor::execute_chunk(&mut Vm::default(), &chunk, &mut context).unwrap();
    let metrics = context.name_resolution_metrics();
    let upvalue = metrics.load_upvalue_count + metrics.store_upvalue_count;
    let named = metrics.load_name_count + metrics.store_name_count;
    assert!(
        upvalue * 100 >= (upvalue + named) * 70,
        "metrics: {metrics:?}"
    );
}

#[test]
fn validation_rejects_an_out_of_bounds_upvalue_slot() {
    let mut chunk = (*compile("function outer(value){return function(){return value;};}")).clone();
    let outer = &mut chunk.functions[0];
    let outer_chunk = std::sync::Arc::make_mut(&mut outer.chunk);
    let inner = &mut outer_chunk.functions[0];
    let inner_chunk = std::sync::Arc::make_mut(&mut inner.chunk);
    let offset = inner_chunk
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::LoadUpvalue(_)))
        .unwrap();
    inner_chunk.instructions[offset] = Instruction::LoadUpvalue(UpvalueSlot(u16::MAX));
    assert_eq!(
        chunk.validate(),
        Err(ChunkError::InvalidUpvalueSlot {
            offset,
            slot: UpvalueSlot(u16::MAX),
            upvalue_count: 1,
        })
    );
}

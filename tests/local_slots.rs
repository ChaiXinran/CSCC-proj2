use agentjs::{
    Engine, ExecutionOptions,
    bytecode::{ChunkError, Compiler, DynamicScopePolicy, Instruction, LocalSlot},
    contracts::{ChunkExecutor, NativeFrontend, ProgramCompiler, SourceParser},
    runtime::NativeContext,
    vm::Vm,
};

fn compile(source: &str) -> agentjs::bytecode::SharedChunk {
    let program = NativeFrontend.parse_source(source).unwrap();
    ProgramCompiler::compile_program(&mut Compiler::new(), &program).unwrap()
}

#[test]
fn lowers_parameters_and_function_vars_to_local_slots() {
    let chunk = compile("function add(a) { var b = 1; b = a + b; return b; }");
    let function = &chunk.functions[0];
    let names = function
        .local_layout
        .bindings
        .iter()
        .map(|binding| binding.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(function.dynamic_scope, DynamicScopePolicy::Static);
    assert!(
        function
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadLocal(_)))
    );
    assert!(
        function
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::StoreLocal(_)))
    );
    assert!(
        function
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::InitializeLocal(_)))
    );
}

#[test]
fn direct_function_declarations_are_activation_locals() {
    let chunk = compile("function outer() { function helper() { return 1; } return helper(); }");
    let outer = &chunk.functions[0];
    assert!(
        outer
            .local_layout
            .bindings
            .iter()
            .any(|binding| binding.name == "helper")
    );
    assert!(outer.chunk.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::LoadLocal(slot)
            if outer.local_layout.bindings[usize::from(slot.0)].name == "helper"
    )));
}

#[test]
fn parameter_preamble_uses_local_slots() {
    let chunk = compile(
        "function defaults(value = 1) { return value; }
         function pattern({ item } = { item: 2 }) { return item; }
         function rest(...[first]) { return first; }",
    );
    for function in &chunk.functions {
        let preamble = &function.chunk.instructions[..function.chunk.function_body_start];
        assert!(
            preamble
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LoadLocal(_))),
            "missing preamble local load for {:?}",
            function.name
        );
        assert!(!preamble.iter().any(|instruction| matches!(
            instruction,
            Instruction::LoadName(_) | Instruction::StoreName(_)
        )));
    }
}

#[test]
fn closures_keep_parent_access_out_of_the_current_local_layout() {
    let chunk = compile("function outer(value) { return function () { return value; }; }");
    let inner = &chunk.functions[0].chunk.functions[0];
    assert!(inner.local_layout.bindings.is_empty());
    assert!(
        inner
            .chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadUpvalue(_)))
    );
}

#[test]
fn direct_eval_and_with_disable_slots_for_the_whole_function() {
    for (source, expected) in [
        (
            "function probe(a) { var b = 1; eval('b = 2'); return a + b; }",
            DynamicScopePolicy::DirectEval,
        ),
        (
            "function probe(a) { var b = 1; with ({ a: 3 }) { b = a; } return b; }",
            DynamicScopePolicy::With,
        ),
    ] {
        let chunk = compile(source);
        let function = &chunk.functions[0];
        assert_eq!(function.dynamic_scope, expected);
        assert!(function.local_layout.bindings.is_empty());
        assert!(
            !function
                .chunk
                .instructions
                .iter()
                .any(|instruction| matches!(
                    instruction,
                    Instruction::LoadLocal(_)
                        | Instruction::StoreLocal(_)
                        | Instruction::InitializeLocal(_)
                ))
        );
    }
}

#[test]
fn local_slots_preserve_runtime_semantics() {
    let report = Engine::default()
        .execute(
            "function outer(a = 2, ...rest) {
                 var total = a + rest[0];
                 function bump() { total = total + 1; return total; }
                 return bump;
             }
             var fn = outer(undefined, 3);
             fn() + ':' + fn();",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "6:7");
}

#[test]
fn function_slots_preserve_annex_b_and_arguments_bindings() {
    let report = Engine::default()
        .execute(
            "function annex() { var helper; { function helper() {} } return typeof helper; }
             function namedArguments() { function arguments() {} return typeof arguments; }
             annex() + ':' + namedArguments();",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "function:function");
}

#[test]
fn dynamic_scope_fallback_preserves_eval_and_with_semantics() {
    let report = Engine::default()
        .execute(
            "function probe(a) {
                 var b = 1;
                 eval('b = 2');
                 with ({ a: 4 }) { b = a; }
                 return a + ':' + b;
             }
             probe(3);",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "3:4");
}

#[test]
fn simple_function_hot_path_is_at_least_seventy_percent_local() {
    let chunk = compile(
        "function sum(limit) {
             var total = 0;
             var index = 0;
             while (index < limit) { total = total + index; index = index + 1; }
             return total;
         }
         sum(1000);",
    );
    let mut context = NativeContext::default();
    context.reset_name_resolution_metrics();
    ChunkExecutor::execute_chunk(&mut Vm::default(), &chunk, &mut context).unwrap();
    let metrics = context.name_resolution_metrics();
    let local = metrics.load_local_count + metrics.store_local_count;
    let named = metrics.load_name_count + metrics.store_name_count;
    assert!(local * 10 >= (local + named) * 7, "metrics: {metrics:?}");
}

#[test]
fn chunk_validation_rejects_an_out_of_bounds_local_slot() {
    let mut chunk = (*compile("function probe(value) { return value; }")).clone();
    let function = &mut chunk.functions[0];
    let child = std::sync::Arc::make_mut(&mut function.chunk);
    let offset = child
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, Instruction::LoadLocal(_)))
        .unwrap();
    child.instructions[offset] = Instruction::LoadLocal(LocalSlot(u16::MAX));
    assert_eq!(
        chunk.validate(),
        Err(ChunkError::InvalidLocalSlot {
            offset,
            slot: LocalSlot(u16::MAX),
            local_count: 1,
        })
    );
}

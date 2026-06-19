//! Minimal A/B integration coverage for Native V4 source lowering.

use agentjs::{
    bytecode::{Compiler, Instruction},
    contracts::{NativeFrontend, ProgramCompiler, SourceParser},
};

fn compile(source: &str) -> agentjs::bytecode::Chunk {
    let program = NativeFrontend
        .parse_source(source)
        .unwrap_or_else(|error| panic!("front end should accept {source:?}: {error}"));
    let chunk = ProgramCompiler::compile_program(&mut Compiler::new(), &program)
        .unwrap_or_else(|error| panic!("compiler should accept {source:?}: {error}"));
    chunk.validate().unwrap();
    chunk
}

#[test]
fn compiles_delete_in_and_instanceof() {
    let chunk = compile("delete object.x; delete object[key]; key in object; value instanceof C;");

    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DeleteProperty(_)))
    );
    assert!(chunk.instructions.contains(&Instruction::DeleteElement));
    assert!(chunk.instructions.contains(&Instruction::HasProperty));
    assert!(chunk.instructions.contains(&Instruction::InstanceOf));
}

#[test]
fn compiles_accessors_and_prototype_setter() {
    let chunk = compile(
        "({ value: 1, get x() { return 2; }, set x(v) { this.saved = v; }, __proto__: base });",
    );

    assert_eq!(chunk.instructions[0], Instruction::ObjectCreateEmpty);
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DefineDataProperty(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DefineGetter(_)))
    );
    assert!(
        chunk
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::DefineSetter(_)))
    );
    assert!(
        chunk
            .instructions
            .contains(&Instruction::SetObjectPrototype)
    );
}

#[test]
fn compiles_sparse_array_holes_and_trailing_commas() {
    let sparse = compile("[1, , 3];");
    assert!(
        sparse
            .instructions
            .contains(&Instruction::ArrayCreateSparse(3))
    );
    assert!(sparse.instructions.contains(&Instruction::DefineElement(0)));
    assert!(sparse.instructions.contains(&Instruction::DefineElement(2)));

    let trailing = compile("[1,];");
    assert!(trailing.instructions.contains(&Instruction::ArrayCreate(1)));

    let trailing_hole = compile("[1,,];");
    assert!(
        trailing_hole
            .instructions
            .contains(&Instruction::ArrayCreateSparse(2))
    );
}

#[test]
fn rejects_invalid_accessors_and_duplicate_prototype_setters() {
    for source in [
        "({ get x(v) {} });",
        "({ set x() {} });",
        "({ __proto__: a, __proto__: b });",
    ] {
        assert!(
            NativeFrontend.parse_source(source).is_err(),
            "source should fail: {source}"
        );
    }
}

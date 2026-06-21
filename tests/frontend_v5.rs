//! A-group acceptance tests for the Native V5 front end.
//!
//! These tests stop at `Program` and do not depend on bytecode or runtime
//! behavior.

use agentjs::{
    NativeError,
    contracts::{
        CatchClause, Expression, Literal, NativeFrontend, Program, SourceParser, Statement,
        SwitchCase, VariableKind,
    },
};

fn parse(source: &str) -> Result<Program, NativeError> {
    NativeFrontend.parse_source(source)
}

fn parse_ok(source: &str) -> Program {
    parse(source).unwrap_or_else(|error| panic!("V5 front end should parse source: {error}"))
}

#[test]
fn parses_try_catch_finally_and_optional_catch_binding() {
    let program = parse_ok(
        "try { throw 1; } catch (error) { var caught = error; } finally { var done = true; }",
    );
    assert_eq!(program.body.len(), 1);
    let Statement::Try {
        block,
        handler,
        finalizer,
    } = &program.body[0]
    else {
        panic!("expected try statement");
    };
    assert!(matches!(block.as_slice(), [Statement::Throw(_)]));
    assert_eq!(
        handler
            .as_ref()
            .and_then(|clause| clause.parameter.as_deref()),
        Some("error")
    );
    assert_eq!(handler.as_ref().map(|clause| clause.body.len()), Some(1));
    assert_eq!(finalizer.as_ref().map(Vec::len), Some(1));

    assert_eq!(
        parse_ok("try {} catch {}").body,
        [Statement::Try {
            block: vec![],
            handler: Some(CatchClause {
                parameter: None,
                body: vec![],
            }),
            finalizer: None,
        }]
    );
}

#[test]
fn parses_switch_cases_default_and_fallthrough() {
    let program =
        parse_ok("switch (value) { case 1: first; case 2: second; break; default: fallback; }");
    let Statement::Switch {
        discriminant,
        cases,
    } = &program.body[0]
    else {
        panic!("expected switch statement");
    };
    assert_eq!(discriminant, &Expression::Identifier("value".into()));
    assert_eq!(cases.len(), 3);
    assert_eq!(
        cases[0],
        SwitchCase {
            test: Some(Expression::Literal(Literal::Number(1.0))),
            consequent: vec![Statement::Expression(Expression::Identifier(
                "first".into()
            ))],
        }
    );
    assert!(matches!(
        cases[1].consequent.as_slice(),
        [Statement::Expression(_), Statement::Break]
    ));
    assert!(cases[2].test.is_none());
}

#[test]
fn parses_let_and_const_declarations() {
    let program = parse_ok("let value; const fixed = 1;");
    assert!(matches!(
        program.body[0],
        Statement::VariableDeclaration {
            kind: VariableKind::Let,
            ..
        }
    ));
    assert!(matches!(
        program.body[1],
        Statement::VariableDeclaration {
            kind: VariableKind::Const,
            ..
        }
    ));
}

#[test]
fn accepts_break_in_switch_but_not_continue() {
    assert!(parse("switch (value) { case 1: break; }").is_ok());
    assert!(matches!(
        parse("switch (value) { case 1: continue; }"),
        Err(NativeError::Parse(_))
    ));
}

#[test]
fn rejects_v5_early_errors() {
    for source in [
        "try {}",
        "try {} catch () {}",
        "try {} catch (first, second) {}",
        "try {} catch value {}",
        "switch (value) { statement; }",
        "switch (value) { default: first; default: second; }",
        "const missing;",
        "let duplicate; const duplicate = 1;",
        "{ const duplicate = 1; let duplicate; }",
        "try {} catch (error) { let error; }",
        "switch (value) { case 1: let duplicate; case 2: const duplicate = 2; }",
    ] {
        assert!(
            matches!(parse(source), Err(NativeError::Parse(_))),
            "expected ParseError for {source}"
        );
    }
}

#[test]
fn nested_functions_do_not_inherit_breakable_contexts() {
    for source in [
        "while (true) { function nested() { break; } }",
        "switch (value) { case 1: function nested() { break; } }",
        "while (true) { function nested() { continue; } }",
    ] {
        assert!(
            matches!(parse(source), Err(NativeError::Parse(_))),
            "expected ParseError for {source}"
        );
    }
}

use agentjs::{BackendKind, Engine, ExecutionOptions, FailureKind, RuntimeConfig};

fn native_engine() -> Engine {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
}

fn eval(source: &str) -> String {
    native_engine()
        .execute(source, ExecutionOptions::default())
        .unwrap()
        .value
}

#[test]
fn executes_v2_if_else_and_conditional_expressions() {
    assert_eq!(
        eval("var x = 0; if (true) { x = 1; } else { x = 2; } x;"),
        "1"
    );
    assert_eq!(eval("var x = false ? 1 : true ? 2 : 3; x;"), "2");
}

#[test]
fn executes_v2_while_break_and_continue() {
    assert_eq!(eval("var i = 0; while (i < 5) { i = i + 1; } i;"), "5");
    assert_eq!(eval("var i = 0; while (true) { i = 1; break; } i;"), "1");
    assert_eq!(
        eval(
            "var i = 0; while (i < 3) { \
             i = i + 1; if (i === 2) continue; \
             } i;"
        ),
        "3"
    );
}

#[test]
fn executes_checked_in_v2_example() {
    assert_eq!(eval(include_str!("../examples/v2.js")), "5");
}

#[test]
fn supports_v2_typeof_and_multiple_var_declarators() {
    assert_eq!(eval("typeof missingName;"), "undefined");
    assert_eq!(eval("var a, b = 2; typeof a === 'undefined' ? b : 0;"), "2");
}

#[test]
fn reports_v2_parse_and_runtime_limit_failures() {
    let syntax = native_engine()
        .execute("break;", ExecutionOptions::default())
        .unwrap_err();
    assert_eq!(syntax.kind, FailureKind::Syntax);

    let engine = Engine::with_backend(
        BackendKind::Native,
        RuntimeConfig {
            loop_limit: 4,
            ..RuntimeConfig::default()
        },
    );
    let limit = engine
        .execute("while (true) {}", ExecutionOptions::default())
        .unwrap_err();
    assert_eq!(limit.kind, FailureKind::RuntimeLimit);
}

#[test]
fn maps_thrown_test262_error_through_the_complete_pipeline() {
    let engine = Engine::with_backend(
        BackendKind::Native,
        RuntimeConfig {
            install_test262_host: true,
            ..RuntimeConfig::default()
        },
    );
    let error = engine
        .execute(
            "throw new Test262Error('expected');",
            ExecutionOptions::default(),
        )
        .unwrap_err();

    assert_eq!(error.kind, FailureKind::Test262);
    assert!(error.message.contains("expected"));
}

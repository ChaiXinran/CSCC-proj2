use agentjs::{BackendKind, Engine, ExecutionOptions, Runtime, RuntimeConfig};

#[test]
fn evaluates_javascript() {
    let report = Engine::default()
        .execute("6 * 7", ExecutionOptions::default())
        .unwrap();
    assert_eq!(report.value, "42");
}

#[test]
fn captures_console_output() {
    let report = Engine::default()
        .execute("console.log('hello', 7)", ExecutionOptions::default())
        .unwrap();
    assert_eq!(report.output, ["hello 7"]);
}

#[test]
fn isolates_separate_executions() {
    let engine = Engine::default();
    engine
        .execute("globalThis.secret = 42", ExecutionOptions::default())
        .unwrap();
    let report = engine
        .execute("typeof secret", ExecutionOptions::default())
        .unwrap();
    assert_eq!(report.value, "undefined");
}

#[test]
fn rejects_runaway_loops() {
    let engine = Engine::new(RuntimeConfig {
        loop_limit: 10,
        ..RuntimeConfig::default()
    });
    assert!(
        engine
            .execute("while (true) {}", ExecutionOptions::default())
            .is_err()
    );
}

#[test]
fn reuses_prepared_scripts_in_a_persistent_runtime() {
    let mut runtime = Runtime::new(RuntimeConfig {
        script_cache_capacity: 2,
        ..RuntimeConfig::default()
    })
    .unwrap();

    let first = runtime
        .eval(
            "(function () { return 21 * 2; })()",
            ExecutionOptions::default(),
        )
        .unwrap();
    let second = runtime
        .eval(
            "(function () { return 21 * 2; })()",
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(first.value, "42");
    assert_eq!(second.value, "42");
}

#[test]
fn native_backend_executes_v1_expressions() {
    let engine = Engine::with_backend(BackendKind::Native, RuntimeConfig::default());
    let report = engine
        .execute("1 + 2 * 3;", ExecutionOptions::default())
        .unwrap();

    assert_eq!(report.value, "7");
}

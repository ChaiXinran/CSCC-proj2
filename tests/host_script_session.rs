use agentjs::{ExecutionOptions, Runtime, RuntimeConfig};

fn runtime() -> Runtime {
    Runtime::new(RuntimeConfig::default()).expect("runtime creates")
}

#[test]
fn later_function_declaration_is_available_to_an_earlier_fragment() {
    let mut runtime = runtime();
    let first = runtime
        .prepare_host_fragment("answer();", "first.js")
        .expect("first fragment prepares");
    let second = runtime
        .prepare_host_fragment("function answer() { return 42; }", "second.js")
        .expect("second fragment prepares");
    let mut session = runtime
        .start_host_script_session(&[first.clone(), second.clone()])
        .expect("session instantiates");

    runtime
        .eval_host_fragment(&mut session, &first)
        .expect("earlier fragment executes");
    runtime
        .eval_host_fragment(&mut session, &second)
        .expect("later fragment executes");
    assert_eq!(
        runtime
            .eval("answer()", ExecutionOptions::default())
            .expect("session remains usable")
            .value,
        "42"
    );
}

#[test]
fn host_session_rejects_cross_fragment_lexical_conflicts_before_execution() {
    let mut runtime = runtime();
    let first = runtime
        .prepare_host_fragment("let shared = 1;", "first.js")
        .expect("first fragment prepares");
    let second = runtime
        .prepare_host_fragment("var shared = 2;", "second.js")
        .expect("second fragment prepares");
    let error = runtime
        .start_host_script_session(&[first, second])
        .expect_err("conflict must be rejected before execution");
    assert_eq!(error.kind.name(), "SyntaxError");
}

#[test]
fn host_session_keeps_var_bindings_visible_across_fragments() {
    let mut runtime = runtime();
    let first = runtime
        .prepare_host_fragment("value = 7;", "first.js")
        .expect("first fragment prepares");
    let second = runtime
        .prepare_host_fragment("var value;", "second.js")
        .expect("second fragment prepares");
    let mut session = runtime
        .start_host_script_session(&[first.clone(), second.clone()])
        .expect("session instantiates");
    runtime
        .eval_host_fragment(&mut session, &first)
        .expect("assignment executes");
    runtime
        .eval_host_fragment(&mut session, &second)
        .expect("declaration executes");
    assert_eq!(
        runtime
            .eval("value", ExecutionOptions::default())
            .expect("value remains visible")
            .value,
        "7"
    );
}

#[test]
fn host_session_preserves_bundle_export_for_following_fragment() {
    let mut runtime = runtime();
    let bundle = runtime
        .prepare_host_fragment(
            include_str!("../benchmarks/JetStream2/validatorjs/dist/bundle.es6.min.js"),
            "bundle.js",
        )
        .expect("bundle prepares");
    let benchmark = runtime
        .prepare_host_fragment(
            include_str!("../benchmarks/JetStream2/validatorjs/benchmark.js"),
            "benchmark.js",
        )
        .expect("benchmark prepares");
    let mut session = runtime
        .start_host_script_session(&[bundle.clone(), benchmark.clone()])
        .expect("session instantiates");
    runtime
        .eval_host_fragment(&mut session, &bundle)
        .expect("bundle executes");
    assert_eq!(
        runtime
            .eval("typeof ValidatorJSBenchmark", ExecutionOptions::default())
            .expect("global lookup works")
            .value,
        "object"
    );
    assert_eq!(
        runtime
            .eval(
                "typeof ValidatorJSBenchmark.runTest",
                ExecutionOptions::default()
            )
            .expect("export method lookup works")
            .value,
        "function"
    );
}

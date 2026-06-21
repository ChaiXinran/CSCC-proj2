use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, FailureKind, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native V5 source should execute: {error}"))
        .value
}

#[test]
fn try_catch_preserves_thrown_javascript_values() {
    assert_eq!(
        native_eval(
            r#"
            var result = 0;
            try { throw 3; } catch (error) { result = error; }
            result;
            "#
        ),
        "3"
    );
}

#[test]
fn finally_return_overrides_try_return() {
    assert_eq!(
        native_eval(
            r#"
            function f() {
              try { return 1; } finally { return 2; }
            }
            f();
            "#
        ),
        "2"
    );
}

#[test]
fn finally_runs_before_an_outer_catch_handles_throw() {
    assert_eq!(
        native_eval(
            r#"
            var flag = 0;
            try {
              try { throw "boom"; } finally { flag = 1; }
            } catch (error) {}
            flag;
            "#
        ),
        "1"
    );
}

#[test]
fn runtime_reference_errors_inside_try_are_catchable() {
    assert_eq!(
        native_eval(
            r#"
            var caught = 0;
            try { missingName; } catch (error) { caught = 1; }
            caught;
            "#
        ),
        "1"
    );
}

#[test]
fn switch_matches_once_and_falls_through_until_break_or_end() {
    assert_eq!(
        native_eval(
            r#"
            var result = "";
            switch (2) {
              case 1: result = "a"; break;
              case 2: result = "b";
              default: result = result + "c";
            }
            result;
            "#
        ),
        "bc"
    );
}

#[test]
fn lexical_bindings_are_block_scoped() {
    assert_eq!(
        native_eval(
            r#"
            let outer = 1;
            { let outer = 2; const fixed = 3; }
            outer;
            "#
        ),
        "1"
    );
}

#[test]
fn temporal_dead_zone_is_a_reference_error() {
    let error = Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute("x; let x;", ExecutionOptions::default())
        .unwrap_err();

    assert_eq!(error.kind, FailureKind::Reference);
}

#[test]
fn initialized_const_rejects_assignment() {
    let error = Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute("const x = 1; x = 2;", ExecutionOptions::default())
        .unwrap_err();

    assert_eq!(error.kind, FailureKind::Type);
}

#[test]
fn array_callback_throw_reaches_the_surrounding_catch() {
    assert_eq!(
        native_eval(
            r#"
            var caught = 0;
            function fail(value) { throw value; }
            try { [1].map(fail); } catch (error) { caught = error; }
            caught;
            "#
        ),
        "1"
    );
}

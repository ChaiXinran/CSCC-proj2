//! V13-B dynamic-import regression coverage.

use agentjs::{Engine, ExecutionOptions, RuntimeConfig};

fn run(source: &str) -> String {
    Engine::new(RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect("native evaluation succeeds")
        .value
}

#[test]
fn dynamic_import_returns_a_promise_when_the_loader_rejects() {
    assert_eq!(
        run("import('./does-not-exist.js', { with: { type: 'json' } }) instanceof Promise"),
        "true"
    );
}

#[test]
fn dynamic_import_evaluates_specifier_and_options_once() {
    assert_eq!(
        run("let n = 0; import((n++, './missing.js'), { with: { type: (n++, 'json') } }); n"),
        "2"
    );
}

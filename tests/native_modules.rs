//! V13-B dynamic-import regression coverage.

use agentjs::runtime::{JsValue, NativeContext};
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

#[test]
fn module_import_binding_reads_the_current_export_cell() {
    let mut context = NativeContext::default();
    let export_environment = context
        .push_environment(Some(context.global_environment()))
        .expect("allocate export environment");
    context
        .create_mutable_binding(export_environment, "value".into(), false, true)
        .expect("create export binding");
    context
        .initialize_binding(export_environment, "value", JsValue::Number(1.0))
        .expect("initialize export binding");
    context.pop_environment().expect("leave export environment");

    let import_environment = context
        .push_environment(Some(context.global_environment()))
        .expect("allocate import environment");
    context
        .create_immutable_binding(import_environment, "imported".into(), true)
        .expect("create import binding");
    context.create_module_import_link(
        import_environment,
        "imported".into(),
        export_environment,
        "value".into(),
    );
    assert_eq!(
        context.resolve_binding("imported").map(|(_, value)| value),
        Some(JsValue::Number(1.0))
    );

    context
        .push_existing_environment(export_environment)
        .expect("enter export environment");
    context
        .set_binding("value", JsValue::Number(2.0))
        .expect("update export cell");
    context.pop_environment().expect("leave export environment");
    assert_eq!(
        context.resolve_binding("imported").map(|(_, value)| value),
        Some(JsValue::Number(2.0))
    );
}

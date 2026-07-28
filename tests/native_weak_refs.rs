use agentjs::{Engine, ExecutionOptions, RuntimeConfig};

fn run(source: &str) -> String {
    Engine::new(RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect("native evaluation succeeds")
        .value
}

#[test]
fn weak_ref_retains_an_observable_target_until_collection() {
    assert_eq!(
        run("let target = {}; new WeakRef(target).deref() === target"),
        "true"
    );
}

#[test]
fn finalization_registry_register_and_unregister_follow_token_identity() {
    assert_eq!(
        run(
            "let r = new FinalizationRegistry(() => {}); let target = {}; let token = {}; r.register(target, 1, token); r.unregister(token) + ':' + r.unregister(token)"
        ),
        "true:false"
    );
}

#[test]
fn weak_targets_reject_ordinary_primitives_and_registered_symbols() {
    assert_eq!(
        run(
            "let count = 0; for (let value of [undefined, null, 1, 'x', Symbol.for('x')]) { try { new WeakRef(value); } catch (e) { count++; } } count"
        ),
        "5"
    );
}

use agentjs::{
    backend::NativeRuntime,
    engine::{ExecutionOptions, RuntimeConfig},
};

fn test262_eval(source: &str) -> String {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        install_test262_host: true,
        ..RuntimeConfig::default()
    });
    runtime
        .eval_source(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native test262 eval failed for `{source}`: {error}"))
}

#[test]
fn create_realm_returns_distinct_global_and_intrinsics() {
    assert_eq!(
        test262_eval(
            "var r = $262.createRealm(); \
             (r.global !== globalThis) + ':' + \
             (r.global.Object !== Object) + ':' + \
             (r.global.Array.prototype !== Array.prototype) + ':' + \
             (r.global.$262.global === r.global);"
        ),
        "true:true:true:true"
    );
}

#[test]
fn realm_eval_script_runs_with_realm_global() {
    assert_eq!(
        test262_eval(
            "var r = $262.createRealm(); \
             r.evalScript('var realmOnly = 41; this.realmProp = 1;'); \
             var inside = r.evalScript('realmOnly + this.realmProp'); \
             var outside = typeof realmOnly + ':' + typeof realmProp; \
             inside + ':' + outside;"
        ),
        "42:undefined:undefined"
    );
}

#[test]
fn cross_realm_function_constructor_uses_target_global() {
    assert_eq!(
        test262_eval(
            "var r = $262.createRealm(); \
             r.evalScript('var marker = 99;'); \
             var fn = r.global.Function('return marker + (this === globalThis ? 1 : 0);'); \
             fn() + ':' + (fn() === 100);"
        ),
        "100:true"
    );
}

#[test]
fn cross_realm_constructors_use_target_intrinsic_prototypes() {
    assert_eq!(
        test262_eval(
            "var r = $262.createRealm(); \
             var arr = new r.global.Array(1, 2); \
             (Object.getPrototypeOf(arr) === r.global.Array.prototype) + ':' + \
             (Object.getPrototypeOf(arr) === Array.prototype) + ':' + \
             arr.length;"
        ),
        "true:false:2"
    );
}

#[test]
fn cross_realm_sloppy_function_boxes_primitive_this_in_callee_realm() {
    assert_eq!(
        test262_eval(
            "var r = $262.createRealm(); \
             var fn = r.global.Function('return this;'); \
             var boxed = fn.call(true); \
             (boxed.constructor === r.global.Boolean) + ':' + \
             (boxed instanceof r.global.Boolean);"
        ),
        "true:true"
    );
}

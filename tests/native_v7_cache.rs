use agentjs::{
    backend::NativeRuntime,
    engine::{ExecutionOptions, RuntimeConfig},
};

#[test]
fn native_script_cache_records_hits_and_misses() {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        script_cache_capacity: 2,
        ..RuntimeConfig::default()
    });

    assert_eq!(
        runtime
            .eval_source("var x = 1; x + 1;", ExecutionOptions::default())
            .unwrap(),
        "2"
    );
    assert_eq!(runtime.cache_stats().misses, 1);
    assert_eq!(runtime.cache_stats().hits, 0);

    assert_eq!(
        runtime
            .eval_source("var x = 1; x + 1;", ExecutionOptions::default())
            .unwrap(),
        "2"
    );
    assert_eq!(runtime.cache_stats().misses, 1);
    assert_eq!(runtime.cache_stats().hits, 1);
}

#[test]
fn native_script_cache_capacity_zero_disables_caching() {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        script_cache_capacity: 0,
        ..RuntimeConfig::default()
    });

    runtime
        .eval_source("var x = 2; x * 3;", ExecutionOptions::default())
        .unwrap();
    runtime
        .eval_source("var x = 2; x * 3;", ExecutionOptions::default())
        .unwrap();

    assert_eq!(runtime.cache_stats().misses, 0);
    assert_eq!(runtime.cache_stats().hits, 0);
}

#[test]
fn native_script_cache_keys_include_strictness() {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        script_cache_capacity: 2,
        ..RuntimeConfig::default()
    });

    runtime
        .eval_source("var x = 3; x;", ExecutionOptions::default())
        .unwrap();
    runtime
        .eval_source(
            "var x = 3; x;",
            ExecutionOptions {
                strict: true,
                ..ExecutionOptions::default()
            },
        )
        .unwrap();

    assert_eq!(runtime.cache_stats().misses, 2);
    assert_eq!(runtime.cache_stats().hits, 0);
}

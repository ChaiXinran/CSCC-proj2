use agentjs::{
    backend::NativeRuntime,
    engine::{ExecutionOptions, RuntimeConfig},
};

fn runtime() -> NativeRuntime {
    NativeRuntime::new(RuntimeConfig::default())
}

#[test]
fn promise_resolve_then_chains_through_job_queue() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = 0; Promise.resolve(20).then(function (value) { result = value + 1; });",
            ExecutionOptions::default(),
        )
        .expect("promise script");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read result"),
        "21"
    );
}

#[test]
fn promise_reject_catch_and_finally_are_drained() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var log = ''; \
             Promise.reject('bad') \
               .catch(function (reason) { log = reason; return 7; }) \
               .finally(function () { log = log + ':finally'; }) \
               .then(function (value) { log = log + ':' + value; });",
            ExecutionOptions::default(),
        )
        .expect("promise script");

    assert_eq!(
        runtime
            .eval_source("log;", ExecutionOptions::default())
            .expect("read log"),
        "bad:finally:7"
    );
}

#[test]
fn promise_constructor_executor_resolves_with_bound_functions() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = 0; \
             new Promise(function (resolve) { resolve(4); }) \
               .then(function (value) { result = value * 2; });",
            ExecutionOptions::default(),
        )
        .expect("promise script");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read result"),
        "8"
    );
}

#[test]
fn promise_try_and_with_resolvers_expose_minimal_capabilities() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = ''; \
             Promise.try(function (a, b) { return a + b; }, 2, 3) \
               .then(function (value) { result = result + value; }); \
             var cap = Promise.withResolvers(); \
             cap.promise.then(function (value) { result = result + ':' + value; }); \
             cap.resolve('ok');",
            ExecutionOptions::default(),
        )
        .expect("promise script");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read result"),
        "5:ok"
    );
}

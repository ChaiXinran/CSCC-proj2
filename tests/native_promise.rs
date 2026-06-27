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

#[test]
fn promise_all_consumes_a_custom_iterator() {
    let mut config = RuntimeConfig::default();
    config.gc_allocation_threshold = 1;
    let mut runtime = NativeRuntime::new(config);
    runtime
        .eval_source(
            "var iterable = {}; \
             iterable[Symbol.iterator] = function () { \
               var value = 0; \
               return { next: function () { \
                 value = value + 1; \
                 return { value: value, done: value > 2 }; \
               } }; \
             }; \
             var result = ''; \
             Promise.all(iterable).then(function (values) { \
               result = '' + values[0] + values[1]; \
             });",
            ExecutionOptions::default(),
        )
        .expect("Promise.all custom iterator");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read result"),
        "12"
    );
}

#[test]
fn async_function_awaits_settled_values_and_rejects_on_throw() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var log = ''; \
             async function fulfilled() { \
               var value = await Promise.resolve(6); \
               return value + 1; \
             } \
             async function rejected() { throw 'bad'; } \
             fulfilled().then(function (value) { log = '' + value; }); \
             rejected().catch(function (reason) { log = log + ':' + reason; });",
            ExecutionOptions::default(),
        )
        .expect("async functions");

    assert_eq!(
        runtime
            .eval_source("log;", ExecutionOptions::default())
            .expect("read log"),
        "7:bad"
    );
}

#[test]
fn for_await_of_uses_the_sync_iterator_fallback() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = 0; \
             async function sum() { \
               for await (var value of [Promise.resolve(2), 3]) { \
                 result = result + value; \
               } \
               return result; \
             } \
             sum();",
            ExecutionOptions::default(),
        )
        .expect("for-await-of over sync iterable");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read result"),
        "5"
    );
}

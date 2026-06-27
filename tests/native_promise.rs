use agentjs::{
    BackendKind,
    backend::NativeRuntime,
    engine::{ExecutionOptions, Runtime, RuntimeConfig},
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
fn promise_static_methods_use_the_receiver_constructor() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "function SubPromise(executor) { \
               var promise = new Promise(executor); \
               Object.setPrototypeOf(promise, SubPromise.prototype); \
               return promise; \
             } \
             SubPromise.prototype = Object.create(Promise.prototype); \
             SubPromise.prototype.constructor = SubPromise; \
             var calls = 0; \
             var inheritedResolve = Promise.resolve; \
             SubPromise.resolve = function(value) { \
               calls = calls + 1; \
               return inheritedResolve.call(this, value); \
             }; \
             var resolved = Promise.resolve.call(SubPromise, 1); \
             var rejected = Promise.reject.call(SubPromise, 'bad'); \
             var combined = Promise.all.call(SubPromise, [1, 2]); \
             var capability = Promise.withResolvers.call(SubPromise); \
             var invalidReceiver = false; \
             try { Promise.resolve.call({}, 1); } \
             catch (error) { invalidReceiver = error.name === 'TypeError'; } \
             var result = \
               (resolved instanceof SubPromise) + ':' + \
               (rejected instanceof SubPromise) + ':' + \
               (combined instanceof SubPromise) + ':' + \
               (capability.promise instanceof SubPromise) + ':' + \
               capability.resolve.name + ':' + calls + ':' + invalidReceiver;",
            ExecutionOptions::default(),
        )
        .expect("receiver-specific Promise capability");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read Promise capability result"),
        "true:true:true:true::2:true"
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
fn promise_combinators_resolve_inputs_and_preserve_order() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var allResult = ''; var settledResult = ''; var anyResult = ''; var raceResult = ''; \
             var thenable = { then: function (resolve) { resolve(2); } }; \
             Promise.all([Promise.resolve(1), thenable]).then(function (values) { \
               allResult = '' + values[0] + values[1]; \
             }); \
             Promise.allSettled([Promise.resolve(3), Promise.reject('bad')]) \
               .then(function (values) { \
                 settledResult = values[0].status + ':' + values[0].value + \
                   '/' + values[1].status + ':' + values[1].reason; \
               }); \
             Promise.any([Promise.reject('first'), Promise.resolve(4)]) \
               .then(function (value) { anyResult = '' + value; }); \
             Promise.race([Promise.resolve(5), Promise.resolve(6)]) \
               .then(function (value) { raceResult = '' + value; });",
            ExecutionOptions::default(),
        )
        .expect("Promise combinators");

    assert_eq!(
        runtime
            .eval_source(
                "allResult + '|' + settledResult + '|' + anyResult + '|' + raceResult;",
                ExecutionOptions::default(),
            )
            .expect("read combinator results"),
        "12|fulfilled:3/rejected:bad|4|5"
    );
}

#[test]
fn promise_combinators_reject_instead_of_throwing_on_iterator_failure() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = ''; \
             var iterable = {}; \
             iterable[Symbol.iterator] = function () { \
               return { next: function () { throw 'iterator failed'; } }; \
             }; \
             Promise.all(iterable).catch(function (reason) { result = reason; });",
            ExecutionOptions::default(),
        )
        .expect("Promise combinator iterator failure");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read iterator rejection"),
        "iterator failed"
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

#[test]
fn for_await_promise_chain_signals_async_completion() {
    let mut config = RuntimeConfig::default();
    config.install_test262_host = true;
    let mut runtime = Runtime::with_backend(BackendKind::Native, config).expect("native runtime");
    let report = runtime
        .eval(
            "function $DONE(error) { print(error ? 'failed' : 'complete'); } \
             var iterCount = 0; \
             async function fn() { \
               for await (var value of [[2]]) { iterCount = iterCount + 1; } \
             } \
             fn() \
               .then(function () { \
                 if (iterCount !== 1) { throw 'wrong count'; } \
               }, $DONE) \
               .then($DONE, $DONE);",
            ExecutionOptions::default(),
        )
        .expect("async completion chain");

    assert_eq!(report.output, ["complete"]);
}

#[test]
fn async_generator_next_returns_a_promise_for_iterator_result() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = ''; \
             async function* values() { yield await Promise.resolve(4); } \
             values().next().then(function (step) { \
               result = '' + step.value + '/' + step.done; \
             });",
            ExecutionOptions::default(),
        )
        .expect("async generator next");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read async generator result"),
        "4/false"
    );
}

#[test]
fn promise_resolution_adopts_foreign_thenables_and_native_promises() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = ''; \
             var thenable = { \
               then: function (resolve) { resolve(6); } \
             }; \
             Promise.resolve(thenable) \
               .then(function (value) { result = '' + value; }); \
             new Promise(function (resolve) { \
               resolve(Promise.resolve(7)); \
             }).then(function (value) { result = result + ':' + value; });",
            ExecutionOptions::default(),
        )
        .expect("Promise resolution procedure");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read adopted values"),
        "6:7"
    );
}

#[test]
fn promise_resolution_rejects_when_then_getter_throws() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var result = ''; \
             var thenable = {}; \
             Object.defineProperty(thenable, 'then', { \
               get: function () { throw 'poison'; } \
             }); \
             Promise.resolve(thenable).catch(function (reason) { result = reason; });",
            ExecutionOptions::default(),
        )
        .expect("poisoned then getter");

    assert_eq!(
        runtime
            .eval_source("result;", ExecutionOptions::default())
            .expect("read rejection"),
        "poison"
    );
}

#[test]
fn for_await_prefers_async_iterator_and_consumes_async_generators() {
    let mut runtime = runtime();
    runtime
        .eval_source(
            "var total = 0; \
             var iterable = {}; \
             iterable[Symbol.asyncIterator] = function () { \
               var value = 0; \
               return { next: function () { \
                 value = value + 1; \
                 return Promise.resolve({ value: value, done: value > 2 }); \
               } }; \
             }; \
             Object.defineProperty(iterable, Symbol.iterator, { \
               get: function () { throw 'sync iterator must not be read'; } \
             }); \
             async function* values() { yield 3; yield 4; } \
             async function run() { \
               for await (var first of iterable) { total = total + first; } \
               for await (var second of values()) { total = total + second; } \
             } \
             run();",
            ExecutionOptions::default(),
        )
        .expect("async iterator preference");

    assert_eq!(
        runtime
            .eval_source("total;", ExecutionOptions::default())
            .expect("read async iterator total"),
        "10"
    );
}

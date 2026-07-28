use agentjs::{Engine, ExecutionOptions, Runtime, RuntimeConfig};

#[test]
fn evaluates_javascript() {
    let report = Engine::default()
        .execute("6 * 7", ExecutionOptions::default())
        .unwrap();
    assert_eq!(report.value, "42");
}

#[test]
fn captures_console_output() {
    let report = Engine::default()
        .execute("console.log('hello', 7)", ExecutionOptions::default())
        .unwrap();
    assert_eq!(report.output, ["hello 7"]);
}

#[test]
fn calls_accept_multiple_and_non_trailing_spreads() {
    let report = Engine::default()
        .execute(
            "function join() { return Array.prototype.join.call(arguments, ','); }
             const receiver = {
                 prefix: 'ok',
                 join(...args) { return this.prefix + ':' + args.join(','); }
             };
             [join(1, ...[2, 3], 4, ...[5]), receiver.join(...[1, 2], 3, ...[4])].join('|');",
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(report.value, "1,2,3,4,5|ok:1,2,3,4");
}

#[test]
fn construct_accepts_multiple_and_non_trailing_spreads() {
    let report = Engine::default()
        .execute(
            "class Values {
                 constructor(...args) { this.text = args.join(','); }
             }
             new Values(1, ...[2, 3], 4, ...[5]).text;",
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(report.value, "1,2,3,4,5");
}

#[test]
fn spread_arguments_are_consumed_left_to_right() {
    let report = Engine::default()
        .execute(
            "const log = [];
             function value(x) { log.push(x); return x; }
             function spread(x) {
                 return {
                     [Symbol.iterator]() {
                         log.push('iter-' + x);
                         let done = false;
                         return { next() {
                             if (done) return { done: true };
                             done = true;
                             return { value: x, done: false };
                         }};
                     }
                 };
             }
             (function () {})(value(1), ...spread(2), value(3), ...spread(4));
             log.join(',');",
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(report.value, "1,iter-2,3,iter-4");
}

#[test]
fn tagged_templates_expose_indexed_cooked_values() {
    let report = Engine::default()
        .execute(
            "function tag(strings, value) {
                 return strings[0] + value + strings[1] + ':' + strings.raw[0];
             }
             tag`left-${7}-right`;",
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(report.value, "left-7-right:left-");

    let escaped = Engine::default()
        .execute(
            r"function tag(strings) { return strings[0] + ':' + strings.raw[0]; }
              tag`\x41`;",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(escaped.value, r"A:\x41");
}

#[test]
fn isolates_separate_executions() {
    let engine = Engine::default();
    engine
        .execute("globalThis.secret = 42", ExecutionOptions::default())
        .unwrap();
    let report = engine
        .execute("typeof secret", ExecutionOptions::default())
        .unwrap();
    assert_eq!(report.value, "undefined");
}

#[test]
fn rejects_runaway_loops() {
    let engine = Engine::new(RuntimeConfig {
        loop_limit: 10,
        ..RuntimeConfig::default()
    });
    assert!(
        engine
            .execute("while (true) {}", ExecutionOptions::default())
            .is_err()
    );
}

#[test]
fn reuses_prepared_scripts_in_a_persistent_runtime() {
    let mut runtime = Runtime::new(RuntimeConfig {
        script_cache_capacity: 2,
        ..RuntimeConfig::default()
    })
    .unwrap();

    let first = runtime
        .eval(
            "(function () { return 21 * 2; })()",
            ExecutionOptions::default(),
        )
        .unwrap();
    let second = runtime
        .eval(
            "(function () { return 21 * 2; })()",
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(first.value, "42");
    assert_eq!(second.value, "42");
}

#[test]
fn native_backend_executes_v1_expressions() {
    let engine = Engine::new(RuntimeConfig::default());
    let report = engine
        .execute("1 + 2 * 3;", ExecutionOptions::default())
        .unwrap();

    assert_eq!(report.value, "7");
}

use agentjs::engine::{ExecutionOptions, FailureKind, Runtime, RuntimeConfig};

fn eval_output(runtime: &mut Runtime, source: &str) -> Vec<String> {
    runtime
        .eval(source, ExecutionOptions::default())
        .unwrap()
        .output
}

fn recursion_source(depth: usize) -> String {
    format!(
        "function recurse(n) {{
            if (n === 0) return 0;
            return recurse(n - 1) + 1;
        }}
        print(recurse({depth}));"
    )
}

#[test]
fn recursion_100_and_1000_preserve_call_state() {
    std::thread::Builder::new()
        .name("agentjs-recursion-gate".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            let mut runtime = Runtime::new(RuntimeConfig {
                recursion_limit: 2_048,
                stack_limit: 2 * 1024 * 1024,
                install_test262_host: true,
                ..RuntimeConfig::default()
            })
            .unwrap();

            assert_eq!(eval_output(&mut runtime, &recursion_source(100)), ["100"]);
            assert_eq!(
                eval_output(&mut runtime, &recursion_source(1_000)),
                ["1000"]
            );
            assert_eq!(eval_output(&mut runtime, "print(1 + 2);"), ["3"]);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn recursion_5000_is_stable_with_the_jetstream_budget() {
    // The interpreter currently permits native recursion. Use a finite worker
    // stack for this diagnostic gate, matching the VM's documented B1 model.
    let worker = std::thread::Builder::new()
        .name("agentjs-deep-recursion-gate".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            // The unoptimized interpreter is deliberately much slower at this
            // depth. Standard debug tests exercise the same path at 1,000;
            // the required 5,000-depth gate runs under `cargo test --release`.
            let depth = if cfg!(debug_assertions) { 1_000 } else { 5_000 };
            let mut runtime = Runtime::new(RuntimeConfig {
                loop_limit: u64::MAX,
                recursion_limit: 8_192,
                stack_limit: 8 * 1024 * 1024,
                script_cache_capacity: 0,
                install_test262_host: true,
                ..RuntimeConfig::default()
            })
            .unwrap();
            assert_eq!(
                eval_output(&mut runtime, &recursion_source(depth)),
                [depth.to_string()]
            );
        })
        .unwrap();

    worker.join().unwrap();
}

#[test]
fn runtime_limit_unwinds_frames_and_allows_the_next_evaluation() {
    std::thread::Builder::new()
        .name("agentjs-runtime-limit-gate".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let mut runtime = Runtime::new(RuntimeConfig {
                recursion_limit: 32,
                install_test262_host: true,
                ..RuntimeConfig::default()
            })
            .unwrap();

            let error = runtime
                .eval(&recursion_source(100), ExecutionOptions::default())
                .unwrap_err();
            assert_eq!(error.kind, FailureKind::RuntimeLimit);

            assert_eq!(
                eval_output(&mut runtime, "print('recovered');"),
                ["recovered"]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn nested_callbacks_preserve_original_throw_and_unwind_forwarders() {
    let mut runtime = Runtime::new(RuntimeConfig {
        install_test262_host: true,
        ..RuntimeConfig::default()
    })
    .unwrap();
    let output = eval_output(
        &mut runtime,
        "
        const marker = { tag: 42 };
        const callback = function () {
            [0].map(function () { throw marker; });
        };
        const bound = callback.bind(undefined);
        const proxy = new Proxy(bound, {});
        try {
            proxy.call(undefined);
        } catch (error) {
            print(error === marker);
        }
        print((function (x) { return x + 1; }).apply(undefined, [2]));
        ",
    );

    assert_eq!(output, ["true", "3"]);
}

#[test]
fn object_null_equality_does_not_reenter_user_conversion() {
    let mut runtime = Runtime::new(RuntimeConfig {
        install_test262_host: true,
        ..RuntimeConfig::default()
    })
    .unwrap();
    let output = eval_output(
        &mut runtime,
        "
        var conversions = 0;
        var value = {
            toString: function () {
                conversions++;
                return 'value';
            }
        };
        print(value != null);
        print(value == null);
        print(conversions);
        ",
    );

    assert_eq!(output, ["true", "false", "0"]);
}

#[test]
fn recursive_construction_and_super_calls_unwind_cleanly() {
    std::thread::Builder::new()
        .name("agentjs-constructor-recursion-gate".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            let mut runtime = Runtime::new(RuntimeConfig {
                recursion_limit: 512,
                install_test262_host: true,
                ..RuntimeConfig::default()
            })
            .unwrap();
            let output = eval_output(
                &mut runtime,
                "
        class Base {
            constructor(n) {
                this.value = n === 0 ? 0 : new Derived(n - 1).value + 1;
            }
        }
        class Derived extends Base {
            constructor(n) { super(n); }
        }
        print(new Derived(100).value);
        print(new Derived(1).value);
        ",
            );

            assert_eq!(output, ["100", "1"]);
        })
        .unwrap()
        .join()
        .unwrap();
}

use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant},
};

use agentjs::{
    AbsoluteDeadline, BackendKind, Engine, ExecutionOptions, HostFileLoader, HostServices,
    RootedFileLoader, RunControl, Runtime, RuntimeConfig,
    test262::{RunnerOptions, Status},
};

const DEFAULT_JETSTREAM_THREAD_STACK_MIB: usize = 32;
const MIN_JETSTREAM_THREAD_STACK_MIB: usize = 4;
const MAX_JETSTREAM_THREAD_STACK_MIB: usize = 256;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("agentjs: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        print_help();
        return Ok(());
    };
    args.remove(0);

    match command.as_str() {
        "eval" => command_eval(&args),
        "run" => command_run(&args),
        "jetstream" => command_jetstream(&args),
        "repl" => command_repl(&args),
        "test262" => command_test262(&args),
        "bench" => command_bench(&args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("agentjs {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err(format!("unknown command `{command}`; use `agentjs help`")),
    }
}

fn command_eval(args: &[String]) -> Result<(), String> {
    let (_backend, source_args) =
        parse_backend_prefixed_args(args, "usage: agentjs eval [--backend native] <source>")?;
    let source = source_args.join(" ");
    let report = Engine::new(RuntimeConfig::default())
        .execute(&source, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    print_report(report);
    Ok(())
}

fn command_run(args: &[String]) -> Result<(), String> {
    let (_backend, file_args) =
        parse_backend_prefixed_args(args, "usage: agentjs run [--backend native] <file.js>")?;
    let path = file_args
        .first()
        .ok_or_else(|| "usage: agentjs run [--backend native] <file.js>".to_string())?;
    let source = fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let report = Engine::new(RuntimeConfig::default())
        .execute(&source, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    print_report(report);
    Ok(())
}

fn command_jetstream(args: &[String]) -> Result<(), String> {
    let mut path = None;
    let mut loop_limit = u64::MAX;
    let mut wall_clock_limit = None;
    let mut resource_root = None;
    let mut gc_threshold = 1_000_000;
    let mut diagnostics = false;
    let mut thread_stack_mib = DEFAULT_JETSTREAM_THREAD_STACK_MIB;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--loop-limit" => {
                index += 1;
                loop_limit = required_value(args, index, "--loop-limit")?
                    .parse::<u64>()
                    .map_err(|_| "--loop-limit must be an unsigned integer".to_string())?;
            }
            "--wall-clock-seconds" => {
                index += 1;
                let seconds = required_value(args, index, "--wall-clock-seconds")?
                    .parse::<u64>()
                    .map_err(|_| "--wall-clock-seconds must be an unsigned integer".to_string())?;
                wall_clock_limit = Some(Duration::from_secs(seconds));
            }
            "--resource-root" => {
                index += 1;
                resource_root = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--resource-root",
                )?));
            }
            "--gc-threshold" => {
                index += 1;
                gc_threshold = parse_usize(required_value(args, index, "--gc-threshold")?)?;
            }
            "--diagnostics" => diagnostics = true,
            "--thread-stack-mib" => {
                index += 1;
                thread_stack_mib = parse_usize(required_value(args, index, "--thread-stack-mib")?)?;
                if !(MIN_JETSTREAM_THREAD_STACK_MIB..=MAX_JETSTREAM_THREAD_STACK_MIB)
                    .contains(&thread_stack_mib)
                {
                    return Err(format!(
                        "--thread-stack-mib must be in {MIN_JETSTREAM_THREAD_STACK_MIB}..={MAX_JETSTREAM_THREAD_STACK_MIB}"
                    ));
                }
            }
            argument if argument.starts_with('-') => {
                return Err(format!("unknown jetstream option {argument}"));
            }
            argument if path.is_none() => path = Some(argument.to_string()),
            _ => {
                return Err(
                    "usage: agentjs jetstream <generated-runner.js> --resource-root <JetStream2-root> [--loop-limit N] [--wall-clock-seconds N] [--gc-threshold N] [--thread-stack-mib N] [--diagnostics]"
                        .into(),
                );
            }
        }
        index += 1;
    }
    let path = path.ok_or_else(|| {
        "usage: agentjs jetstream <generated-runner.js> --resource-root <JetStream2-root> [--loop-limit N] [--wall-clock-seconds N] [--gc-threshold N] [--thread-stack-mib N] [--diagnostics]"
            .to_string()
    })?;
    let resource_root = resource_root
        .ok_or_else(|| "--resource-root is required for JetStream runners".to_string())?;
    let run_started = Instant::now();
    let run_control = RunControl {
        deadline: AbsoluteDeadline::from_duration(run_started, wall_clock_limit),
    };
    run_control
        .deadline
        .check()
        .map_err(|error| error.to_string())?;
    if diagnostics {
        eprintln!("runner_read_start:{}", path);
    }
    let source = fs::read_to_string(&path).map_err(|error| format!("{path}: {error}"))?;
    if diagnostics {
        eprintln!("runner_read_end:bytes={}", source.len());
        eprintln!(
            "run_control:thread_stack_mib={} deadline_ms={}",
            thread_stack_mib,
            wall_clock_limit.map_or_else(|| "none".into(), |limit| limit.as_millis().to_string())
        );
    }
    run_control
        .deadline
        .check()
        .map_err(|error| error.to_string())?;
    std::thread::Builder::new()
        .stack_size(thread_stack_mib * 1024 * 1024)
        .spawn(move || {
            jetstream_run(
                &source,
                resource_root,
                loop_limit,
                run_control,
                run_started,
                gc_threshold,
                diagnostics,
            )
        })
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "JetStream thread panicked".to_string())?
}

fn jetstream_run(
    source: &str,
    resource_root: PathBuf,
    loop_limit: u64,
    run_control: RunControl,
    run_started: Instant,
    gc_threshold: usize,
    diagnostics: bool,
) -> Result<(), String> {
    run_control
        .deadline
        .check()
        .map_err(|error| error.to_string())?;
    let loader = Arc::new(RootedFileLoader::new(resource_root).map_err(|error| error.to_string())?);
    let host = HostServices {
        file_loader: Some(loader.clone()),
    };
    let mut runtime = Runtime::with_host(
        RuntimeConfig {
            loop_limit,
            recursion_limit: 8_192,
            stack_limit: 8 * 1024 * 1024,
            backtrace_limit: 20,
            script_cache_capacity: 0,
            install_test262_host: true,
            install_jetstream_host: true,
            diagnostics,
            heap_object_limit: usize::MAX,
            heap_byte_limit: usize::MAX,
            wall_clock_limit: None,
            // Large benchmark-owned arrays are long-lived. Collecting in the
            // middle of a JetStream iteration distorts timing and can sweep
            // harness values that are still reachable through benchmark state.
            gc_allocation_threshold: gc_threshold,
        },
        host,
    )
    .map_err(|error| error.to_string())?;
    run_control
        .deadline
        .check()
        .map_err(|error| error.to_string())?;
    runtime.set_run_control(Some(run_control));
    let (prelude, launch) = source
        .split_once("/*__AGENTJS_LOAD_RESOURCES__*/")
        .ok_or_else(|| "JetStream runner is missing the resource-load boundary".to_string())?;
    runtime.set_diagnostic_phase("prelude");
    runtime
        .eval(prelude, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    for line in prelude.lines() {
        let Some(path) = line.strip_prefix("// AGENTJS_RESOURCE:") else {
            continue;
        };
        if diagnostics {
            eprintln!("resource_read_start:{path}");
        }
        run_control
            .deadline
            .check()
            .map_err(|error| error.to_string())?;
        let resource = loader
            .read_text(std::path::Path::new(path))
            .map_err(|error| error.to_string())?;
        run_control
            .deadline
            .check()
            .map_err(|error| error.to_string())?;
        if diagnostics {
            eprintln!("resource_read_end:{path}:bytes={}", resource.len());
        }
        runtime.set_diagnostic_phase("resource");
        runtime
            .eval(resource.as_ref(), ExecutionOptions::default())
            .map_err(|error| format!("{path}: {error}"))?;
    }
    runtime.set_diagnostic_phase("launch");
    let report = runtime
        .eval(launch, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    let benchmark_failure = report
        .output
        .iter()
        .find(|line| line.starts_with("JetStream2 failed:"))
        .cloned();
    print_report(report);
    if let Some(failure) = benchmark_failure {
        return Err(failure);
    }
    if diagnostics {
        eprintln!("run_end:elapsed_ms={}", run_started.elapsed().as_millis());
    }
    Ok(())
}

fn command_repl(args: &[String]) -> Result<(), String> {
    let _backend = parse_backend_only_args(args, "usage: agentjs repl [--backend native]")?;
    let mut runtime = Runtime::new(RuntimeConfig::default()).map_err(|error| error.to_string())?;
    let stdin = io::stdin();
    println!(
        "AgentJS {} (native) - Ctrl-D to exit",
        env!("CARGO_PKG_VERSION")
    );

    loop {
        print!("agentjs:native> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        if stdin
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            println!();
            break;
        }
        match runtime.eval(&line, ExecutionOptions::default()) {
            Ok(report) => {
                for line in report.output {
                    println!("{line}");
                }
                if report.value != "undefined" {
                    println!("{}", report.value);
                }
            }
            Err(error) => eprintln!("{error}"),
        }
    }
    Ok(())
}

fn command_test262(args: &[String]) -> Result<(), String> {
    let mut options = RunnerOptions::default();
    let mut json_path = None;
    let mut verbose = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                options.test262_root = PathBuf::from(required_value(args, index, "--root")?);
            }
            "--suite" => {
                index += 1;
                options.suite = PathBuf::from(required_value(args, index, "--suite")?);
            }
            "--filter" => {
                index += 1;
                options.filter = Some(required_value(args, index, "--filter")?.to_string());
            }
            "--limit" => {
                index += 1;
                options.limit = Some(parse_usize(required_value(args, index, "--limit")?)?);
            }
            "--jobs" => {
                index += 1;
                options.jobs = parse_usize(required_value(args, index, "--jobs")?)?;
            }
            "--backend" => {
                index += 1;
                options.backend = parse_backend(required_value(args, index, "--backend")?)?;
            }
            "--native-v1" => options.select_native_v1(),
            "--native-v2" => options.select_native_v2(),
            "--native-v3" => options.select_native_v3(),
            "--native-v4" => options.select_native_v4(),
            "--native-v4-scan" => options.select_native_v4_scan(),
            "--native-v5" => options.select_native_v5(),
            "--native-v5-scan" => options.select_native_v5_scan(),
            "--native-v6" => options.select_native_v6(),
            "--native-v6-scan" => options.select_native_v6_scan(),
            "--native-v7" => options.select_native_v7(),
            "--native-v7-scan" => options.select_native_v7_scan(),
            "--native-v8-scan" => options.select_native_v8_scan(),
            "--native-v9-scan" => options.select_native_v9_scan(),
            "--native-v10-scan" => options.select_native_v10_scan(),
            "--native-v11-scan" => options.select_native_v11_scan(),
            "--json" => {
                index += 1;
                json_path = Some(PathBuf::from(required_value(args, index, "--json")?));
            }
            "--progress" => options.progress = true,
            "--skip-runtime-errors" => options.skip_runtime_errors = true,
            "--verbose" | "-v" => verbose = true,
            unknown => return Err(format!("unknown test262 option `{unknown}`")),
        }
        index += 1;
    }

    let summary = agentjs::test262::run(options)?;
    if verbose {
        for case in &summary.cases {
            if case.status != Status::Passed {
                println!(
                    "{:?}\t{}\t{}",
                    case.status,
                    case.path.display(),
                    case.detail
                );
            }
        }
    }
    println!(
        "total={} passed={} failed={} skipped={} conformance={:.2}% elapsed={:.2}s",
        summary.total,
        summary.passed,
        summary.failed,
        summary.skipped,
        summary.conformance_percent(),
        summary.elapsed.as_secs_f64()
    );

    if let Some(path) = json_path {
        fs::write(&path, summary.to_json())
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    }
    Ok(())
}

fn command_bench(args: &[String]) -> Result<(), String> {
    let mut iter_arg: Option<&str> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--backend" => {
                index += 1;
                let _backend = parse_backend(required_value(args, index, "--backend")?)?;
            }
            value if value.starts_with("--") => {
                return Err(format!(
                    "unknown option `{value}`; usage: agentjs bench [--backend native] [iterations]"
                ));
            }
            value if iter_arg.is_none() => iter_arg = Some(value),
            _ => {
                return Err("usage: agentjs bench [--backend native] [iterations]".into());
            }
        }
        index += 1;
    }
    let iterations = match iter_arg {
        Some(value) => parse_usize(value)?,
        None => 1_000,
    };
    if iterations == 0 {
        return Err("benchmark iterations must be greater than zero".into());
    }
    let source = "(function(){ let x = 0; for (let i = 0; i < 1000; i++) x += i; return x; })()";
    bench_native(source, iterations)
}

fn bench_native(source: &str, iterations: usize) -> Result<(), String> {
    use agentjs::backend::NativeRuntime;

    println!("backend=native iterations={iterations}");

    // Cold: fresh isolate per iteration via Engine.
    let cold_started = Instant::now();
    let engine = Engine::new(RuntimeConfig::default());
    for _ in 0..iterations {
        engine
            .execute(source, ExecutionOptions::default())
            .map_err(|error| error.to_string())?;
    }
    let cold = cold_started.elapsed();

    // Warm uncached: persistent isolate, cache disabled.
    let mut uncached_runtime = NativeRuntime::new(RuntimeConfig {
        script_cache_capacity: 0,
        ..RuntimeConfig::default()
    });
    let uncached_started = Instant::now();
    for _ in 0..iterations {
        uncached_runtime
            .eval_source(source, ExecutionOptions::default())
            .map_err(|error| error.to_string())?;
    }
    let uncached = uncached_started.elapsed();

    // Warm cached: persistent isolate, LRU cache enabled.
    let mut cached_runtime = NativeRuntime::new(RuntimeConfig::default());
    let cached_started = Instant::now();
    for _ in 0..iterations {
        cached_runtime
            .eval_source(source, ExecutionOptions::default())
            .map_err(|error| error.to_string())?;
    }
    let cached = cached_started.elapsed();

    print_bench_results(cold, uncached, cached, iterations);
    println!(
        "cache_hits={} cache_misses={}",
        cached_runtime.cache_stats().hits,
        cached_runtime.cache_stats().misses,
    );
    Ok(())
}

fn print_bench_results(
    cold: std::time::Duration,
    uncached: std::time::Duration,
    cached: std::time::Duration,
    iterations: usize,
) {
    println!(
        "cold_total_ms={} cold_avg_us={:.2}",
        cold.as_millis(),
        cold.as_secs_f64() * 1_000_000.0 / iterations as f64
    );
    println!(
        "warm_uncached_total_ms={} warm_uncached_avg_us={:.2}",
        uncached.as_millis(),
        uncached.as_secs_f64() * 1_000_000.0 / iterations as f64
    );
    println!(
        "warm_cached_total_ms={} warm_cached_avg_us={:.2}",
        cached.as_millis(),
        cached.as_secs_f64() * 1_000_000.0 / iterations as f64
    );
}

fn print_report(report: agentjs::ExecutionReport) {
    for line in report.output {
        println!("{line}");
    }
    if report.value != "undefined" {
        println!("{}", report.value);
    }
}

fn required_value<'a>(args: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_usize(value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("`{value}` is not a positive integer"))
}

fn parse_backend_prefixed_args<'a>(
    args: &'a [String],
    usage: &str,
) -> Result<(BackendKind, &'a [String]), String> {
    let mut backend = BackendKind::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--backend" => {
                index += 1;
                backend = parse_backend(required_value(args, index, "--backend")?)?;
                index += 1;
            }
            "--" => {
                index += 1;
                break;
            }
            value if value.starts_with("--") => {
                return Err(format!("unknown option `{value}`; {usage}"));
            }
            _ => break,
        }
    }

    if index >= args.len() {
        return Err(usage.into());
    }

    Ok((backend, &args[index..]))
}

fn parse_backend_only_args(args: &[String], usage: &str) -> Result<BackendKind, String> {
    let mut backend = BackendKind::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--backend" => {
                index += 1;
                backend = parse_backend(required_value(args, index, "--backend")?)?;
                index += 1;
            }
            "--" => {
                index += 1;
                break;
            }
            value => return Err(format!("unknown option `{value}`; {usage}")),
        }
    }

    if index != args.len() {
        return Err(usage.into());
    }

    Ok(backend)
}

fn parse_backend(value: &str) -> Result<BackendKind, String> {
    match value {
        "native" => Ok(BackendKind::Native),
        "boa" => Err("the embedded Boa backend was removed in V12; \
             build the external Boa CLI for comparison experiments: \
             `cargo build --release --manifest-path boa/Cargo.toml -p boa_cli`"
            .into()),
        _ => Err(format!("unknown backend `{value}`; expected `native`")),
    }
}

fn print_help() {
    println!(
        "\
AgentJS - lightweight JavaScript execution for AI agents

USAGE:
  agentjs eval [--backend native] <source>
  agentjs run [--backend native] <file.js>
  agentjs jetstream <generated-runner.js>
                  --resource-root <JetStream2-root>
                  [--loop-limit N] [--wall-clock-seconds N]
                  [--gc-threshold N] [--thread-stack-mib N] [--diagnostics]
  agentjs repl [--backend native]
  agentjs test262 [--root test262] [--suite test] [--filter text]
                  [--backend native] [--limit N] [--jobs N]
                  [--native-v1|--native-v2|--native-v3|--native-v4|--native-v4-scan|--native-v5|--native-v5-scan|--native-v6|--native-v6-scan|--native-v7|--native-v7-scan|--native-v8-scan|--native-v9-scan|--native-v10-scan|--native-v11-scan]
                  [--progress] [--skip-runtime-errors] [--json result.json] [-v]
  agentjs bench [--backend native] [iterations]

BACKENDS:
  native  Default self-developed engine (always available).
  boa     Removed from embedded dispatch in V12. Build the external Boa CLI
          for comparison: cargo build --release --manifest-path boa/Cargo.toml -p boa_cli"
    );
}

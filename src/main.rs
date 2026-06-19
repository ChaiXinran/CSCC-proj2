use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
    time::Instant,
};

use agentjs::{
    BackendKind, Engine, ExecutionOptions, Runtime, RuntimeConfig,
    test262::{RunnerOptions, Status},
};

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
    let (backend, source_args) =
        parse_backend_prefixed_args(args, "usage: agentjs eval [--backend boa|native] <source>")?;
    let source = source_args.join(" ");
    let report = Engine::with_backend(backend, RuntimeConfig::default())
        .execute(&source, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    print_report(report);
    Ok(())
}

fn command_run(args: &[String]) -> Result<(), String> {
    let (backend, file_args) =
        parse_backend_prefixed_args(args, "usage: agentjs run [--backend boa|native] <file.js>")?;
    let path = file_args
        .first()
        .ok_or_else(|| "usage: agentjs run [--backend boa|native] <file.js>".to_string())?;
    let source = fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let report = Engine::with_backend(backend, RuntimeConfig::default())
        .execute(&source, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    print_report(report);
    Ok(())
}

fn command_jetstream(args: &[String]) -> Result<(), String> {
    let path = args
        .first()
        .ok_or_else(|| "usage: agentjs jetstream <generated-runner.js>".to_string())?;
    let source = fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let mut runtime = Runtime::new(RuntimeConfig {
        loop_limit: u64::MAX,
        recursion_limit: 1_024,
        stack_limit: 1024 * 1024,
        backtrace_limit: 20,
        script_cache_capacity: 0,
        install_test262_host: false,
    })
    .map_err(|error| error.to_string())?;
    let report = runtime
        .eval(&source, ExecutionOptions::default())
        .map_err(|error| error.to_string())?;
    print_report(report);
    Ok(())
}

fn command_repl(args: &[String]) -> Result<(), String> {
    let backend = parse_backend_only_args(args, "usage: agentjs repl [--backend boa|native]")?;
    let mut runtime = Runtime::with_backend(backend, RuntimeConfig::default())
        .map_err(|error| error.to_string())?;
    let stdin = io::stdin();
    println!(
        "AgentJS {} ({}) - Ctrl-D to exit",
        env!("CARGO_PKG_VERSION"),
        backend.name()
    );

    loop {
        print!("agentjs:{}> ", backend.name());
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
            "--json" => {
                index += 1;
                json_path = Some(PathBuf::from(required_value(args, index, "--json")?));
            }
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
    let iterations = match args.first() {
        Some(value) => parse_usize(value)?,
        None => 1_000,
    };
    if iterations == 0 {
        return Err("benchmark iterations must be greater than zero".into());
    }
    let source = "(function(){ let x = 0; for (let i = 0; i < 1000; i++) x += i; return x; })()";

    let cold_started = Instant::now();
    let engine = Engine::default();
    for _ in 0..iterations {
        engine
            .execute(source, ExecutionOptions::default())
            .map_err(|error| error.to_string())?;
    }
    let cold = cold_started.elapsed();

    let mut uncached_runtime = Runtime::new(RuntimeConfig {
        script_cache_capacity: 0,
        ..RuntimeConfig::default()
    })
    .map_err(|error| error.to_string())?;
    let uncached_started = Instant::now();
    for _ in 0..iterations {
        uncached_runtime
            .eval(source, ExecutionOptions::default())
            .map_err(|error| error.to_string())?;
    }
    let uncached = uncached_started.elapsed();

    let mut cached_runtime =
        Runtime::new(RuntimeConfig::default()).map_err(|error| error.to_string())?;
    let cached_started = Instant::now();
    for _ in 0..iterations {
        cached_runtime
            .eval(source, ExecutionOptions::default())
            .map_err(|error| error.to_string())?;
    }
    let cached = cached_started.elapsed();

    println!("iterations={iterations}");
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
    Ok(())
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
    let mut backend = BackendKind::Boa;
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
    let mut backend = BackendKind::Boa;
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
        "boa" => Ok(BackendKind::Boa),
        "native" => Ok(BackendKind::Native),
        _ => Err(format!(
            "unknown backend `{value}`; expected `boa` or `native`"
        )),
    }
}

trait BackendName {
    fn name(self) -> &'static str;
}

impl BackendName for BackendKind {
    fn name(self) -> &'static str {
        match self {
            Self::Boa => "boa",
            Self::Native => "native",
        }
    }
}

fn print_help() {
    println!(
        "\
AgentJS - lightweight JavaScript execution for AI agents

USAGE:
  agentjs eval [--backend boa|native] <source>
  agentjs run [--backend boa|native] <file.js>
  agentjs jetstream <generated-runner.js>
  agentjs repl [--backend boa|native]
  agentjs test262 [--root test262] [--suite test] [--filter text]
                  [--backend boa|native] [--limit N] [--jobs N]
                  [--native-v1|--native-v2|--native-v3] [--json result.json] [-v]
  agentjs bench [iterations]"
    );
}

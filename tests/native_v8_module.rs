use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use agentjs::{
    FailureKind,
    backend::NativeRuntime,
    engine::{ExecutionOptions, RuntimeConfig},
    runtime::{ModuleStatus, resolve_module_specifier},
};

#[test]
fn module_source_runs_in_strict_mode() {
    let path = temp_module_path("strict-mode");
    let mut runtime = NativeRuntime::new(RuntimeConfig::default());

    let value = runtime
        .eval_module_source(
            "function f() { return this === undefined; } f();",
            &path,
            true,
        )
        .expect("module source should execute");

    assert_eq!(value, "true");
    assert_eq!(
        runtime.module_status_for_path(&path),
        Some(ModuleStatus::Evaluated)
    );
    let record = runtime
        .module_record_for_path(&path)
        .expect("module record should be stored");
    assert!(record.imports.is_empty());
    assert!(record.exports.is_empty());
}

#[test]
fn module_top_level_this_is_undefined() {
    let path = temp_module_path("top-level-this");
    let mut runtime = NativeRuntime::new(RuntimeConfig::default());

    let value = runtime
        .eval_module_source("this === undefined;", &path, true)
        .expect("module source should execute");

    assert_eq!(value, "true");
}

#[test]
fn module_registry_prevents_duplicate_evaluation_for_same_path() {
    let path = temp_module_path("duplicate");
    let mut runtime = NativeRuntime::new(RuntimeConfig::default());

    runtime
        .eval_module_source(
            "globalThis.__v8ModuleCount = (globalThis.__v8ModuleCount || 0) + 1;",
            &path,
            true,
        )
        .expect("first module evaluation should run");
    runtime
        .eval_module_source(
            "globalThis.__v8ModuleCount = (globalThis.__v8ModuleCount || 0) + 1;",
            &path,
            true,
        )
        .expect("second module evaluation should be deduplicated");

    let count = runtime
        .eval_source("globalThis.__v8ModuleCount;", ExecutionOptions::default())
        .expect("module side effect should be visible to later script code");

    assert_eq!(runtime.module_registry_len(), 1);
    assert_eq!(count, "1");
}

#[test]
fn relative_module_dependency_loader_reuses_registry() {
    let dir = temp_module_dir("relative-loader");
    let importer = dir.join("main.mjs");
    let dependency = dir.join("dep.mjs");
    fs::write(&importer, "").expect("temporary importer should be writable");
    fs::write(
        &dependency,
        "globalThis.__v8DependencyCount = (globalThis.__v8DependencyCount || 0) + 1;",
    )
    .expect("temporary dependency should be writable");

    let mut runtime = NativeRuntime::new(RuntimeConfig::default());
    runtime
        .load_module_dependency(&importer, "./dep.mjs", true)
        .expect("relative dependency should load");
    runtime
        .load_module_dependency(&importer, "./dep.mjs", true)
        .expect("relative dependency should be deduplicated");

    let count = runtime
        .eval_source(
            "globalThis.__v8DependencyCount;",
            ExecutionOptions::default(),
        )
        .expect("dependency side effect should be visible");

    assert_eq!(runtime.module_registry_len(), 1);
    assert_eq!(count, "1");
    assert_eq!(
        resolve_module_specifier(&importer, "./dep.mjs").unwrap(),
        fs::canonicalize(&dependency).unwrap()
    );
}

#[test]
fn bare_module_specifier_is_explicitly_unsupported() {
    let importer = temp_module_path("bare-specifier");
    let mut runtime = NativeRuntime::new(RuntimeConfig::default());

    let error = runtime
        .load_module_dependency(&importer, "not-relative", true)
        .expect_err("bare specifiers are outside V8-B scope");

    assert_eq!(error.kind, FailureKind::Unsupported);
    assert!(error.message.contains("unsupported module specifier"));
}

fn temp_module_path(label: &str) -> PathBuf {
    let path = temp_module_dir(label).join("module.mjs");
    fs::write(&path, "").expect("temporary module path should be writable");
    path
}

fn temp_module_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "agentjs-v8-module-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temporary module directory should be writable");
    path
}

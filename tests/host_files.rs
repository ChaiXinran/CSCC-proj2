use std::{path::PathBuf, sync::Arc};

use agentjs::{
    ExecutionOptions, FailureKind, HostServices, RootedFileLoader, Runtime, RuntimeConfig,
};

#[test]
fn ordinary_runtime_does_not_expose_read_file() {
    let mut runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    let error = runtime
        .eval("readFile('./Cargo.toml')", ExecutionOptions::default())
        .unwrap_err();
    assert_eq!(error.kind, FailureKind::Reference);
}

#[test]
fn opted_in_runtime_reads_only_below_its_root() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let host = HostServices {
        file_loader: Some(Arc::new(RootedFileLoader::new(root).unwrap())),
    };
    let mut runtime = Runtime::with_host(
        RuntimeConfig {
            install_jetstream_host: true,
            ..RuntimeConfig::default()
        },
        host,
    )
    .unwrap();

    let report = runtime
        .eval(
            "readFile('./Cargo.toml').includes('[package]')",
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "true");

    let error = runtime
        .eval("readFile('../Cargo.toml')", ExecutionOptions::default())
        .unwrap_err();
    assert!(error.message.contains("escapes resource root"));
}

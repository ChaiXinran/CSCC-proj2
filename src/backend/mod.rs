//! JavaScript execution backend boundary.
//!
//! The native backend is AgentJS's sole self-developed parser, bytecode
//! compiler, virtual machine, and runtime. Boa remains available as an external
//! comparison engine through the pinned submodule (`boa/`), built separately via
//! `cargo build --release --manifest-path boa/Cargo.toml -p boa_cli`.

use std::{
    collections::VecDeque,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    ast::ModuleDeclaration,
    builtins,
    contracts::{
        ChunkCacheMetadata, NativeContext, NativeError, NativePipeline, Program, SharedChunk,
        VmErrorKind,
    },
    engine::{
        EvalFailure, ExecutionOptions, FailureKind, FrontendControl, PhaseDiagnostics, RunControl,
        RuntimeConfig, SourceKind,
    },
    host::HostServices,
    lexer::Lexer,
    parser::Parser,
    runtime::{
        DynamicImportRequest, GcMetrics, JsValue, ModuleEvaluationState, ModuleExportBinding,
        ModuleImportBinding, ModuleRegistry, ModuleStatus, NativeErrorKind,
        resolve_module_specifier,
    },
};

/// Selects the JavaScript implementation used by [`crate::Runtime`].
///
/// V12 removed the embedded Boa dispatch path. Only the native self-developed
/// engine remains in-tree. Boa is still available as an external reference
/// engine built from the pinned submodule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendKind {
    /// AgentJS's self-developed backend.
    #[default]
    Native,
}

/// Backend-neutral result produced by one evaluation.
#[derive(Debug)]
pub(crate) struct BackendExecution {
    pub value: String,
    pub output: Vec<String>,
}

/// Internal contract implemented by every persistent JavaScript isolate.
pub(crate) trait RuntimeBackend {
    fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure>;

    fn parse_only(&mut self, source: &str, options: ExecutionOptions) -> Result<(), EvalFailure>;

    fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure>;

    fn eval_module_source(
        &mut self,
        source: &str,
        path: &Path,
        drain_jobs: bool,
    ) -> Result<BackendExecution, EvalFailure>;

    fn run_jobs(&mut self) -> Result<(), EvalFailure>;

    fn set_strict(&mut self, strict: bool);

    fn set_dynamic_import_referrer(&mut self, path: &Path);

    fn clear_output(&mut self);

    fn take_output(&mut self) -> Vec<String>;

    fn set_run_control(&mut self, control: Option<RunControl>);

    fn set_diagnostic_phase(&mut self, phase: &'static str);
}

pub(crate) fn create_runtime(
    kind: BackendKind,
    config: RuntimeConfig,
) -> Result<Box<dyn RuntimeBackend>, EvalFailure> {
    match kind {
        BackendKind::Native => Ok(Box::new(NativeRuntime::new(config))),
    }
}

pub(crate) fn create_runtime_with_host(
    kind: BackendKind,
    config: RuntimeConfig,
    host: HostServices,
) -> Result<Box<dyn RuntimeBackend>, EvalFailure> {
    match kind {
        BackendKind::Native => Ok(Box::new(NativeRuntime::with_host(config, host))),
    }
}

// ---------------------------------------------------------------------------
// NativeRuntime – script cache, parse, compile, execute
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeScriptCacheKey {
    source_hash: u64,
    strict: bool,
    source_kind: SourceKind,
}

#[derive(Debug, Clone)]
struct NativeScriptCacheEntry {
    key: NativeScriptCacheKey,
    chunk: SharedChunk,
    metadata: ChunkCacheMetadata,
}

impl NativeScriptCacheEntry {
    fn cached_chunk(&self) -> SharedChunk {
        let _metadata = self.metadata;
        std::sync::Arc::clone(&self.chunk)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeScriptCacheStats {
    pub hits: u64,
    pub misses: u64,
}

/// Self-developed AgentJS runtime.
///
/// This type owns one native isolate, including its runtime context, VM pipeline,
/// and V7 isolate-local script cache.
pub struct NativeRuntime {
    config: RuntimeConfig,
    context: NativeContext,
    pipeline: NativePipeline,
    script_cache: VecDeque<NativeScriptCacheEntry>,
    cache_stats: NativeScriptCacheStats,
    module_registry: ModuleRegistry,
    current_source_kind: SourceKind,
    run_control: Option<RunControl>,
    diagnostic_phase: &'static str,
    last_token_count: usize,
}

impl NativeRuntime {
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        Self::with_host(config, HostServices::default())
    }

    pub fn with_host(config: RuntimeConfig, host: HostServices) -> Self {
        let mut context =
            NativeContext::with_heap_limits(config.heap_object_limit, config.heap_byte_limit);
        context.install_host_services(host);
        context.configure_heap_limits(config.heap_byte_limit, config.gc_allocation_threshold);
        builtins::install_foundation(&mut context);
        if config.install_test262_host {
            builtins::install_test262_harness(&mut context);
        }
        if config.install_jetstream_host {
            crate::host::install_jetstream_host(&mut context)
                .expect("install JetStream host services");
        }
        Self {
            config,
            context,
            pipeline: NativePipeline::default(),
            script_cache: VecDeque::new(),
            cache_stats: NativeScriptCacheStats::default(),
            module_registry: ModuleRegistry::default(),
            current_source_kind: SourceKind::Script,
            run_control: None,
            diagnostic_phase: "eval",
            last_token_count: 0,
        }
    }

    fn reset_limits(&mut self) {
        self.context.reset_execution_budget(self.config.loop_limit);
        self.context
            .reset_call_depth(self.config.recursion_limit as u64);
        self.context.reset_stack_limit(self.config.stack_limit);
        if let Some(control) = self.run_control {
            self.context
                .set_absolute_deadline(control.deadline.instant());
        } else {
            self.context.reset_deadline(self.config.wall_clock_limit);
        }
    }

    fn evaluate(&mut self, source: &str) -> Result<crate::runtime::JsValue, EvalFailure> {
        self.reset_limits();
        self.check_run_deadline()?;
        let chunk = self.prepare_chunk(source).map_err(classify_native_error)?;
        self.check_run_deadline()?;
        if self.config.diagnostics {
            eprintln!("execute_start");
            self.context.reset_name_resolution_metrics();
        }
        let execute_started = Instant::now();
        let result = self
            .pipeline
            .execute(&chunk, &mut self.context)
            .map_err(classify_native_error);
        if self.config.diagnostics {
            eprintln!("execute_end");
            self.emit_phase_diagnostics(
                match self.diagnostic_phase {
                    "prelude" => "prelude_execute",
                    "resource" => "resource_execute",
                    "launch" => "launch_execute",
                    _ => "execute",
                },
                execute_started,
                source.len(),
                Some(self.last_token_count),
                chunk.cache_metadata().ok(),
            );
            let metrics = self.context.name_resolution_metrics();
            eprintln!(
                "name_resolution:load_local_count={} store_local_count={} load_name_count={} store_name_count={} environment_hops={}",
                metrics.load_local_count,
                metrics.store_local_count,
                metrics.load_name_count,
                metrics.store_name_count,
                metrics.environment_hops
            );
        }
        result
    }

    fn check_run_deadline(&self) -> Result<(), EvalFailure> {
        self.context
            .check_deadline()
            .map_err(|error| classify_native_error(NativeError::Execute(error)))
    }

    fn emit_phase_diagnostics(
        &self,
        phase: &'static str,
        started: Instant,
        source_bytes: usize,
        token_count: Option<usize>,
        metadata: Option<ChunkCacheMetadata>,
    ) {
        if !self.config.diagnostics {
            return;
        }
        let instruction_count = metadata.map(|value| value.total_instructions);
        let constant_count = metadata.map(|value| value.total_constants);
        let function_count = metadata.map(|value| value.total_functions);
        let diagnostic = PhaseDiagnostics {
            phase,
            elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            source_bytes,
            token_count,
            instruction_count,
            constant_count,
            function_count,
            heap: self.context.heap_stats(),
            gc: self.context.gc_metrics(),
        };
        eprintln!(
            "phase_diagnostics:phase={} elapsed_ms={} source_bytes={} token_count={} instruction_count={} constant_count={} function_count={} heap_estimated_bytes={} heap_live_objects={} heap_live_environments={} heap_live_functions={} gc_count={} gc_total_pause_ns={} gc_max_pause_ns={}",
            diagnostic.phase,
            diagnostic.elapsed_ms,
            diagnostic.source_bytes,
            diagnostic
                .token_count
                .map_or_else(|| "-".into(), |value| value.to_string()),
            diagnostic
                .instruction_count
                .map_or_else(|| "-".into(), |value| value.to_string()),
            diagnostic
                .constant_count
                .map_or_else(|| "-".into(), |value| value.to_string()),
            diagnostic
                .function_count
                .map_or_else(|| "-".into(), |value| value.to_string()),
            diagnostic.heap.estimated_bytes,
            diagnostic.heap.live_objects,
            diagnostic.heap.live_environments,
            diagnostic.heap.live_functions,
            diagnostic.gc.collection_count,
            diagnostic.gc.total_pause_ns,
            diagnostic.gc.max_pause_ns,
        );
    }

    fn prepare_chunk(&mut self, source: &str) -> Result<SharedChunk, NativeError> {
        if self.config.script_cache_capacity == 0 {
            let parse_started = Instant::now();
            if self.config.diagnostics {
                eprintln!("parse_start");
            }
            let program = self.parse_current_source(source)?;
            self.emit_phase_diagnostics(
                match self.diagnostic_phase {
                    "prelude" => "prelude_parse",
                    "resource" => "resource_parse",
                    "launch" => "launch_parse",
                    _ => "parse",
                },
                parse_started,
                source.len(),
                Some(self.last_token_count),
                None,
            );
            if self.config.diagnostics {
                eprintln!("parse_end");
                eprintln!("compile_start");
            }
            let compile_started = Instant::now();
            let chunk = self.pipeline.compile(&program);
            if self.config.diagnostics {
                eprintln!("compile_end");
            }
            let chunk = chunk?;
            let metadata = chunk.cache_metadata().ok();
            self.emit_phase_diagnostics(
                match self.diagnostic_phase {
                    "prelude" => "prelude_compile",
                    "resource" => "resource_compile",
                    "launch" => "launch_compile",
                    _ => "compile",
                },
                compile_started,
                source.len(),
                None,
                metadata,
            );
            return Ok(chunk);
        }

        let key = NativeScriptCacheKey {
            source_hash: hash_source(source),
            strict: self.context.strict(),
            source_kind: self.current_source_kind,
        };
        if let Some(index) = self.script_cache.iter().position(|entry| entry.key == key) {
            let entry = self
                .script_cache
                .remove(index)
                .expect("cache index came from position");
            let chunk = entry.cached_chunk();
            self.script_cache.push_back(entry);
            self.cache_stats.hits = self.cache_stats.hits.saturating_add(1);
            return Ok(chunk);
        }

        self.cache_stats.misses = self.cache_stats.misses.saturating_add(1);
        if self.config.diagnostics {
            eprintln!("parse_start");
        }
        let parse_started = Instant::now();
        let program = self.parse_current_source(source)?;
        self.emit_phase_diagnostics(
            "parse",
            parse_started,
            source.len(),
            Some(self.last_token_count),
            None,
        );
        if self.config.diagnostics {
            eprintln!("parse_end");
            eprintln!("compile_start");
        }
        let compile_started = Instant::now();
        let chunk = self.pipeline.compile(&program)?;
        if self.config.diagnostics {
            eprintln!("compile_end");
        }
        let metadata = chunk.cache_metadata().map_err(|error| {
            NativeError::Execute(crate::vm::VmError::runtime(format!(
                "invalid cache-safe bytecode: {error}"
            )))
        })?;
        self.emit_phase_diagnostics(
            "compile",
            compile_started,
            source.len(),
            None,
            Some(metadata),
        );
        let entry = NativeScriptCacheEntry {
            key,
            chunk: std::sync::Arc::clone(&chunk),
            metadata,
        };
        if self.script_cache.len() == self.config.script_cache_capacity {
            self.script_cache.pop_front();
        }
        self.script_cache.push_back(entry);
        Ok(chunk)
    }

    fn parse_current_source(&mut self, source: &str) -> Result<Program, NativeError> {
        let control = FrontendControl {
            deadline: self.run_control.unwrap_or_default().deadline,
        };
        control.checkpoint().map_err(|error| {
            NativeError::Execute(crate::vm::VmError::runtime_limit(error.to_string()))
        })?;
        if self.current_source_kind == SourceKind::Module {
            let tokens = Lexer::new(source).with_control(control).tokenize()?;
            self.last_token_count = tokens.len();
            Ok(Parser::with_source_and_control(tokens, source, control).parse_module()?)
        } else {
            let tokens = Lexer::new(source).with_control(control).tokenize()?;
            self.last_token_count = tokens.len();
            Ok(Parser::with_source_and_control(tokens, source, control).parse_program()?)
        }
    }

    pub fn eval_source(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<String, EvalFailure> {
        RuntimeBackend::eval(self, source, options).map(|execution| execution.value)
    }

    pub fn cache_stats(&self) -> NativeScriptCacheStats {
        self.cache_stats
    }

    pub fn gc_metrics(&self) -> GcMetrics {
        self.context.gc_metrics()
    }

    pub fn module_registry_len(&self) -> usize {
        self.module_registry.len()
    }

    pub fn module_status_for_path(&self, path: &Path) -> Option<ModuleStatus> {
        self.module_registry.status_for_path(path)
    }

    pub fn module_record_for_path(&self, path: &Path) -> Option<&crate::runtime::ModuleRecord> {
        self.module_registry.record_for_path(path)
    }

    pub fn module_evaluation_state_for_path(&self, path: &Path) -> Option<&ModuleEvaluationState> {
        self.module_registry.evaluation_state_for_path(path)
    }

    pub fn eval_module_source(
        &mut self,
        source: &str,
        path: &Path,
        drain_jobs: bool,
    ) -> Result<String, EvalFailure> {
        RuntimeBackend::eval_module_source(self, source, path, drain_jobs)
            .map(|execution| execution.value)
    }

    pub fn load_module_dependency(
        &mut self,
        importer_path: &Path,
        specifier: &str,
        drain_jobs: bool,
    ) -> Result<String, EvalFailure> {
        let path = resolve_module_specifier(importer_path, specifier)
            .map_err(|message| EvalFailure::new(FailureKind::Unsupported, message))?;
        let source = fs::read_to_string(&path).map_err(|error| {
            EvalFailure::new(
                FailureKind::Reference,
                format!("cannot load module `{}`: {error}", path.display()),
            )
        })?;
        self.eval_module_source(&source, &path, drain_jobs)
    }

    fn set_dynamic_import_referrer(&mut self, path: &Path) {
        self.context.declare_global(
            "__agentjs_dynamic_import_referrer",
            JsValue::String(path.to_string_lossy().into_owned().into()),
        );
    }

    /// Resolves a local dynamic import and returns its already-settled native Promise.
    ///
    /// Import attributes are accepted at this boundary for forward compatibility;
    /// V13 deliberately does not implement a host attribute resolver.
    #[allow(dead_code)] // Called by the host dynamic-import bridge as it is installed.
    pub(crate) fn dynamic_import(
        &mut self,
        request: DynamicImportRequest,
    ) -> Result<JsValue, EvalFailure> {
        let promise = self
            .context
            .create_promise()
            .map_err(|error| classify_native_error(NativeError::Execute(error)))?;
        let prototype = self
            .context
            .get_global("Promise")
            .and_then(|value| self.context.get_property(value, "prototype").ok())
            .and_then(|value| match value {
                JsValue::Object(id) => Some(id),
                _ => None,
            });
        let promise_value = self
            .context
            .create_promise_object(promise, prototype)
            .map_err(|error| classify_native_error(NativeError::Execute(error)))?;

        let outcome: Result<JsValue, EvalFailure> = (|| {
            let referrer = request.referrer.as_deref().ok_or_else(|| {
                EvalFailure::new(
                    FailureKind::Reference,
                    "dynamic import has no referrer path",
                )
            })?;
            let path: PathBuf = resolve_module_specifier(referrer, &request.specifier)
                .map_err(|message| EvalFailure::new(FailureKind::Unsupported, message))?;
            let source = fs::read_to_string(&path).map_err(|error| {
                EvalFailure::new(
                    FailureKind::Reference,
                    format!("cannot load module `{}`: {error}", path.display()),
                )
            })?;
            self.eval_module_source(&source, &path, true)?;
            Ok(JsValue::Undefined)
        })();

        match outcome {
            Ok(namespace) => self.context.fulfill_promise(promise, namespace),
            Err(error) => self.context.reject_promise(
                promise,
                JsValue::Error(crate::runtime::NativeErrorValue::new(
                    NativeErrorKind::Type,
                    error.message,
                )),
            ),
        }
        .map_err(|error| classify_native_error(NativeError::Execute(error)))?;
        Ok(promise_value)
    }
}

impl RuntimeBackend for NativeRuntime {
    fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure> {
        self.context.clear_output();
        self.current_source_kind = options.source_kind;
        self.context
            .set_strict(options.strict || options.source_kind == SourceKind::Module);
        self.context
            .set_top_level_this(if options.source_kind == SourceKind::Module {
                JsValue::Undefined
            } else {
                self.context.global_this_value()
            });
        let value = self.evaluate(source)?;
        if options.drain_jobs {
            self.run_jobs()?;
        }
        Ok(BackendExecution {
            value: value.to_string(),
            // Async Test262 callers drain the job queue after evaluation and
            // then inspect the runtime output.  Keep output resident until
            // that explicit drain/check boundary instead of consuming an
            // already-observed synchronous `$DONE` here.
            output: if options.drain_jobs {
                self.context.take_output()
            } else {
                Vec::new()
            },
        })
    }

    fn parse_only(&mut self, source: &str, options: ExecutionOptions) -> Result<(), EvalFailure> {
        self.current_source_kind = options.source_kind;
        self.context
            .set_strict(options.strict || options.source_kind == SourceKind::Module);
        self.context
            .set_top_level_this(if options.source_kind == SourceKind::Module {
                JsValue::Undefined
            } else {
                self.context.global_this_value()
            });
        let _ = self.prepare_chunk(source).map_err(classify_native_error)?;
        Ok(())
    }

    fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure> {
        self.evaluate(source).map(|_| ())
    }

    fn eval_module_source(
        &mut self,
        source: &str,
        path: &Path,
        drain_jobs: bool,
    ) -> Result<BackendExecution, EvalFailure> {
        self.context.clear_output();
        self.current_source_kind = SourceKind::Module;
        self.context.set_strict(true);
        self.context.set_top_level_this(JsValue::Undefined);

        let module_id = self.module_registry.ensure_record(path);
        match self.module_registry.status_for_path(path) {
            Some(ModuleStatus::Evaluated) => {
                return Ok(BackendExecution {
                    value: JsValue::Undefined.to_string(),
                    output: self.context.take_output(),
                });
            }
            Some(ModuleStatus::Linked) => {
                return Err(EvalFailure::new(
                    FailureKind::Unsupported,
                    format!(
                        "cyclic module graph is not supported yet at `{}`",
                        path.display()
                    ),
                ));
            }
            _ => {}
        }

        self.module_registry
            .set_status(module_id, ModuleStatus::Linked);
        self.module_registry
            .set_evaluation_state(module_id, ModuleEvaluationState::Pending);
        let outcome = (|| {
            self.reset_limits();
            let program = self.parse_current_source(source)?;
            let (dependencies, imports, exports) = collect_module_metadata(&program);
            let uses_dynamic_import = source.contains("import(")
                || source.contains("import (")
                || source.contains("import.defer");
            let has_dependencies = !dependencies.is_empty();
            self.module_registry
                .set_metadata(module_id, dependencies, imports, exports);
            if has_dependencies || uses_dynamic_import {
                crate::vm::evaluate_local_module(
                    &mut self.pipeline.executor,
                    &mut self.context,
                    path,
                )
                .map_err(NativeError::Execute)
            } else {
                let chunk = self.pipeline.compile(&program)?;
                self.pipeline.execute(&chunk, &mut self.context)
            }
        })()
        .map_err(classify_native_error);
        match outcome {
            Ok(value) => {
                if drain_jobs && let Err(error) = self.run_jobs() {
                    self.module_registry
                        .set_status(module_id, ModuleStatus::Failed);
                    self.module_registry.set_evaluation_state(
                        module_id,
                        ModuleEvaluationState::Rejected(JsValue::String(
                            error.message.clone().into(),
                        )),
                    );
                    return Err(error);
                }
                self.module_registry
                    .set_status(module_id, ModuleStatus::Evaluated);
                self.module_registry
                    .set_evaluation_state(module_id, ModuleEvaluationState::Fulfilled);
                Ok(BackendExecution {
                    value: value.to_string(),
                    // Top-level await is currently driven through the same
                    // explicit job-drain boundary as other async evaluation.
                    // Preserve completion output until that boundary.
                    output: if drain_jobs {
                        self.context.take_output()
                    } else {
                        Vec::new()
                    },
                })
            }
            Err(error) => {
                self.module_registry
                    .set_status(module_id, ModuleStatus::Failed);
                self.module_registry.set_evaluation_state(
                    module_id,
                    ModuleEvaluationState::Rejected(JsValue::String(error.message.clone().into())),
                );
                Err(error)
            }
        }
    }

    fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        self.check_run_deadline()?;
        let started = Instant::now();
        if self.config.diagnostics {
            eprintln!("job_drain_start");
        }
        let result = self
            .pipeline
            .executor
            .drain_jobs(&mut self.context)
            .map_err(|error| classify_native_error(NativeError::Execute(error)));
        if self.config.diagnostics {
            eprintln!("job_drain_end");
            self.emit_phase_diagnostics("job_drain", started, 0, None, None);
        }
        result
    }

    fn set_strict(&mut self, strict: bool) {
        self.context.set_strict(strict);
    }

    fn set_dynamic_import_referrer(&mut self, path: &Path) {
        self.set_dynamic_import_referrer(path);
    }

    fn clear_output(&mut self) {
        self.context.clear_output();
    }

    fn take_output(&mut self) -> Vec<String> {
        self.context.take_output()
    }

    fn set_run_control(&mut self, control: Option<RunControl>) {
        self.run_control = control;
        if let Some(control) = control {
            self.context
                .set_absolute_deadline(control.deadline.instant());
        } else {
            self.context.reset_deadline(None);
        }
    }

    fn set_diagnostic_phase(&mut self, phase: &'static str) {
        self.diagnostic_phase = phase;
    }
}

fn hash_source(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn collect_module_metadata(
    program: &Program,
) -> (
    Vec<String>,
    Vec<ModuleImportBinding>,
    Vec<ModuleExportBinding>,
) {
    let mut dependencies = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();

    for statement in &program.body {
        match statement {
            crate::ast::Statement::ModuleDeclaration(ModuleDeclaration::Import(decl)) => {
                push_dependency(&mut dependencies, &decl.source);
                imports.extend(decl.entries.iter().map(|entry| ModuleImportBinding {
                    source: decl.source.clone(),
                    imported_name: entry.imported_name.clone(),
                    local_name: entry.local_name.clone(),
                }));
            }
            crate::ast::Statement::ModuleDeclaration(ModuleDeclaration::Export(decl)) => {
                if let Some(source) = &decl.source {
                    push_dependency(&mut dependencies, source);
                }
                exports.extend(decl.entries.iter().map(|entry| ModuleExportBinding {
                    export_name: entry.export_name.clone(),
                    local_name: entry.local_name.clone(),
                    source: decl.source.clone(),
                }));
            }
            _ => {}
        }
    }

    (dependencies, imports, exports)
}

fn push_dependency(dependencies: &mut Vec<String>, source: &str) {
    if !dependencies.iter().any(|existing| existing == source) {
        dependencies.push(source.to_owned());
    }
}

fn classify_native_error(error: NativeError) -> EvalFailure {
    let kind = match &error {
        NativeError::Lex(error) if error.is_runtime_limit() => FailureKind::RuntimeLimit,
        NativeError::Parse(error) if error.is_runtime_limit() => FailureKind::RuntimeLimit,
        NativeError::Lex(_) | NativeError::Parse(_) => FailureKind::Syntax,
        NativeError::Compile(e) => {
            if e.is_syntax {
                FailureKind::Syntax
            } else {
                FailureKind::Unsupported
            }
        }
        NativeError::Execute(error) => match error.kind {
            VmErrorKind::Reference => FailureKind::Reference,
            VmErrorKind::Type => FailureKind::Type,
            VmErrorKind::Syntax => FailureKind::Syntax,
            VmErrorKind::Range => FailureKind::Range,
            VmErrorKind::Test262 => FailureKind::Test262,
            VmErrorKind::RuntimeLimit => FailureKind::RuntimeLimit,
            VmErrorKind::Runtime => FailureKind::Other,
        },
    };
    EvalFailure::new(kind, error.to_string())
}

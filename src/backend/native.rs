use std::{
    collections::{VecDeque, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

use crate::{
    ast::ModuleDeclaration,
    backend::{BackendExecution, RuntimeBackend},
    builtins,
    contracts::{Chunk, NativeContext, NativeError, NativePipeline, Program, VmErrorKind},
    engine::{EvalFailure, ExecutionOptions, FailureKind, RuntimeConfig, SourceKind},
    lexer::Lexer,
    parser::Parser,
    runtime::{
        JsValue, ModuleExportBinding, ModuleImportBinding, ModuleRegistry, ModuleStatus,
        resolve_module_specifier,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeScriptCacheKey {
    source_hash: u64,
    strict: bool,
    source_kind: SourceKind,
}

#[derive(Debug, Clone)]
struct NativeScriptCacheEntry {
    key: NativeScriptCacheKey,
    program: Program,
    chunk: Chunk,
    max_stack_depth: usize,
}

impl NativeScriptCacheEntry {
    fn cached_chunk(&self) -> Chunk {
        let _metadata = (self.program.body.len(), self.max_stack_depth);
        self.chunk.clone()
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
}

impl NativeRuntime {
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        let mut context =
            NativeContext::with_heap_limits(config.heap_object_limit, config.heap_byte_limit);
        context.configure_heap_limits(config.heap_byte_limit, config.gc_allocation_threshold);
        builtins::install_foundation(&mut context);
        if config.install_test262_host {
            builtins::install_test262_harness(&mut context);
        }
        Self {
            config,
            context,
            pipeline: NativePipeline::default(),
            script_cache: VecDeque::new(),
            cache_stats: NativeScriptCacheStats::default(),
            module_registry: ModuleRegistry::default(),
            current_source_kind: SourceKind::Script,
        }
    }

    fn reset_limits(&mut self) {
        self.context.reset_execution_budget(self.config.loop_limit);
        self.context
            .reset_call_depth(self.config.recursion_limit as u64);
        self.context.reset_stack_limit(self.config.stack_limit);
        self.context.reset_deadline(self.config.wall_clock_limit);
    }

    fn evaluate(&mut self, source: &str) -> Result<crate::runtime::JsValue, EvalFailure> {
        self.reset_limits();
        let chunk = self.prepare_chunk(source).map_err(classify_native_error)?;
        self.pipeline
            .execute(&chunk, &mut self.context)
            .map_err(classify_native_error)
    }

    fn prepare_chunk(&mut self, source: &str) -> Result<Chunk, NativeError> {
        if self.config.script_cache_capacity == 0 {
            let program = self.parse_current_source(source)?;
            return self.pipeline.compile(&program);
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
        let program = self.parse_current_source(source)?;
        let chunk = self.pipeline.compile(&program)?;
        let max_stack_depth = chunk
            .analyze_stack()
            .map_err(|error| {
                NativeError::Execute(crate::vm::VmError::runtime(format!(
                    "invalid bytecode stack: {error}"
                )))
            })?
            .max_depth;
        let entry = NativeScriptCacheEntry {
            key,
            program: program.clone(),
            chunk: chunk.clone(),
            max_stack_depth,
        };
        if self.script_cache.len() == self.config.script_cache_capacity {
            self.script_cache.pop_front();
        }
        self.script_cache.push_back(entry);
        Ok(chunk)
    }

    fn parse_current_source(&mut self, source: &str) -> Result<Program, NativeError> {
        if self.current_source_kind == SourceKind::Module {
            let tokens = Lexer::new(source).tokenize()?;
            Ok(Parser::with_source(tokens, source).parse_module()?)
        } else {
            self.pipeline.parse(source)
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

    pub fn module_registry_len(&self) -> usize {
        self.module_registry.len()
    }

    pub fn module_status_for_path(&self, path: &Path) -> Option<ModuleStatus> {
        self.module_registry.status_for_path(path)
    }

    pub fn module_record_for_path(&self, path: &Path) -> Option<&crate::runtime::ModuleRecord> {
        self.module_registry.record_for_path(path)
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
            output: self.context.take_output(),
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
        let outcome = (|| {
            self.reset_limits();
            let program = self.parse_current_source(source)?;
            let (dependencies, imports, exports) = collect_module_metadata(&program);
            self.module_registry
                .set_metadata(module_id, dependencies, imports, exports);
            let chunk = self.pipeline.compile(&program)?;
            self.pipeline.execute(&chunk, &mut self.context)
        })()
        .map_err(classify_native_error);
        match outcome {
            Ok(value) => {
                self.module_registry
                    .set_status(module_id, ModuleStatus::Evaluated);
                if drain_jobs {
                    self.run_jobs()?;
                }
                Ok(BackendExecution {
                    value: value.to_string(),
                    output: self.context.take_output(),
                })
            }
            Err(error) => {
                self.module_registry
                    .set_status(module_id, ModuleStatus::Failed);
                Err(error)
            }
        }
    }

    fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        self.pipeline
            .executor
            .drain_jobs(&mut self.context)
            .map_err(|error| classify_native_error(NativeError::Execute(error)))
    }

    fn set_strict(&mut self, strict: bool) {
        self.context.set_strict(strict);
    }

    fn clear_output(&mut self) {
        self.context.clear_output();
    }

    fn take_output(&mut self) -> Vec<String> {
        self.context.take_output()
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
        NativeError::Lex(_) | NativeError::Parse(_) => FailureKind::Syntax,
        NativeError::Compile(_) => FailureKind::Unsupported,
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

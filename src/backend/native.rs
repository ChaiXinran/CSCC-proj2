use std::{
    collections::{VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use crate::{
    backend::{BackendExecution, RuntimeBackend},
    builtins,
    contracts::{Chunk, NativeContext, NativeError, NativePipeline, Program, VmErrorKind},
    engine::{EvalFailure, ExecutionOptions, FailureKind, RuntimeConfig},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeScriptCacheKey {
    source_hash: u64,
    strict: bool,
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
            let program = self.pipeline.parse(source)?;
            return self.pipeline.compile(&program);
        }

        let key = NativeScriptCacheKey {
            source_hash: hash_source(source),
            strict: self.context.strict(),
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
        let program = self.pipeline.parse(source)?;
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
}

impl RuntimeBackend for NativeRuntime {
    fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure> {
        self.context.clear_output();
        self.context.set_strict(options.strict);
        let value = self.evaluate(source)?;
        Ok(BackendExecution {
            value: value.to_string(),
            output: self.context.take_output(),
        })
    }

    fn parse_only(&mut self, source: &str, options: ExecutionOptions) -> Result<(), EvalFailure> {
        self.context.set_strict(options.strict);
        let _ = self.prepare_chunk(source).map_err(classify_native_error)?;
        Ok(())
    }

    fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure> {
        self.evaluate(source).map(|_| ())
    }

    fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        Ok(())
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

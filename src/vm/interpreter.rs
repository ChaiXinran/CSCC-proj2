//! Bytecode interpreter.

use std::{collections::HashMap, fmt, fs, path::Path};

use crate::{
    ast::{ModuleDeclaration, Statement},
    builtins::{proxy, string},
    bytecode::{
        Chunk, Compiler, Constant, EnvironmentCapturePolicy, ExceptionHandler, HandlerKind,
        Instruction,
    },
    lexer::Lexer,
    parser::Parser,
    runtime::{
        EnvironmentId, FunctionId, GeneratorRecord, GeneratorState, IteratorKind, IteratorRecord,
        Job, JsFunction, JsObject, JsValue, NativeContext, NativeErrorKind, ObjectId, ObjectKind,
        PreferredType, PrimitiveValue, PrivateBrandId, PromiseCallbackJob, PromiseReaction,
        PropertyDescriptor, PropertyDescriptorUpdate, PropertyKey, PropertyKind, SymbolId,
        TypedArrayViewId, array_index, bigint, to_property_key,
    },
    vm::{CallFrame, Completion},
};

#[derive(Debug)]
struct LoadedModule {
    id: crate::runtime::ModuleId,
    program: crate::ast::Program,
    chunk: Chunk,
    imports: Vec<crate::runtime::ModuleImportBinding>,
    exports: Vec<crate::runtime::ModuleExportBinding>,
    dependencies: Vec<String>,
}

fn load_module_graph(
    context: &mut NativeContext,
    path: &Path,
    graph: &mut HashMap<std::path::PathBuf, LoadedModule>,
) -> Result<(), VmError> {
    let path = crate::runtime::normalize_module_path(path);
    if graph.contains_key(&path) {
        return Ok(());
    }
    let source = fs::read_to_string(&path).map_err(|error| {
        VmError::reference(format!("cannot load module `{}`: {error}", path.display()))
    })?;
    let tokens = Lexer::new(&source)
        .tokenize()
        .map_err(|error| VmError::syntax_error(error.to_string()))?;
    let mut program = Parser::with_source(tokens, &source)
        .parse_module()
        .map_err(|error| VmError::syntax_error(error.to_string()))?;
    normalize_default_export_bindings(&mut program);
    let chunk = Compiler::new()
        .compile_program(&program)
        .map_err(|error| VmError::runtime(error.to_string()))?;

    let mut dependencies = Vec::new();
    let mut import_metadata = Vec::new();
    let mut export_metadata = Vec::new();
    for item in &program.body {
        match item {
            Statement::ModuleDeclaration(ModuleDeclaration::Import(declaration)) => {
                dependencies.push(declaration.source.clone());
                import_metadata.extend(declaration.entries.iter().map(|entry| {
                    crate::runtime::ModuleImportBinding {
                        source: declaration.source.clone(),
                        imported_name: entry.imported_name.clone(),
                        local_name: entry.local_name.clone(),
                    }
                }));
            }
            Statement::ModuleDeclaration(ModuleDeclaration::Export(declaration)) => {
                if let Some(source) = &declaration.source {
                    dependencies.push(source.clone());
                }
                export_metadata.extend(declaration.entries.iter().map(|entry| {
                    crate::runtime::ModuleExportBinding {
                        export_name: entry.export_name.clone(),
                        local_name: entry.local_name.clone(),
                        source: declaration.source.clone(),
                    }
                }));
                if let Some(Statement::VariableDeclaration { declarations, .. }) =
                    declaration.declaration.as_deref()
                {
                    export_metadata.extend(declarations.iter().map(|decl| {
                        crate::runtime::ModuleExportBinding {
                            export_name: decl.name.clone(),
                            local_name: Some(decl.name.clone()),
                            source: None,
                        }
                    }));
                }
            }
            _ => {}
        }
    }
    let id = context.module_registry_mut().ensure_record(&path);
    context.module_registry_mut().set_metadata(
        id,
        dependencies.clone(),
        import_metadata.clone(),
        export_metadata.clone(),
    );
    graph.insert(
        path.clone(),
        LoadedModule {
            id,
            program,
            chunk,
            imports: import_metadata,
            exports: export_metadata,
            dependencies: dependencies.clone(),
        },
    );
    for specifier in dependencies {
        let dependency_path = crate::runtime::resolve_module_specifier(&path, &specifier)
            .map_err(VmError::type_error)?;
        load_module_graph(context, &dependency_path, graph)?;
    }
    Ok(())
}

fn normalize_default_export_bindings(program: &mut crate::ast::Program) {
    for statement in &mut program.body {
        let Statement::ModuleDeclaration(ModuleDeclaration::Export(declaration)) = statement else {
            continue;
        };
        if !declaration
            .entries
            .iter()
            .any(|entry| entry.export_name == "default" && entry.local_name.is_none())
        {
            continue;
        }
        let Some(Statement::Expression(expression)) = declaration.declaration.as_deref() else {
            continue;
        };
        let binding_name = "\0module_default".to_string();
        declaration.declaration = Some(Box::new(Statement::VariableDeclaration {
            kind: crate::ast::VariableKind::Const,
            declarations: vec![crate::ast::VariableDeclarator {
                name: binding_name.clone(),
                pattern: None,
                initializer: Some(expression.clone()),
            }],
        }));
        for entry in &mut declaration.entries {
            if entry.export_name == "default" && entry.local_name.is_none() {
                entry.local_name = Some(binding_name.clone());
            }
        }
    }
}

fn instantiate_module_graph(
    context: &mut NativeContext,
    graph: &HashMap<std::path::PathBuf, LoadedModule>,
) -> Result<(), VmError> {
    for module in graph.values() {
        context
            .module_registry_mut()
            .set_status(module.id, crate::runtime::ModuleStatus::Linking);
        if context.module_registry().environment(module.id).is_none() {
            let depth = context.environment_depth();
            let environment = context.push_environment(Some(context.global_environment()))?;
            context.restore_environment_depth(depth)?;
            context
                .module_registry_mut()
                .set_environment(module.id, environment);
        }
        if context.module_registry().namespace(module.id).is_none() {
            let namespace = context.ordinary_object_with_prototype(None)?;
            let namespace_object = context.require_object(&namespace, "create module namespace")?;
            context.define_symbol_own_property(
                namespace_object,
                context.well_known_symbols().to_string_tag,
                PropertyDescriptor::data_with(
                    JsValue::String("Module".into()),
                    false,
                    false,
                    false,
                ),
            )?;
            context
                .module_registry_mut()
                .set_namespace(module.id, namespace);
        }
        context
            .module_registry_mut()
            .set_status(module.id, crate::runtime::ModuleStatus::Linked);
    }
    // Declaration instantiation is graph-wide and precedes evaluation. This
    // creates TDZ cells before following a cycle, so a back-edge observes an
    // uninitialized binding instead of an absent global.
    for module in graph.values() {
        let environment = context
            .module_registry()
            .environment(module.id)
            .ok_or_else(|| VmError::runtime("module environment is missing"))?;
        instantiate_module_declarations(context, environment, &module.program.body)?;
    }
    // Imports are indirect cells, not copied values. Wire them only after all
    // module environments and export cells exist.
    for module in graph.values() {
        let environment = context
            .module_registry()
            .environment(module.id)
            .ok_or_else(|| VmError::runtime("module environment is missing"))?;
        for binding in &module.imports {
            let dependency_path = crate::runtime::resolve_module_specifier(
                &module_path_for(graph, module.id)?,
                &binding.source,
            )
            .map_err(VmError::type_error)?;
            let dependency = graph
                .get(&dependency_path)
                .ok_or_else(|| VmError::reference("missing linked dependency"))?;
            if binding.imported_name == "*" {
                let namespace = context
                    .module_registry()
                    .namespace(dependency.id)
                    .ok_or_else(|| VmError::runtime("module namespace is missing"))?;
                context.initialize_binding(environment, &binding.local_name, namespace)?;
            } else {
                let (target_environment, target_name) = resolve_export_target(
                    context,
                    graph,
                    &dependency_path,
                    &binding.imported_name,
                )?;
                context.create_module_import_link(
                    environment,
                    binding.local_name.clone(),
                    target_environment,
                    target_name,
                );
            }
        }
    }
    Ok(())
}

fn module_path_for(
    graph: &HashMap<std::path::PathBuf, LoadedModule>,
    id: crate::runtime::ModuleId,
) -> Result<std::path::PathBuf, VmError> {
    graph
        .iter()
        .find_map(|(path, module)| (module.id == id).then(|| path.clone()))
        .ok_or_else(|| VmError::runtime("module path is missing"))
}

fn instantiate_module_declarations(
    context: &mut NativeContext,
    environment: EnvironmentId,
    statements: &[Statement],
) -> Result<(), VmError> {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration { kind, declarations } => {
                for declaration in declarations {
                    if declaration.pattern.is_none() {
                        match kind {
                            crate::ast::VariableKind::Const => context.create_immutable_binding(
                                environment,
                                declaration.name.clone(),
                                true,
                            )?,
                            crate::ast::VariableKind::Let => context.create_mutable_binding(
                                environment,
                                declaration.name.clone(),
                                false,
                                true,
                            )?,
                            crate::ast::VariableKind::Var => context.declare_binding(
                                environment,
                                declaration.name.clone(),
                                JsValue::Undefined,
                                true,
                                false,
                            )?,
                        }
                    }
                }
            }
            Statement::ClassDeclaration(declaration) => context.create_mutable_binding(
                environment,
                declaration.name.clone(),
                false,
                true,
            )?,
            Statement::FunctionDeclaration { name, .. } => context.declare_binding(
                environment,
                name.clone(),
                JsValue::Undefined,
                true,
                false,
            )?,
            Statement::ModuleDeclaration(ModuleDeclaration::Import(declaration)) => {
                for entry in &declaration.entries {
                    context.create_immutable_binding(
                        environment,
                        entry.local_name.clone(),
                        true,
                    )?;
                }
            }
            Statement::ModuleDeclaration(ModuleDeclaration::Export(declaration)) => {
                if let Some(statement) = declaration.declaration.as_deref() {
                    instantiate_module_declarations(
                        context,
                        environment,
                        std::slice::from_ref(statement),
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn resolve_export_target(
    context: &NativeContext,
    graph: &HashMap<std::path::PathBuf, LoadedModule>,
    module_path: &Path,
    export_name: &str,
) -> Result<(EnvironmentId, String), VmError> {
    let mut visited = std::collections::HashSet::new();
    resolve_export_target_inner(context, graph, module_path, export_name, &mut visited)
}

fn resolve_export_target_inner(
    context: &NativeContext,
    graph: &HashMap<std::path::PathBuf, LoadedModule>,
    module_path: &Path,
    export_name: &str,
    visited: &mut std::collections::HashSet<(std::path::PathBuf, String)>,
) -> Result<(EnvironmentId, String), VmError> {
    let module_path = crate::runtime::normalize_module_path(module_path);
    if !visited.insert((module_path.clone(), export_name.to_string())) {
        return Err(VmError::syntax_error(format!(
            "circular re-export of {export_name}"
        )));
    }
    let module = graph
        .get(&module_path)
        .ok_or_else(|| VmError::reference("missing linked module"))?;
    let binding = module
        .exports
        .iter()
        .find(|binding| binding.export_name == export_name)
        .ok_or_else(|| {
            VmError::syntax_error(format!("module has no export named {export_name}"))
        })?;
    if let Some(source) = &binding.source {
        let dependency_path = crate::runtime::resolve_module_specifier(&module_path, source)
            .map_err(VmError::type_error)?;
        return resolve_export_target_inner(context, graph, &dependency_path, export_name, visited);
    }
    let environment = context
        .module_registry()
        .environment(module.id)
        .ok_or_else(|| VmError::runtime("module was not instantiated"))?;
    Ok((
        environment,
        binding
            .local_name
            .clone()
            .unwrap_or_else(|| export_name.to_string()),
    ))
}

fn evaluate_module_graph(
    vm: &mut Vm,
    context: &mut NativeContext,
    graph: &HashMap<std::path::PathBuf, LoadedModule>,
    path: &Path,
) -> Result<JsValue, VmError> {
    let path = crate::runtime::normalize_module_path(path);
    let module = graph
        .get(&path)
        .ok_or_else(|| VmError::reference("missing loaded module"))?;
    match context.module_registry().status_for_path(&path) {
        Some(crate::runtime::ModuleStatus::Evaluated)
        | Some(crate::runtime::ModuleStatus::Evaluating) => {
            return context
                .module_registry()
                .namespace(module.id)
                .ok_or_else(|| VmError::runtime("module namespace is missing"));
        }
        Some(crate::runtime::ModuleStatus::Failed) => {
            return Err(VmError::runtime("module evaluation previously failed"));
        }
        _ => {}
    }
    context
        .module_registry_mut()
        .set_status(module.id, crate::runtime::ModuleStatus::Evaluating);
    context
        .module_registry_mut()
        .set_evaluation_state(module.id, crate::runtime::ModuleEvaluationState::Pending);

    for specifier in &module.dependencies {
        let dependency_path = crate::runtime::resolve_module_specifier(&path, specifier)
            .map_err(VmError::type_error)?;
        evaluate_module_graph(vm, context, graph, &dependency_path)?;
    }

    let environment = context
        .module_registry()
        .environment(module.id)
        .ok_or_else(|| VmError::runtime("module environment is missing"))?;
    let depth = context.environment_depth();
    context.push_existing_environment(environment)?;
    let execution = vm.eval_execute(&module.chunk, context);
    context.restore_environment_depth(depth)?;
    if let Err(error) = execution {
        let rejection = vm
            .pending_exception
            .clone()
            .unwrap_or_else(|| vm_error_to_value(error.clone()));
        context
            .module_registry_mut()
            .set_status(module.id, crate::runtime::ModuleStatus::Failed);
        context.module_registry_mut().set_evaluation_state(
            module.id,
            crate::runtime::ModuleEvaluationState::Rejected(rejection),
        );
        return Err(error);
    }

    let namespace = context
        .module_registry()
        .namespace(module.id)
        .ok_or_else(|| VmError::runtime("module namespace is missing"))?;
    let namespace_object = context.require_object(&namespace, "populate module namespace")?;
    for binding in &module.exports {
        if binding.source.is_some() {
            continue;
        }
        let local_name = binding
            .local_name
            .as_deref()
            .unwrap_or(&binding.export_name);
        if let Ok(value) = context.binding_value_in_environment(environment, local_name) {
            context.define_own_property(
                namespace_object,
                binding.export_name.clone(),
                PropertyDescriptor::data_with(value, true, true, false),
            )?;
        }
    }
    context.prevent_extensions(namespace_object)?;
    context
        .module_registry_mut()
        .set_status(module.id, crate::runtime::ModuleStatus::Evaluated);
    context
        .module_registry_mut()
        .set_evaluation_state(module.id, crate::runtime::ModuleEvaluationState::Fulfilled);
    Ok(namespace)
}

fn evaluate_local_module(
    vm: &mut Vm,
    context: &mut NativeContext,
    path: &Path,
) -> Result<JsValue, VmError> {
    let path = crate::runtime::normalize_module_path(path);
    if matches!(
        context.module_registry().status_for_path(&path),
        Some(crate::runtime::ModuleStatus::Evaluated | crate::runtime::ModuleStatus::Evaluating)
    ) {
        let id = context.module_registry_mut().ensure_record(&path);
        return context
            .module_registry()
            .namespace(id)
            .ok_or_else(|| VmError::runtime("module namespace is missing"));
    }
    let mut graph = HashMap::new();
    load_module_graph(context, &path, &mut graph)?;
    instantiate_module_graph(context, &graph)?;
    evaluate_module_graph(vm, context, &graph, &path)
}

/// B's local dynamic-module loader. Each module owns a persistent lexical
/// environment and namespace instead of projecting bindings onto the global.
fn load_dynamic_module_namespace(
    vm: &mut Vm,
    context: &mut NativeContext,
    specifier: &str,
) -> Result<JsValue, VmError> {
    let referrer = context
        .get_global("__agentjs_dynamic_import_referrer")
        .and_then(|value| value.to_js_string())
        .ok_or_else(|| VmError::reference("dynamic import has no referrer path"))?;
    let path = crate::runtime::resolve_module_specifier(Path::new(&referrer), specifier)
        .map_err(VmError::type_error)?;
    evaluate_local_module(vm, context, &path)
}

const ITERATOR_MAX_ARRAY_LENGTH: usize = 1_000_000;

fn define_arguments_iterator(
    context: &mut NativeContext,
    arguments_id: ObjectId,
) -> Result<(), VmError> {
    let Some(intrinsics) = context.intrinsics().cloned() else {
        return Ok(());
    };
    let iterator = context.well_known_symbols().iterator;
    if let Some(descriptor) =
        context.get_own_symbol_property_descriptor(intrinsics.array_prototype, iterator)
    {
        context.define_symbol_own_property(arguments_id, iterator, descriptor)?;
    }
    Ok(())
}

/// Native VM failure category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmErrorKind {
    Reference,
    Type,
    Syntax,
    Range,
    Test262,
    RuntimeLimit,
    Runtime,
}

/// Native VM failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub kind: VmErrorKind,
    pub message: String,
}

impl VmError {
    #[must_use]
    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::Runtime,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn reference(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::Reference,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn type_error(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::Type,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn syntax_error(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::Syntax,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn range(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::Range,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn test262(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::Test262,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn runtime_limit(message: impl Into<String>) -> Self {
        Self {
            kind: VmErrorKind::RuntimeLimit,
            message: message.into(),
        }
    }
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for VmError {}

/// Stack-based AgentJS interpreter.
#[derive(Debug, Default)]
pub struct Vm {
    stack: Vec<JsValue>,
    pending_exception: Option<JsValue>,
    finally_stack: Vec<Completion>,
}

#[derive(Debug, Clone, PartialEq)]
enum OperationResult {
    Value(JsValue),
    Throw(JsValue),
}

#[derive(Debug, Clone, PartialEq)]
enum IteratorStepResult {
    Value { value: JsValue, done: bool },
    Throw(JsValue),
}

enum YieldStarStepResult {
    Yield(JsValue),
    Complete(JsValue),
    Throw(JsValue),
}

#[derive(Debug, Clone, Copy)]
struct RunBaseline {
    stack_depth: usize,
    environment_depth: usize,
}

impl Vm {
    pub fn execute(&mut self, chunk: &Chunk) -> Result<JsValue, VmError> {
        self.execute_with_context(chunk, &mut NativeContext::default())
    }

    pub(crate) fn operand_stack_roots(&self) -> Vec<JsValue> {
        self.stack.clone()
    }

    pub(crate) fn pending_exception_root(&self) -> Option<JsValue> {
        self.pending_exception.clone()
    }

    pub(crate) fn take_pending_exception_from_builtin(&mut self) -> Option<JsValue> {
        self.pending_exception.take()
    }

    pub(crate) fn throw_value_from_builtin(&mut self, value: JsValue) -> VmError {
        self.pending_exception = Some(value);
        VmError::runtime("JavaScript callback threw")
    }

    pub(crate) fn with_root_from_builtin<T>(
        &mut self,
        value: JsValue,
        operation: impl FnOnce(&mut Self) -> Result<T, VmError>,
    ) -> Result<T, VmError> {
        let root_depth = self.stack.len();
        self.stack.push(value);
        let result = operation(self);
        self.stack.truncate(root_depth);
        result
    }

    pub fn execute_with_context(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        chunk
            .cache_metadata()
            .map_err(|error| VmError::runtime(format!("invalid bytecode chunk: {error}")))?;
        self.stack.clear();
        self.pending_exception = None;
        self.finally_stack.clear();
        chunk
            .validate()
            .map_err(|error| VmError::runtime(format!("invalid bytecode chunk: {error}")))?;
        let analysis = chunk
            .analyze_stack()
            .map_err(|error| VmError::runtime(format!("invalid bytecode stack: {error}")))?;
        context.check_stack_depth(analysis.max_depth)?;
        self.stack.reserve(analysis.max_depth);
        let result = self.run_completion(chunk, context);
        if result.is_err() {
            self.stack.clear();
            self.pending_exception = None;
            self.finally_stack.clear();
        }
        match result? {
            Completion::Normal(value) | Completion::Return(value) => Ok(value),
            Completion::Yield { .. } | Completion::YieldDelegate { .. } => Err(VmError::runtime(
                "yield completion escaped outside a generator",
            )),
            Completion::Throw(value) => {
                self.stack.clear();
                self.pending_exception = None;
                self.finally_stack.clear();
                Err(throw_value(value, context))
            }
            Completion::Break(label) => {
                self.stack.clear();
                Err(VmError::runtime(format!(
                    "unhandled break completion{}",
                    label_suffix(label.as_deref())
                )))
            }
            Completion::Continue(label) => {
                self.stack.clear();
                Err(VmError::runtime(format!(
                    "unhandled continue completion{}",
                    label_suffix(label.as_deref())
                )))
            }
        }
    }

    /// Execute a chunk nested inside an already-running VM (i.e. from `eval()`).
    ///
    /// Unlike `execute_with_context`, this does NOT clear the operand stack —
    /// the eval'd code runs on top of the current stack and the stack is
    /// restored to its pre-eval depth when execution finishes.
    pub(crate) fn eval_execute(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        chunk
            .cache_metadata()
            .map_err(|e| VmError::runtime(format!("invalid bytecode chunk: {e}")))?;
        chunk
            .validate()
            .map_err(|e| VmError::runtime(format!("invalid bytecode chunk: {e}")))?;

        let saved_depth = self.stack.len();
        let result = self.run_completion(chunk, context);
        // Restore stack to pre-eval depth regardless of how execution ended.
        // The return value travels via Result, not via the operand stack.
        self.stack.truncate(saved_depth);

        match result {
            // EvalDeclarationInstantiation §18.2.1.1: duplicate bindings
            // detected during eval execution are SyntaxErrors, not TypeErrors.
            Err(ref error)
                if error.message.starts_with("duplicate binding")
                    && error.kind == VmErrorKind::Type =>
            {
                Err(VmError::syntax_error(error.message.clone()))
            }
            other => match other? {
                Completion::Normal(value) | Completion::Return(value) => Ok(value),
                Completion::Yield { .. } | Completion::YieldDelegate { .. } => Err(
                    VmError::runtime("yield completion escaped outside a generator"),
                ),
                Completion::Throw(value) => {
                    self.pending_exception = Some(value);
                    Err(VmError::runtime("nested JavaScript execution threw"))
                }
                Completion::Break(label) => Err(VmError::runtime(format!(
                    "break completion in eval context{}",
                    label_suffix(label.as_deref())
                ))),
                Completion::Continue(label) => Err(VmError::runtime(format!(
                    "continue completion in eval context{}",
                    label_suffix(label.as_deref())
                ))),
            },
        }
    }

    fn run_completion(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<Completion, VmError> {
        self.run_completion_from(chunk, context, 0)
    }

    fn run_completion_from(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
        start_ip: usize,
    ) -> Result<Completion, VmError> {
        self.run_completion_from_with_initial(chunk, context, start_ip, None, None)
    }

    fn run_completion_from_with_initial(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
        start_ip: usize,
        initial: Option<Completion>,
        baseline_environment_depth: Option<usize>,
    ) -> Result<Completion, VmError> {
        self.run_completion_from_with_initial_until(
            chunk,
            context,
            start_ip,
            initial,
            None,
            baseline_environment_depth,
        )
    }

    fn run_completion_until(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
        start_ip: usize,
        stop_ip: usize,
    ) -> Result<Completion, VmError> {
        self.run_completion_from_with_initial_until(
            chunk,
            context,
            start_ip,
            None,
            Some(stop_ip),
            None,
        )
    }

    fn run_completion_from_with_initial_until(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
        start_ip: usize,
        initial: Option<Completion>,
        stop_ip: Option<usize>,
        baseline_environment_depth: Option<usize>,
    ) -> Result<Completion, VmError> {
        let analysis = chunk
            .analyze_stack()
            .map_err(|error| VmError::runtime(format!("invalid bytecode stack: {error}")))?;
        context.check_stack_depth(self.stack.len().saturating_add(analysis.max_depth))?;
        self.stack.reserve(analysis.max_depth);

        let mut instruction_pointer = start_ip;
        let baseline = RunBaseline {
            stack_depth: self.stack.len(),
            environment_depth: baseline_environment_depth
                .unwrap_or_else(|| context.environment_depth()),
        };
        if let Some(completion) = initial
            && !self.enter_handler(
                chunk,
                start_ip.saturating_sub(1),
                completion.clone(),
                baseline,
                context,
                &mut instruction_pointer,
            )?
        {
            return Ok(completion);
        }
        'dispatch: while instruction_pointer < chunk.instructions.len() {
            if let Some(stop_ip) = stop_ip
                && instruction_pointer >= stop_ip
            {
                break;
            }
            context.check_deadline()?;
            if context.should_collect_garbage() {
                context.collect_garbage_for_vm(self)?;
            }
            let current_instruction = instruction_pointer;
            let instruction = chunk.instructions[current_instruction];
            instruction_pointer += 1;
            let mut abrupt = None;
            let mut discard_saved_finally = false;

            match instruction {
                Instruction::Constant(index) => {
                    let constant = self.constant_at(chunk, index, current_instruction)?;
                    self.stack.push(constant_to_value(constant));
                }
                Instruction::Pop => {
                    self.pop_value()?;
                }
                Instruction::DeclareGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = self.pop_value()?;
                    if context
                        .module_registry()
                        .is_module_environment(context.current_environment())
                    {
                        context.declare_binding(
                            context.current_environment(),
                            name,
                            value,
                            true,
                            false,
                        )?;
                    } else if !context.declare_global(name, value.clone()) {
                        context.set_global(name, value);
                    }
                }
                Instruction::LoadGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    if context
                        .module_registry()
                        .is_module_environment(context.current_environment())
                        && let Some((_, value)) = context.resolve_binding_value(name)?
                    {
                        self.stack.push(value);
                        continue;
                    }
                    match context.get_global(name) {
                        Some(value) => self.stack.push(value),
                        None => {
                            // Spec: global environment's object record checks the global object.
                            // Properties set via `this.x = v` are reachable as global identifiers.
                            let global_id = context.global_object();
                            let name_s = name.to_string();
                            if context.get_own_property(global_id, &name_s).is_some() {
                                let global_obj = context.global_this_value();
                                match context.get_property(global_obj, &name_s) {
                                    Ok(value) => self.stack.push(value),
                                    Err(error) => {
                                        abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                        discard_saved_finally = true;
                                    }
                                }
                            } else {
                                let error = VmError::reference(format!(
                                    "{name} is not defined at instruction {current_instruction}"
                                ));
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::StoreGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = self.pop_value()?;
                    let stored = if context
                        .module_registry()
                        .is_module_environment(context.current_environment())
                    {
                        context.set_binding(name, value.clone()).is_ok()
                    } else {
                        context.set_global(name, value.clone())
                    };
                    if !stored {
                        let error = VmError::reference(format!(
                            "{name} is not defined at instruction {current_instruction}"
                        ));
                        abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                        discard_saved_finally = true;
                    } else {
                        self.stack.push(value);
                    }
                }
                Instruction::UnaryPlus => {
                    let value = self.pop_value()?;
                    match value {
                        JsValue::Number(_) => self.stack.push(value),
                        value => match self.to_number(value, context) {
                            Ok(value) => self.stack.push(JsValue::Number(value)),
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::ToString => {
                    let value = self.pop_value()?;
                    match self.to_string_coerce(value, context) {
                        Ok(value) => self.stack.push(JsValue::String(value)),
                        Err(error) => {
                            abrupt = Some(Completion::Throw(self.throw_value_from_error(error)));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::ToNumeric => {
                    let value = self.pop_value()?;
                    match value {
                        JsValue::Number(_) | JsValue::BigInt(_) => self.stack.push(value),
                        other => match self.to_number(other, context) {
                            Ok(n) => self.stack.push(JsValue::Number(n)),
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::Increment => {
                    let value = self.pop_value()?;
                    match value {
                        JsValue::Number(value) => {
                            self.stack.push(JsValue::Number(value + 1.0));
                        }
                        value => match increment_numeric(self, context, value) {
                            Ok(value) => self.stack.push(value),
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::Decrement => {
                    let value = self.pop_value()?;
                    match value {
                        JsValue::Number(value) => {
                            self.stack.push(JsValue::Number(value - 1.0));
                        }
                        value => match decrement_numeric(self, context, value) {
                            Ok(value) => self.stack.push(value),
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::Negate => {
                    let value = self.pop_value()?;
                    match value {
                        JsValue::Number(value) => self.stack.push(JsValue::Number(-value)),
                        JsValue::BigInt(value) => self
                            .stack
                            .push(JsValue::BigInt(bigint::sub(&bigint::from_i64(0), &value))),
                        value => match self.to_number(value, context) {
                            Ok(value) => self.stack.push(JsValue::Number(-value)),
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::LogicalNot => {
                    let value = self.pop_value()?;
                    self.stack.push(JsValue::Boolean(!value.to_boolean()));
                }
                Instruction::Add => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    match (left, right) {
                        (JsValue::Number(left), JsValue::Number(right)) => {
                            self.stack.push(JsValue::Number(left + right));
                        }
                        (left, right) => match add_values(self, context, left, right) {
                            Ok(value) => self.stack.push(value),
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::Subtract => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(left - right));
                    } else if let Err(error) = self.execute_numeric_binary_slow(
                        left,
                        right,
                        context,
                        |left, right| Ok(bigint::sub(left, right)),
                        |left, right| left - right,
                    ) {
                        abrupt = Some(Completion::Throw(self.throw_value_from_error(error)));
                        discard_saved_finally = true;
                    }
                }
                Instruction::Multiply => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(left * right));
                    } else if let Err(error) = self.execute_numeric_binary_slow(
                        left,
                        right,
                        context,
                        |left, right| Ok(bigint::mul(left, right)),
                        |left, right| left * right,
                    ) {
                        abrupt = Some(Completion::Throw(self.throw_value_from_error(error)));
                        discard_saved_finally = true;
                    }
                }
                Instruction::Divide => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(left / right));
                    } else {
                        match self.to_numeric_operands(left, right, context) {
                            Ok((left, right)) => match bigint_divide(left.clone(), right.clone()) {
                                Ok(Some(value)) => self.stack.push(value),
                                Ok(None) => {
                                    let (left, right) = numeric_number_pair(left, right)?;
                                    self.stack.push(JsValue::Number(left / right));
                                }
                                Err(error) => {
                                    abrupt =
                                        Some(Completion::Throw(self.throw_value_from_error(error)));
                                    discard_saved_finally = true;
                                }
                            },
                            Err(error) => {
                                abrupt =
                                    Some(Completion::Throw(self.throw_value_from_error(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::Remainder => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(left % right));
                    } else {
                        match self.to_numeric_operands(left, right, context) {
                            Ok((left, right)) => {
                                match bigint_remainder(left.clone(), right.clone()) {
                                    Ok(Some(value)) => self.stack.push(value),
                                    Ok(None) => {
                                        let (left, right) = numeric_number_pair(left, right)?;
                                        self.stack.push(JsValue::Number(left % right));
                                    }
                                    Err(error) => {
                                        abrupt = Some(Completion::Throw(
                                            self.throw_value_from_error(error),
                                        ));
                                        discard_saved_finally = true;
                                    }
                                }
                            }
                            Err(error) => {
                                abrupt =
                                    Some(Completion::Throw(self.throw_value_from_error(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::Exponentiation => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(left.powf(*right)));
                    } else {
                        match self.to_numeric_operands(left, right, context) {
                            Ok((left, right)) => {
                                match bigint_exponentiation(left.clone(), right.clone()) {
                                    Ok(Some(value)) => self.stack.push(value),
                                    Ok(None) => {
                                        let (left, right) = numeric_number_pair(left, right)?;
                                        self.stack.push(JsValue::Number(left.powf(right)));
                                    }
                                    Err(error) => {
                                        abrupt = Some(Completion::Throw(
                                            self.throw_value_from_error(error),
                                        ));
                                        discard_saved_finally = true;
                                    }
                                }
                            }
                            Err(error) => {
                                abrupt =
                                    Some(Completion::Throw(self.throw_value_from_error(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::BitwiseAnd => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(f64::from(
                            number_to_int32(*left) & number_to_int32(*right),
                        )));
                    } else if let Err(error) = self.execute_numeric_binary_slow(
                        left,
                        right,
                        context,
                        |left, right| Ok(bigint::bitand(left, right)),
                        |left, right| f64::from(number_to_int32(left) & number_to_int32(right)),
                    ) {
                        abrupt = Some(Completion::Throw(self.throw_value_from_error(error)));
                        discard_saved_finally = true;
                    }
                }
                Instruction::BitwiseOr => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(f64::from(
                            number_to_int32(*left) | number_to_int32(*right),
                        )));
                    } else if let Err(error) = self.execute_numeric_binary_slow(
                        left,
                        right,
                        context,
                        |left, right| Ok(bigint::bitor(left, right)),
                        |left, right| f64::from(number_to_int32(left) | number_to_int32(right)),
                    ) {
                        abrupt = Some(Completion::Throw(self.throw_value_from_error(error)));
                        discard_saved_finally = true;
                    }
                }
                Instruction::BitwiseXor => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(f64::from(
                            number_to_int32(*left) ^ number_to_int32(*right),
                        )));
                    } else if let Err(error) = self.execute_numeric_binary_slow(
                        left,
                        right,
                        context,
                        |left, right| Ok(bigint::bitxor(left, right)),
                        |left, right| f64::from(number_to_int32(left) ^ number_to_int32(right)),
                    ) {
                        abrupt = Some(Completion::Throw(self.throw_value_from_error(error)));
                        discard_saved_finally = true;
                    }
                }
                Instruction::BitwiseNot => {
                    let value = self.pop_value()?;
                    match value {
                        JsValue::Number(value) => self
                            .stack
                            .push(JsValue::Number(f64::from(!number_to_int32(value)))),
                        JsValue::BigInt(value) => {
                            self.stack.push(JsValue::BigInt(bigint::bitnot(&value)));
                        }
                        value => match self.to_int32(value, context) {
                            Ok(value) => {
                                self.stack.push(JsValue::Number(f64::from(!value)));
                            }
                            Err(error) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                                discard_saved_finally = true;
                            }
                        },
                    }
                }
                Instruction::LeftShift => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(f64::from(
                            number_to_int32(*left) << (number_to_uint32(*right) & 0x1f),
                        )));
                    } else {
                        match self.to_numeric_operands(left, right, context) {
                            Ok((left, right)) => {
                                match bigint_shift(left.clone(), right.clone(), false) {
                                    Ok(Some(value)) => self.stack.push(value),
                                    Ok(None) => {
                                        let (left, right) = numeric_number_pair(left, right)?;
                                        self.stack.push(JsValue::Number(f64::from(
                                            number_to_int32(left)
                                                << (number_to_uint32(right) & 0x1f),
                                        )));
                                    }
                                    Err(error) => {
                                        abrupt = Some(Completion::Throw(
                                            self.throw_value_from_error(error),
                                        ));
                                        discard_saved_finally = true;
                                    }
                                }
                            }
                            Err(error) => {
                                abrupt =
                                    Some(Completion::Throw(self.throw_value_from_error(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::RightShift => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(f64::from(
                            number_to_int32(*left) >> (number_to_uint32(*right) & 0x1f),
                        )));
                    } else {
                        match self.to_numeric_operands(left, right, context) {
                            Ok((left, right)) => {
                                match bigint_shift(left.clone(), right.clone(), true) {
                                    Ok(Some(value)) => self.stack.push(value),
                                    Ok(None) => {
                                        let (left, right) = numeric_number_pair(left, right)?;
                                        self.stack.push(JsValue::Number(f64::from(
                                            number_to_int32(left)
                                                >> (number_to_uint32(right) & 0x1f),
                                        )));
                                    }
                                    Err(error) => {
                                        abrupt = Some(Completion::Throw(
                                            self.throw_value_from_error(error),
                                        ));
                                        discard_saved_finally = true;
                                    }
                                }
                            }
                            Err(error) => {
                                abrupt =
                                    Some(Completion::Throw(self.throw_value_from_error(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::UnsignedRightShift => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    if let (JsValue::Number(left), JsValue::Number(right)) = (&left, &right) {
                        self.stack.push(JsValue::Number(f64::from(
                            number_to_uint32(*left) >> (number_to_uint32(*right) & 0x1f),
                        )));
                    } else {
                        match self.to_numeric_operands(left, right, context) {
                            Ok((left, right)) => {
                                if matches!(left, JsValue::BigInt(_))
                                    || matches!(right, JsValue::BigInt(_))
                                {
                                    abrupt = Some(Completion::Throw(vm_error_to_value(
                                        VmError::type_error(
                                            "BigInt does not support unsigned right shift",
                                        ),
                                    )));
                                    discard_saved_finally = true;
                                } else {
                                    let (left, right) = numeric_number_pair(left, right)?;
                                    self.stack.push(JsValue::Number(f64::from(
                                        number_to_uint32(left) >> (number_to_uint32(right) & 0x1f),
                                    )));
                                }
                            }
                            Err(error) => {
                                abrupt =
                                    Some(Completion::Throw(self.throw_value_from_error(error)));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::Equal => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let result = self.abstract_equals(left, right, context)?;
                    self.stack.push(JsValue::Boolean(result));
                }
                Instruction::NotEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let result = self.abstract_equals(left, right, context)?;
                    self.stack.push(JsValue::Boolean(!result));
                }
                Instruction::StrictEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    self.stack
                        .push(JsValue::Boolean(left.strict_equals(&right)));
                }
                Instruction::StrictNotEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    self.stack
                        .push(JsValue::Boolean(!left.strict_equals(&right)));
                }
                Instruction::LessThan => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value = match (left, right) {
                        (JsValue::Number(left), JsValue::Number(right)) => left < right,
                        (left, right) => {
                            compare_values(self, context, left, right, |ordering| ordering.is_lt())?
                        }
                    };
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::LessThanOrEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value = match (left, right) {
                        (JsValue::Number(left), JsValue::Number(right)) => left <= right,
                        (left, right) => {
                            compare_values(self, context, left, right, |ordering| ordering.is_le())?
                        }
                    };
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::GreaterThan => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value = match (left, right) {
                        (JsValue::Number(left), JsValue::Number(right)) => left > right,
                        (left, right) => {
                            compare_values(self, context, left, right, |ordering| ordering.is_gt())?
                        }
                    };
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::GreaterThanOrEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value = match (left, right) {
                        (JsValue::Number(left), JsValue::Number(right)) => left >= right,
                        (left, right) => {
                            compare_values(self, context, left, right, |ordering| ordering.is_ge())?
                        }
                    };
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::JumpIfFalse(target) => {
                    self.validate_jump_target(target, chunk, current_instruction)?;
                    if !self.peek_value()?.to_boolean() {
                        self.jump_to(
                            target,
                            current_instruction,
                            context,
                            &mut instruction_pointer,
                        )?;
                    }
                }
                Instruction::JumpIfTrue(target) => {
                    self.validate_jump_target(target, chunk, current_instruction)?;
                    if self.peek_value()?.to_boolean() {
                        self.jump_to(
                            target,
                            current_instruction,
                            context,
                            &mut instruction_pointer,
                        )?;
                    }
                }
                Instruction::JumpIfNotNullish(target) => {
                    self.validate_jump_target(target, chunk, current_instruction)?;
                    let top = self.peek_value()?;
                    if !matches!(top, JsValue::Null | JsValue::Undefined) {
                        self.jump_to(
                            target,
                            current_instruction,
                            context,
                            &mut instruction_pointer,
                        )?;
                    }
                }
                Instruction::JumpIfNotUndefined(target) => {
                    self.validate_jump_target(target, chunk, current_instruction)?;
                    let top = self.peek_value()?;
                    if !matches!(top, JsValue::Undefined) {
                        self.jump_to(
                            target,
                            current_instruction,
                            context,
                            &mut instruction_pointer,
                        )?;
                    }
                }
                Instruction::Jump(target) => {
                    self.validate_jump_target(target, chunk, current_instruction)?;
                    self.jump_to(
                        target,
                        current_instruction,
                        context,
                        &mut instruction_pointer,
                    )?;
                }
                Instruction::GetProperty(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let object = self.pop_value()?;
                    match self.get_property_value_completion(object, name, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::GetClassHeritagePrototype => {
                    let heritage = self.pop_value()?;
                    if matches!(heritage, JsValue::Null) {
                        self.stack.push(JsValue::Null);
                    } else if !context.is_constructable_value(&heritage) {
                        abrupt = Some(Completion::Throw(vm_error_to_value(VmError::type_error(
                            "class extends value is not a constructor or null",
                        ))));
                        discard_saved_finally = true;
                    } else {
                        match self.get_property_value_completion(heritage, "prototype", context)? {
                            OperationResult::Value(value)
                                if matches!(value, JsValue::Null)
                                    || context.value_object(&value).is_some() =>
                            {
                                self.stack.push(value)
                            }
                            OperationResult::Value(_) => {
                                abrupt = Some(Completion::Throw(vm_error_to_value(
                                    VmError::type_error(
                                        "superclass prototype must be an object or null",
                                    ),
                                )));
                                discard_saved_finally = true;
                            }
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::Call(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let callee = self.pop_value()?;
                    match self.call_value(callee, JsValue::Undefined, arguments, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::Construct(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let callee = self.pop_value()?;
                    match self.construct_value(callee, arguments, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::TypeOf => {
                    let value = self.pop_value()?;
                    self.stack
                        .push(JsValue::String(self.type_of_value(&value, context).into()));
                }
                Instruction::TypeOfGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let type_name = context
                        .get_global(name)
                        .map_or("undefined", |value| self.type_of_value(&value, context));
                    self.stack.push(JsValue::String(type_name.into()));
                }
                Instruction::Throw => {
                    let value = self.pop_value()?;
                    abrupt = Some(Completion::Throw(value));
                    discard_saved_finally = true;
                }
                Instruction::ThrowReferenceError => {
                    abrupt = Some(Completion::Throw(vm_error_to_value(VmError::reference(
                        "invalid delete of super property",
                    ))));
                    discard_saved_finally = true;
                }
                Instruction::Return => {
                    abrupt = Some(Completion::Return(self.pop_value()?));
                    discard_saved_finally = true;
                }
                Instruction::ReturnUndefined => {
                    abrupt = Some(Completion::Return(JsValue::Undefined));
                    discard_saved_finally = true;
                }
                Instruction::CreateFunction(function) => {
                    let value = self.create_function(chunk, function, context)?;
                    self.stack.push(value);
                }
                Instruction::DeclareFunction { name, function } => {
                    let name = self
                        .constant_string(chunk, name, current_instruction)?
                        .to_string();
                    let value = self.create_function(chunk, function, context)?;
                    if context.current_environment() == context.global_environment() {
                        // Create the binding and a matching global property.
                        // §15.1.3 / Annex B §B.3.3.2: global function bindings
                        // are non-configurable.
                        context.declare_binding(
                            context.current_environment(),
                            name.clone(),
                            value.clone(),
                            true,
                            false,
                        )?;
                        let _ = context.define_own_property(
                            context.global_object(),
                            name,
                            crate::runtime::PropertyDescriptor::data_with(
                                value, true,  // writable
                                true,  // enumerable
                                false, // configurable
                            ),
                        );
                    } else {
                        context.declare_binding(
                            context.current_environment(),
                            name,
                            value,
                            true,
                            false,
                        )?;
                    }
                }
                Instruction::DeclareLocal(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    context.declare_binding(
                        context.current_environment(),
                        name,
                        value,
                        true,
                        false,
                    )?;
                }
                Instruction::LoadName(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    match context.resolve_binding_value(name) {
                        Ok(Some((_, value))) => self.stack.push(value),
                        Ok(None) => {
                            let error = VmError::reference(format!(
                                "{name} is not defined at instruction {current_instruction}"
                            ));
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                        Err(error) => {
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::TypeOfName(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    match context.resolve_binding_value(name) {
                        Ok(value) => {
                            let type_name = value.map_or("undefined", |(_, value)| {
                                self.type_of_value(&value, context)
                            });
                            self.stack.push(JsValue::String(type_name.into()));
                        }
                        Err(error) => {
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::StoreName(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = self.pop_value()?;
                    match context.set_binding(name, value.clone()) {
                        Ok(()) => self.stack.push(value),
                        Err(error) => {
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::LoadThis => {
                    let this_value = context.current_or_global_this();
                    let is_uninitialized_derived_this = context
                        .current_function()
                        .and_then(|function| context.function(function))
                        .is_some_and(|function| function.is_derived_constructor)
                        && matches!(this_value, JsValue::Undefined);
                    if is_uninitialized_derived_this {
                        abrupt = Some(Completion::Throw(vm_error_to_value(VmError::reference(
                            "must call super constructor before accessing this",
                        ))));
                        discard_saved_finally = true;
                    } else {
                        self.stack.push(this_value);
                    }
                }
                Instruction::LoadNewTarget => {
                    // Returns `undefined` in regular calls. Constructor calls
                    // would set new.target to the constructor, but our VM does
                    // not yet track this — return undefined as a safe default.
                    self.stack.push(context.current_new_target());
                }
                Instruction::ArrayCreate(count) => {
                    let elements = self.pop_arguments(count)?;
                    self.stack.push(context.create_array(elements)?);
                }
                Instruction::ObjectCreate(count) => {
                    let mut properties = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let value = self.pop_value()?;
                        let key = to_property_key(&self.pop_value()?)?;
                        // ObjectCreate only handles string keys (non-computed
                        // property names in object literals). Computed symbol
                        // keys use SetElement at runtime instead.
                        let PropertyKey::String(key) = key else {
                            return Err(VmError::type_error(
                                "symbol keys in object literals are not yet supported",
                            ));
                        };
                        properties.push((key, value));
                    }
                    properties.reverse();
                    self.stack.push(context.create_object(properties)?);
                }
                Instruction::GetElement => {
                    let key = self.pop_value()?;
                    let object = self.pop_value()?;
                    // Fast path: non-negative integer key on Array / TypedArray.
                    'fast: {
                        let JsValue::Number(n) = key else { break 'fast };
                        if n.fract() != 0.0 || n < 0.0 || n >= 4_294_967_295.0 {
                            break 'fast;
                        }
                        let JsValue::Object(obj_id) = object else {
                            break 'fast;
                        };
                        let idx = n as usize;
                        let Some(result) = Self::fast_get_element(obj_id, idx, context) else {
                            break 'fast;
                        };
                        match result {
                            Ok(value) => {
                                self.stack.push(value);
                                continue 'dispatch;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    // Per spec §13.2.3: check for null/undefined BEFORE coercing
                    // the property key. The null/undefined check must happen first
                    // so that TypeError takes precedence over any side effects
                    // from ToPropertyKey (e.g. toString() on key objects).
                    let is_nullish = matches!(object, JsValue::Null | JsValue::Undefined);
                    if is_nullish {
                        let kind = if matches!(object, JsValue::Null) {
                            "null"
                        } else {
                            "undefined"
                        };
                        abrupt = Some(Completion::Throw(vm_error_to_value(
                            VmError::type_error(format!(
                                "Cannot read properties of {kind} (reading property key)"
                            )),
                        )));
                        discard_saved_finally = true;
                    } else if let JsValue::Symbol(sym_id) = &key {
                        match self.get_symbol_property_value_completion(object, *sym_id, context)? {
                            OperationResult::Value(value) => self.stack.push(value),
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                        }
                    } else {
                        match self.coerce_to_property_key(key, context)? {
                            OperationResult::Value(JsValue::String(key_str)) => {
                                match self
                                    .get_property_value_completion(object, &key_str, context)?
                                {
                                    OperationResult::Value(value) => self.stack.push(value),
                                    OperationResult::Throw(value) => {
                                        abrupt = Some(Completion::Throw(value));
                                        discard_saved_finally = true;
                                    }
                                }
                            }
                            OperationResult::Value(JsValue::Symbol(symbol)) => {
                                match self
                                    .get_symbol_property_value_completion(object, symbol, context)?
                                {
                                    OperationResult::Value(value) => self.stack.push(value),
                                    OperationResult::Throw(value) => {
                                        abrupt = Some(Completion::Throw(value));
                                        discard_saved_finally = true;
                                    }
                                }
                            }
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                            _ => unreachable!("ToPropertyKey returns a string or symbol"),
                        }
                    }
                }
                Instruction::CreateRegExp => {
                    let flags = match self.pop_value()? {
                        JsValue::String(s) => s,
                        _ => return Err(VmError::type_error("regexp flags must be a string")),
                    };
                    let pattern = match self.pop_value()? {
                        JsValue::String(s) => s,
                        _ => return Err(VmError::type_error("regexp pattern must be a string")),
                    };
                    let regexp = context.create_regexp(pattern, flags)?;
                    self.stack.push(regexp);
                }

                // V8-A: spread / array-push / rest
                Instruction::ArrayPush => {
                    let value = self.pop_value()?;
                    let array_val = self.peek_value()?.clone();
                    let array_id = context.require_object(&array_val, "ArrayPush")?;
                    let length = self.get_property_value(array_val, "length", context)?;
                    let index = match length {
                        JsValue::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                        _ => 0,
                    };
                    context.define_own_property(
                        array_id,
                        index.to_string(),
                        crate::runtime::PropertyDescriptor::data(value),
                    )?;
                    let new_len = JsValue::Number((index + 1) as f64);
                    context.define_own_property(
                        array_id,
                        "length".to_string(),
                        crate::runtime::PropertyDescriptor::data_with(new_len, true, false, true),
                    )?;
                }
                Instruction::IterableToArray => {
                    let iterable = self.pop_value()?;
                    // Fast path: already an array — return as-is.
                    if let JsValue::Object(id) = &iterable {
                        let is_array = context
                            .heap()
                            .object(*id)
                            .map(|o| matches!(o.kind, crate::runtime::ObjectKind::Array { .. }))
                            .unwrap_or(false);
                        if is_array {
                            self.stack.push(iterable);
                        } else {
                            match self.collect_iterable_spread(iterable, context)? {
                                Ok(elements) => {
                                    let arr = context.create_array(elements)?;
                                    self.stack.push(arr);
                                }
                                Err(throw_val) => {
                                    abrupt = Some(Completion::Throw(throw_val));
                                    discard_saved_finally = true;
                                }
                            }
                        }
                    } else {
                        match self.collect_iterable_spread(iterable, context)? {
                            Ok(elements) => {
                                let arr = context.create_array(elements)?;
                                self.stack.push(arr);
                            }
                            Err(throw_val) => {
                                abrupt = Some(Completion::Throw(throw_val));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::RequireObjectCoercible => {
                    let value = self.peek_value()?;
                    if matches!(value, JsValue::Null | JsValue::Undefined) {
                        abrupt = Some(Completion::Throw(vm_error_to_value(VmError::type_error(
                            "Cannot destructure null or undefined",
                        ))));
                        discard_saved_finally = true;
                    }
                }
                Instruction::SpreadIntoArray => {
                    let iterable = self.pop_value()?;
                    let array_val = self.peek_value()?.clone();
                    let array_id = context.require_object(&array_val, "SpreadIntoArray")?;
                    match self.collect_iterable_spread(iterable, context)? {
                        Ok(elements) => {
                            let start_len = {
                                let len_val = self.get_property_value(
                                    context.object_value(array_id),
                                    "length",
                                    context,
                                )?;
                                match len_val {
                                    JsValue::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                                    _ => 0,
                                }
                            };
                            let n_added = elements.len();
                            for (i, elem) in elements.into_iter().enumerate() {
                                context.define_own_property(
                                    array_id,
                                    (start_len + i).to_string(),
                                    crate::runtime::PropertyDescriptor::data(elem),
                                )?;
                            }
                            context.define_own_property(
                                array_id,
                                "length".to_string(),
                                crate::runtime::PropertyDescriptor::data_with(
                                    JsValue::Number((start_len + n_added) as f64),
                                    true,
                                    false,
                                    true,
                                ),
                            )?;
                        }
                        Err(throw_val) => {
                            abrupt = Some(Completion::Throw(throw_val));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SpreadCall(n_regular) => {
                    let spread_val = self.pop_value()?;
                    let n = n_regular as usize;
                    match self.collect_iterable_spread(spread_val, context)? {
                        Ok(spread_args) => {
                            let regular_args = self.pop_arguments(n_regular)?;
                            let callee = self.pop_value()?;
                            let mut all_args = regular_args;
                            all_args.extend(spread_args);
                            match self.call_value(callee, JsValue::Undefined, all_args, context)? {
                                OperationResult::Value(v) => self.stack.push(v),
                                OperationResult::Throw(v) => {
                                    abrupt = Some(Completion::Throw(v));
                                    discard_saved_finally = true;
                                }
                            }
                        }
                        Err(throw_val) => {
                            // Discard regular args and callee from stack before throwing
                            for _ in 0..n {
                                let _ = self.pop_value()?;
                            }
                            let _ = self.pop_value()?; // callee
                            abrupt = Some(Completion::Throw(throw_val));
                            discard_saved_finally = true;
                        }
                    }
                    let _ = n;
                }
                Instruction::SpreadCallWithThis(n_regular) => {
                    let spread_val = self.pop_value()?;
                    let n = n_regular as usize;
                    match self.collect_iterable_spread(spread_val, context)? {
                        Ok(spread_args) => {
                            let regular_args = self.pop_arguments(n_regular)?;
                            let this_value = self.pop_value()?;
                            let callee = self.pop_value()?;
                            let mut all_args = regular_args;
                            all_args.extend(spread_args);
                            match self.call_value(callee, this_value, all_args, context)? {
                                OperationResult::Value(v) => self.stack.push(v),
                                OperationResult::Throw(v) => {
                                    abrupt = Some(Completion::Throw(v));
                                    discard_saved_finally = true;
                                }
                            }
                        }
                        Err(throw_val) => {
                            for _ in 0..n {
                                let _ = self.pop_value()?;
                            }
                            let _ = self.pop_value()?; // this
                            let _ = self.pop_value()?; // callee
                            abrupt = Some(Completion::Throw(throw_val));
                            discard_saved_finally = true;
                        }
                    }
                    let _ = n;
                }
                Instruction::SpreadConstruct(n_regular) => {
                    let spread_val = self.pop_value()?;
                    let n = n_regular as usize;
                    match self.collect_iterable_spread(spread_val, context)? {
                        Ok(spread_args) => {
                            let regular_args = self.pop_arguments(n_regular)?;
                            let callee = self.pop_value()?;
                            let mut all_args = regular_args;
                            all_args.extend(spread_args);
                            match self.construct_value(callee, all_args, context)? {
                                OperationResult::Value(v) => self.stack.push(v),
                                OperationResult::Throw(v) => {
                                    abrupt = Some(Completion::Throw(v));
                                    discard_saved_finally = true;
                                }
                            }
                        }
                        Err(throw_val) => {
                            for _ in 0..n {
                                let _ = self.pop_value()?;
                            }
                            let _ = self.pop_value()?; // callee
                            abrupt = Some(Completion::Throw(throw_val));
                            discard_saved_finally = true;
                        }
                    }
                    let _ = n;
                }

                // Iterator protocol — wired to IteratorRecord helpers in NativeContext.
                Instruction::GetIterator => {
                    let iterable = self.pop_value()?;
                    match self.create_iterator_object(iterable, context)? {
                        OperationResult::Value(iterator) => self.stack.push(iterator),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::GetAsyncIterator => {
                    let iterable = self.pop_value()?;
                    match self.create_async_iterator_object(iterable, context)? {
                        OperationResult::Value(iterator) => self.stack.push(iterator),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::IteratorNext => {
                    let iterator = self.pop_value()?;
                    match self.step_iterator_object(iterator, context)? {
                        IteratorStepResult::Value { value, done } => {
                            // Push value first, then done-flag on top (JumpIfTrue peeks the top).
                            self.stack.push(value);
                            self.stack.push(JsValue::Boolean(done));
                        }
                        IteratorStepResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::AsyncIteratorNext => {
                    let iterator = self.pop_value()?;
                    match self.step_async_iterator_object(iterator, context)? {
                        IteratorStepResult::Value { value, done } => {
                            self.stack.push(value);
                            self.stack.push(JsValue::Boolean(done));
                        }
                        IteratorStepResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::IteratorClose => {
                    let iterator = self.pop_value()?;
                    let preserve_throw_completion =
                        matches!(self.finally_stack.last(), Some(Completion::Throw(_)));
                    match self.close_iterator_object_completion(iterator, context)? {
                        OperationResult::Value(_) => {}
                        OperationResult::Throw(value) => {
                            if !preserve_throw_completion {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }

                // V9-A stubs: generator support (B group provides full implementation)
                Instruction::CreateGenerator(function) => {
                    let value = self.create_function(chunk, function, context)?;
                    self.stack.push(value);
                }
                Instruction::YieldValue => {
                    let value = self.pop_value()?;
                    return Ok(Completion::Yield {
                        value,
                        next_ip: instruction_pointer,
                    });
                }
                Instruction::YieldDelegate => {
                    let iterable = self.pop_value()?;
                    let is_async_generator = context
                        .current_function()
                        .and_then(|function| context.function(function))
                        .is_some_and(|function| function.is_async && function.is_generator);
                    let iterator_result = if is_async_generator {
                        self.create_async_iterator_object(iterable, context)?
                    } else {
                        self.create_iterator_object(iterable, context)?
                    };
                    let iterator = match iterator_result {
                        OperationResult::Value(iterator) => iterator,
                        OperationResult::Throw(value) => return Ok(Completion::Throw(value)),
                    };
                    let iterator_root_depth = self.stack.len();
                    self.stack.push(iterator.clone());
                    match self.step_yield_star_iterator(
                        iterator.clone(),
                        JsValue::Undefined,
                        context,
                    )? {
                        YieldStarStepResult::Yield(value) => {
                            self.stack.truncate(iterator_root_depth);
                            return Ok(Completion::YieldDelegate {
                                iterator,
                                value,
                                next_ip: instruction_pointer,
                            });
                        }
                        YieldStarStepResult::Complete(value) => {
                            self.stack.truncate(iterator_root_depth);
                            self.stack.push(value);
                        }
                        YieldStarStepResult::Throw(value) => {
                            self.stack.truncate(iterator_root_depth);
                            return Ok(Completion::Throw(value));
                        }
                    }
                }

                Instruction::CreateAsyncFunction(function) => {
                    let value = self.create_function(chunk, function, context)?;
                    self.stack.push(value);
                }
                Instruction::AwaitValue => {
                    let value = self.pop_value()?;
                    match self.await_value_now(value, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }

                Instruction::ForInKeys => {
                    let value = self.pop_value()?;
                    let keys: Vec<String> = match &value {
                        JsValue::Null | JsValue::Undefined => Vec::new(),
                        JsValue::String(text) => {
                            let count = text.encode_utf16().count();
                            context.ensure_heap_capacity(count.saturating_mul(8))?;
                            (0..count).map(|index| index.to_string()).collect()
                        }
                        _ => match context.value_object(&value) {
                            Some(_) => proxy::internal_for_in_keys(self, context, value)?,
                            None => Vec::new(),
                        },
                    };
                    let elements = keys.into_iter().map(JsValue::String).collect();
                    let array = context.create_array(elements)?;
                    self.stack.push(array);
                }
                Instruction::GetMethod(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let object = self.pop_value()?;
                    match self.get_property_value_completion(object.clone(), name, context)? {
                        OperationResult::Value(method) => {
                            self.stack.push(method);
                            self.stack.push(object);
                        }
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::GetSuperMethod(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    match self.get_super_property_value(name, context)? {
                        OperationResult::Value(method) => {
                            self.stack.push(method);
                            self.stack.push(context.current_or_global_this());
                        }
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::GetSuperElementMethod => {
                    let key = self.pop_value()?;
                    let result = if let JsValue::Symbol(sym_id) = key {
                        self.get_super_symbol_property_value(sym_id, context)?
                    } else {
                        match self.coerce_to_property_key(key, context)? {
                            OperationResult::Throw(value) => OperationResult::Throw(value),
                            OperationResult::Value(JsValue::String(key)) => {
                                self.get_super_property_value(&key, context)?
                            }
                            OperationResult::Value(JsValue::Symbol(symbol)) => {
                                self.get_super_symbol_property_value(symbol, context)?
                            }
                            _ => unreachable!("ToPropertyKey returns a string or symbol"),
                        }
                    };
                    match result {
                        OperationResult::Value(method) => {
                            self.stack.push(method);
                            self.stack.push(context.current_or_global_this());
                        }
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::GetElementMethod => {
                    let key = self.pop_value()?;
                    let object = self.pop_value()?;
                    let result = if let JsValue::Symbol(sym_id) = key {
                        self.get_symbol_property_value_completion(object.clone(), sym_id, context)?
                    } else {
                        match self.coerce_to_property_key(key, context)? {
                            OperationResult::Throw(v) => OperationResult::Throw(v),
                            OperationResult::Value(JsValue::String(k)) => {
                                self.get_property_value_completion(object.clone(), &k, context)?
                            }
                            OperationResult::Value(JsValue::Symbol(symbol)) => self
                                .get_symbol_property_value_completion(
                                    object.clone(),
                                    symbol,
                                    context,
                                )?,
                            _ => unreachable!("ToPropertyKey returns a string or symbol"),
                        }
                    };
                    match result {
                        OperationResult::Value(method) => {
                            self.stack.push(method);
                            self.stack.push(object);
                        }
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SetProperty(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    let object = self.pop_value()?;
                    match self.set_property_value(object, &name, value, context)? {
                        OperationResult::Value(result) => self.stack.push(result),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SetElement => {
                    let value = self.pop_value()?;
                    let key = self.pop_value()?;
                    let object = self.pop_value()?;
                    match self.set_element_value(object, key, value, context)? {
                        OperationResult::Value(result) => self.stack.push(result),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SetElementKeepOld => {
                    let value = self.pop_value()?;
                    let old = self.pop_value()?;
                    let key = self.pop_value()?;
                    let object = self.pop_value()?;
                    match self.set_element_value(object, key, value, context)? {
                        OperationResult::Value(_) => self.stack.push(old),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::CallWithThis(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let this_value = self.pop_value()?;
                    let callee = self.pop_value()?;
                    match self.call_value(callee, this_value, arguments, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SuperCall(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let super_constructor = self.pop_value()?;
                    match self.call_super_constructor(super_constructor, arguments, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SuperSpreadCall(regular_count) => {
                    let spread = self.pop_value()?;
                    let regular = self.pop_arguments(regular_count)?;
                    let super_constructor = self.pop_value()?;
                    match self.collect_iterable_spread(spread, context)? {
                        Ok(mut spread) => {
                            let mut arguments = regular;
                            arguments.append(&mut spread);
                            match self.call_super_constructor(
                                super_constructor,
                                arguments,
                                context,
                            )? {
                                OperationResult::Value(value) => self.stack.push(value),
                                OperationResult::Throw(value) => {
                                    abrupt = Some(Completion::Throw(value));
                                    discard_saved_finally = true;
                                }
                            }
                        }
                        Err(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SuperForwardCall => {
                    let rest = self.pop_value()?;
                    let super_constructor = self.pop_value()?;
                    let rest_object =
                        context.require_object(&rest, "forward constructor arguments")?;
                    let length = context
                        .heap()
                        .object(rest_object)
                        .and_then(JsObject::array_length)
                        .ok_or_else(|| {
                            VmError::type_error("default constructor rest is not an array")
                        })?;
                    let mut arguments = Vec::with_capacity(length);
                    for index in 0..length {
                        let value = context.get_property(rest.clone(), &index.to_string())?;
                        arguments.push(value);
                    }
                    match self.call_super_constructor(super_constructor, arguments, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::DynamicImport => {
                    // Dynamic import always returns a Promise. The V13-B local loader
                    // settles it synchronously into the native Promise registry; its
                    // reactions still run through the normal job queue.
                    let _options = self.pop_value()?;
                    let specifier = self.pop_value()?;
                    let promise = context.create_promise()?;
                    let prototype = context
                        .get_global("Promise")
                        .and_then(|value| context.get_property(value, "prototype").ok())
                        .and_then(|value| match value {
                            JsValue::Object(id) => Some(id),
                            _ => None,
                        });
                    let value = context.create_promise_object(promise, prototype)?;
                    match self.to_string_coerce(specifier, context) {
                        Ok(specifier) => {
                            match load_dynamic_module_namespace(self, context, &specifier) {
                                Ok(namespace) => {
                                    context.fulfill_promise(promise, namespace)?;
                                }
                                Err(error) => {
                                    let reason = self
                                        .pending_exception
                                        .take()
                                        .unwrap_or_else(|| vm_error_to_value(error));
                                    context.reject_promise(promise, reason)?;
                                }
                            }
                        }
                        Err(error) => {
                            let reason = self
                                .pending_exception
                                .take()
                                .unwrap_or_else(|| vm_error_to_value(error));
                            context.reject_promise(promise, reason)?;
                        }
                    }
                    self.stack.push(value);
                }
                Instruction::ObjectCreateEmpty => {
                    self.stack.push(context.create_object([])?);
                }
                Instruction::ArrayCreateSparse(length) => {
                    self.stack
                        .push(context.create_sparse_array(length as usize)?);
                }
                Instruction::DefineDataProperty(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    let object = context.require_object(self.peek_value()?, "define property")?;
                    context.set_function_home_object(&value, object)?;
                    context.define_own_property(object, name, PropertyDescriptor::data(value))?;
                }
                Instruction::DefineClassPrototype => {
                    let proto = self.pop_value()?;
                    let ctor =
                        context.require_object(self.peek_value()?, "define class prototype")?;
                    context.define_own_property(
                        ctor,
                        "prototype".into(),
                        PropertyDescriptor::data_with(proto, false, false, false),
                    )?;
                }
                Instruction::DefineGetter(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let getter = self.pop_value()?;
                    let object = context.require_object(self.peek_value()?, "define getter")?;
                    context.set_function_home_object(&getter, object)?;
                    let setter = existing_accessor_setter(context, object, &name);
                    context.define_own_property(
                        object,
                        name,
                        PropertyDescriptor::accessor(Some(getter), setter, true, true),
                    )?;
                }
                Instruction::DefineSetter(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let setter = self.pop_value()?;
                    let object = context.require_object(self.peek_value()?, "define setter")?;
                    context.set_function_home_object(&setter, object)?;
                    let getter = existing_accessor_getter(context, object, &name);
                    context.define_own_property(
                        object,
                        name,
                        PropertyDescriptor::accessor(getter, Some(setter), true, true),
                    )?;
                }
                Instruction::DefineComputedGetter => {
                    let getter = self.pop_value()?;
                    let key = self.pop_value()?;
                    let object_value = self.pop_value()?;
                    let object = context.require_object(&object_value, "define getter")?;
                    self.define_computed_accessor(
                        object,
                        key,
                        Some(getter),
                        None,
                        true,
                        true,
                        context,
                    )?;
                }
                Instruction::DefineComputedSetter => {
                    let setter = self.pop_value()?;
                    let key = self.pop_value()?;
                    let object_value = self.pop_value()?;
                    let object = context.require_object(&object_value, "define setter")?;
                    self.define_computed_accessor(
                        object,
                        key,
                        None,
                        Some(setter),
                        true,
                        true,
                        context,
                    )?;
                }
                Instruction::DefineClassMethod(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    let object =
                        context.require_object(self.peek_value()?, "define class method")?;
                    context.set_function_home_object(&value, object)?;
                    context.define_own_property(
                        object,
                        name,
                        PropertyDescriptor::data_with(value, true, false, true),
                    )?;
                }
                Instruction::SetFunctionHomeObject => {
                    let function = self.peek_value()?.clone();
                    let home = self
                        .stack
                        .get(self.stack.len().saturating_sub(2))
                        .ok_or_else(|| VmError::runtime("missing class initializer home object"))?
                        .clone();
                    let home = context.require_object(&home, "class initializer home object")?;
                    context.set_function_home_object(&function, home)?;
                }
                Instruction::DefineClassGetter(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let getter = self.pop_value()?;
                    let object =
                        context.require_object(self.peek_value()?, "define class getter")?;
                    context.set_function_home_object(&getter, object)?;
                    let setter = existing_accessor_setter(context, object, &name);
                    context.define_own_property(
                        object,
                        name,
                        PropertyDescriptor::accessor(Some(getter), setter, false, true),
                    )?;
                }
                Instruction::DefineClassSetter(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let setter = self.pop_value()?;
                    let object =
                        context.require_object(self.peek_value()?, "define class setter")?;
                    context.set_function_home_object(&setter, object)?;
                    let getter = existing_accessor_getter(context, object, &name);
                    context.define_own_property(
                        object,
                        name,
                        PropertyDescriptor::accessor(getter, Some(setter), false, true),
                    )?;
                }
                Instruction::DefineClassMethodComputed => {
                    let value = self.pop_value()?;
                    let key = self.pop_value()?;
                    match self.coerce_to_property_key(key, context)? {
                        OperationResult::Throw(v) => {
                            abrupt = Some(Completion::Throw(v));
                            discard_saved_finally = true;
                        }
                        OperationResult::Value(JsValue::String(name)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define class method (computed)",
                            )?;
                            // Static method named "prototype" is a TypeError per spec.
                            if name == "prototype"
                                && matches!(self.peek_value()?, JsValue::Function(_))
                            {
                                let err = VmError::type_error(
                                    "Classes may not have a static property named 'prototype'",
                                );
                                abrupt = Some(Completion::Throw(self.throw_value_from_error(err)));
                                discard_saved_finally = true;
                            } else {
                                context.set_function_home_object(&value, object)?;
                                self.maybe_set_function_name_from_key(
                                    &value,
                                    &JsValue::String(name.clone()),
                                    None,
                                    context,
                                );
                                context.define_own_property(
                                    object,
                                    name,
                                    PropertyDescriptor::data_with(value, true, false, true),
                                )?;
                            }
                        }
                        OperationResult::Value(JsValue::Symbol(symbol)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define class method (computed)",
                            )?;
                            context.set_function_home_object(&value, object)?;
                            self.maybe_set_function_name_from_key(
                                &value,
                                &JsValue::Symbol(symbol),
                                None,
                                context,
                            );
                            context.define_symbol_own_property(
                                object,
                                symbol,
                                PropertyDescriptor::data_with(value, true, false, true),
                            )?;
                        }
                        _ => unreachable!("ToPropertyKey returns a string or symbol"),
                    }
                }
                Instruction::DefineClassGetterComputed => {
                    let getter = self.pop_value()?;
                    let key = self.pop_value()?;
                    match self.coerce_to_property_key(key, context)? {
                        OperationResult::Throw(v) => {
                            abrupt = Some(Completion::Throw(v));
                            discard_saved_finally = true;
                        }
                        OperationResult::Value(JsValue::String(name)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define class getter (computed)",
                            )?;
                            // Static getter/setter named "prototype" is a TypeError per spec.
                            if name == "prototype"
                                && matches!(self.peek_value()?, JsValue::Function(_))
                            {
                                let err = VmError::type_error(
                                    "Classes may not have a static property named 'prototype'",
                                );
                                abrupt = Some(Completion::Throw(self.throw_value_from_error(err)));
                                discard_saved_finally = true;
                            } else {
                                context.set_function_home_object(&getter, object)?;
                                let setter = existing_accessor_setter(context, object, &name);
                                context.define_own_property(
                                    object,
                                    name,
                                    PropertyDescriptor::accessor(Some(getter), setter, false, true),
                                )?;
                            }
                        }
                        OperationResult::Value(JsValue::Symbol(symbol)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define class getter (computed)",
                            )?;
                            self.define_computed_accessor(
                                object,
                                JsValue::Symbol(symbol),
                                Some(getter),
                                None,
                                false,
                                true,
                                context,
                            )?;
                        }
                        _ => unreachable!("ToPropertyKey returns a string or symbol"),
                    }
                }
                Instruction::DefineClassSetterComputed => {
                    let setter = self.pop_value()?;
                    let key = self.pop_value()?;
                    match self.coerce_to_property_key(key, context)? {
                        OperationResult::Throw(v) => {
                            abrupt = Some(Completion::Throw(v));
                            discard_saved_finally = true;
                        }
                        OperationResult::Value(JsValue::String(name)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define class setter (computed)",
                            )?;
                            // Static setter named "prototype" is a TypeError per spec.
                            if name == "prototype"
                                && matches!(self.peek_value()?, JsValue::Function(_))
                            {
                                let err = VmError::type_error(
                                    "Classes may not have a static property named 'prototype'",
                                );
                                abrupt = Some(Completion::Throw(self.throw_value_from_error(err)));
                                discard_saved_finally = true;
                            } else {
                                context.set_function_home_object(&setter, object)?;
                                let getter = existing_accessor_getter(context, object, &name);
                                context.define_own_property(
                                    object,
                                    name,
                                    PropertyDescriptor::accessor(getter, Some(setter), false, true),
                                )?;
                            }
                        }
                        OperationResult::Value(JsValue::Symbol(symbol)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define class setter (computed)",
                            )?;
                            self.define_computed_accessor(
                                object,
                                JsValue::Symbol(symbol),
                                None,
                                Some(setter),
                                false,
                                true,
                                context,
                            )?;
                        }
                        _ => unreachable!("ToPropertyKey returns a string or symbol"),
                    }
                }
                Instruction::DefineDataPropertyComputed => {
                    let value = self.pop_value()?;
                    let key = self.pop_value()?;
                    match self.coerce_to_property_key(key, context)? {
                        OperationResult::Throw(v) => {
                            abrupt = Some(Completion::Throw(v));
                            discard_saved_finally = true;
                        }
                        OperationResult::Value(JsValue::String(name)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define data property (computed)",
                            )?;
                            context.set_function_home_object(&value, object)?;
                            context.define_own_property(
                                object,
                                name,
                                PropertyDescriptor::data(value),
                            )?;
                        }
                        OperationResult::Value(JsValue::Symbol(symbol)) => {
                            let object = context.require_object(
                                self.peek_value()?,
                                "define data property (computed)",
                            )?;
                            context.set_function_home_object(&value, object)?;
                            context.define_symbol_own_property(
                                object,
                                symbol,
                                PropertyDescriptor::data(value),
                            )?;
                        }
                        _ => unreachable!("ToPropertyKey returns a string or symbol"),
                    }
                }
                Instruction::ToPropertyKey => {
                    let value = self.pop_value()?;
                    match self.coerce_to_property_key(value, context)? {
                        OperationResult::Value(key) => self.stack.push(key),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::SpreadObject => {
                    let spread_val = self.pop_value()?;
                    let target_val = self.peek_value()?.clone();
                    let target_id = context.require_object(&target_val, "SpreadObject")?;
                    if let JsValue::Object(source_id) = &spread_val {
                        let keys = context.own_enumerable_keys(*source_id);
                        let mut threw: Option<JsValue> = None;
                        for key in keys {
                            // Use the VM call path to handle getter properties.
                            match self.get_property_value_completion(
                                spread_val.clone(),
                                &key,
                                context,
                            )? {
                                OperationResult::Throw(v) => {
                                    threw = Some(v);
                                    break;
                                }
                                OperationResult::Value(value) => {
                                    context.set_property(JsValue::Object(target_id), key, value)?;
                                }
                            }
                        }
                        if let Some(v) = threw {
                            abrupt = Some(Completion::Throw(v));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::CopyDataPropertiesExcluded(count) => {
                    let mut raw_excluded = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        raw_excluded.push(self.pop_value()?);
                    }
                    raw_excluded.reverse();
                    let source = self.pop_value()?;

                    let mut excluded = Vec::with_capacity(count as usize);
                    for raw_key in raw_excluded {
                        match self.to_property_key_from_builtin(raw_key, context) {
                            Ok(JsValue::String(key)) => {
                                excluded.push(PropertyKey::String(key));
                            }
                            Ok(JsValue::Symbol(symbol)) => {
                                excluded.push(PropertyKey::Symbol(symbol));
                            }
                            Ok(_) => unreachable!("ToPropertyKey returns string or symbol"),
                            Err(error) => match self.error_to_operation_result(error)? {
                                OperationResult::Value(_) => unreachable!(),
                                OperationResult::Throw(value) => {
                                    abrupt = Some(Completion::Throw(value));
                                    discard_saved_finally = true;
                                    break;
                                }
                            },
                        }
                    }

                    if abrupt.is_none() {
                        match self.copy_data_properties_excluded(source, &excluded, context)? {
                            OperationResult::Value(value) => self.stack.push(value),
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                        }
                    }
                }
                Instruction::SetObjectPrototype => {
                    let prototype = self.pop_value()?;
                    let object = context.require_object(self.peek_value()?, "set prototype")?;
                    match prototype.clone() {
                        JsValue::Null => {
                            context.set_prototype_of(object, None)?;
                        }
                        JsValue::Object(prototype) => {
                            context.set_prototype_of(object, Some(prototype))?;
                        }
                        _ => {
                            if let Some(prototype) = context.value_object(&prototype) {
                                context.set_prototype_of(object, Some(prototype))?;
                            }
                        }
                    }
                }
                Instruction::DefineElement(index) => {
                    let value = self.pop_value()?;
                    let array = self.peek_value()?.clone();
                    context.set_element(array, JsValue::Number(index as f64), value)?;
                }
                Instruction::DeleteProperty(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    match context.property_lookup_object(&value) {
                        Ok(object) => {
                            let strict = context.is_strict_code();
                            let result = if context.proxy_record(object).is_some() {
                                proxy::internal_delete(
                                    self,
                                    context,
                                    value,
                                    &PropertyKey::String(name),
                                )
                            } else {
                                context.delete_property(object, &name, strict)
                            };
                            match result {
                                Ok(false) if strict => {
                                    abrupt = Some(Completion::Throw(vm_error_to_value(
                                        VmError::type_error("cannot delete property"),
                                    )));
                                    discard_saved_finally = true;
                                }
                                Ok(deleted) => self.stack.push(JsValue::Boolean(deleted)),
                                Err(error) => match self.error_to_operation_result(error)? {
                                    OperationResult::Throw(value) => {
                                        abrupt = Some(Completion::Throw(value));
                                        discard_saved_finally = true;
                                    }
                                    OperationResult::Value(_) => unreachable!(),
                                },
                            }
                        }
                        Err(error) => match self.error_to_operation_result(error)? {
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                            OperationResult::Value(_) => unreachable!(),
                        },
                    }
                }
                Instruction::DeleteName(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    match context.resolve_binding_environment(&name)? {
                        Some(_env) => {
                            // Binding exists — declared bindings are
                            // non-configurable, so delete returns false
                            // (non-strict) or throws (strict). With-object
                            // property deletion is handled separately.
                            if context.is_strict_code() {
                                abrupt = Some(Completion::Throw(
                                    vm_error_to_value(VmError::type_error(
                                        "cannot delete property",
                                    )),
                                ));
                                discard_saved_finally = true;
                            } else {
                                self.stack.push(JsValue::Boolean(false));
                            }
                        }
                        None => {
                            // Unresolvable reference — always returns true
                            // in non-strict mode. Strict-mode delete of
                            // identifier is rejected at parse time.
                            self.stack.push(JsValue::Boolean(true));
                        }
                    }
                }
                Instruction::DeleteElement => {
                    let key = self.pop_value()?;
                    let value = self.pop_value()?;
                    match context.property_lookup_object(&value) {
                        Ok(object) => {
                            let strict = context.is_strict_code();
                            let result = if context.proxy_record(object).is_some() {
                                let property_key = to_property_key(&key)?;
                                proxy::internal_delete(self, context, value, &property_key)
                            } else {
                                match to_property_key(&key)? {
                                    PropertyKey::Symbol(symbol) => {
                                        context.delete_symbol_property(object, symbol, strict)
                                    }
                                    PropertyKey::String(key) => {
                                        context.delete_property(object, &key, strict)
                                    }
                                }
                            };
                            match result {
                                Ok(false) if strict => {
                                    abrupt = Some(Completion::Throw(vm_error_to_value(
                                        VmError::type_error("cannot delete property"),
                                    )));
                                    discard_saved_finally = true;
                                }
                                Ok(deleted) => self.stack.push(JsValue::Boolean(deleted)),
                                Err(error) => match self.error_to_operation_result(error)? {
                                    OperationResult::Throw(value) => {
                                        abrupt = Some(Completion::Throw(value));
                                        discard_saved_finally = true;
                                    }
                                    OperationResult::Value(_) => unreachable!(),
                                },
                            }
                        }
                        Err(error) => match self.error_to_operation_result(error)? {
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                            OperationResult::Value(_) => unreachable!(),
                        },
                    }
                }
                Instruction::HasProperty => {
                    let value = self.pop_value()?;
                    context.require_object(&value, "test property")?;
                    let key = self.pop_value()?;
                    let property_key = to_property_key(&key)?;
                    let result = proxy::internal_has_property(self, context, value, &property_key);
                    match result {
                        Ok(has) => self.stack.push(JsValue::Boolean(has)),
                        Err(error) => match self.error_to_operation_result(error)? {
                            OperationResult::Throw(value) => {
                                abrupt = Some(Completion::Throw(value));
                                discard_saved_finally = true;
                            }
                            OperationResult::Value(_) => unreachable!(),
                        },
                    }
                }
                Instruction::InstanceOf => {
                    let constructor = self.pop_value()?;
                    let value = self.pop_value()?;
                    match self.instance_of_value(value, constructor, context)? {
                        OperationResult::Value(value) => self.stack.push(value),
                        OperationResult::Throw(value) => {
                            abrupt = Some(Completion::Throw(value));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::Duplicate => {
                    self.stack.push(self.peek_value()?.clone());
                }
                Instruction::DuplicatePair => {
                    let depth = self.stack.len();
                    if depth < 2 {
                        return Err(VmError::runtime("operand stack underflow"));
                    }
                    let first = self.stack[depth - 2].clone();
                    let second = self.stack[depth - 1].clone();
                    self.stack.push(first);
                    self.stack.push(second);
                }
                Instruction::Swap => {
                    let depth = self.stack.len();
                    if depth < 2 {
                        return Err(VmError::runtime("operand stack underflow"));
                    }
                    self.stack.swap(depth - 1, depth - 2);
                }
                Instruction::SetFunctionName(index) => {
                    let inferred_name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    self.maybe_set_function_name(&inferred_name, context);
                }
                Instruction::CreateLexicalEnvironment => {
                    context.push_environment(Some(context.current_environment()))?;
                }
                Instruction::EnterWithEnvironment => {
                    let value = self.pop_value()?;
                    context.push_with_environment(value)?;
                }
                Instruction::PopEnvironment => {
                    context.pop_environment()?;
                }
                Instruction::CreateMutableBinding(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    if !(context
                        .module_registry()
                        .is_module_environment(context.current_environment())
                        && context.has_binding_in_environment(context.current_environment(), &name))
                    {
                        context.create_mutable_binding(
                            context.current_environment(),
                            name,
                            false,
                            true,
                        )?;
                    }
                }
                Instruction::CreateImmutableBinding(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    if !(context
                        .module_registry()
                        .is_module_environment(context.current_environment())
                        && context.has_binding_in_environment(context.current_environment(), &name))
                    {
                        context.create_immutable_binding(
                            context.current_environment(),
                            name.clone(),
                            true,
                        )?;
                    }
                    if let Some((target_environment, target_name)) =
                        context.take_pending_module_import_link(&name)
                    {
                        context.create_module_import_link(
                            context.current_environment(),
                            name,
                            target_environment,
                            target_name,
                        );
                    } else if let Some(value) = context.take_pending_module_import(&name) {
                        context.initialize_binding(context.current_environment(), &name, value)?;
                    }
                }
                Instruction::InitializeBinding(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = self.pop_value()?;
                    let environment =
                        context.resolve_binding_environment(name)?.ok_or_else(|| {
                            VmError::reference(format!(
                                "{name} is not defined at instruction {current_instruction}"
                            ))
                        })?;
                    context.initialize_binding(environment, name, value)?;
                }
                Instruction::CreatePrivateBrand(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let binding_name = format!("\0#brand#{name}");
                    let brand = context.allocate_private_brand();
                    context.declare_binding(
                        context.current_environment(),
                        binding_name,
                        JsValue::Number(f64::from(brand.0)),
                        false,
                        false,
                    )?;
                }
                Instruction::LoadException => {
                    let value = self.pending_exception.take().ok_or_else(|| {
                        VmError::runtime("LoadException executed without a pending exception")
                    })?;
                    self.stack.push(value);
                }
                Instruction::EndFinally => {
                    if let Some(saved) = self.finally_stack.pop() {
                        abrupt = Some(saved);
                    }
                }
            }

            if let Some(completion) = abrupt {
                if discard_saved_finally {
                    self.finally_stack.pop();
                }
                if self.enter_handler(
                    chunk,
                    current_instruction,
                    completion.clone(),
                    baseline,
                    context,
                    &mut instruction_pointer,
                )? {
                    continue;
                }
                return Ok(completion);
            }
        }

        if stop_ip.is_some() {
            Ok(Completion::Normal(JsValue::Undefined))
        } else {
            Err(VmError::runtime(
                "bytecode ended without a return instruction",
            ))
        }
    }

    fn constant_at<'a>(
        &self,
        chunk: &'a Chunk,
        index: u16,
        instruction_pointer: usize,
    ) -> Result<&'a Constant, VmError> {
        chunk.constants.get(index as usize).ok_or_else(|| {
            VmError::runtime(format!(
                "constant index {index} is out of bounds at instruction {instruction_pointer}"
            ))
        })
    }

    fn constant_string<'a>(
        &self,
        chunk: &'a Chunk,
        index: u16,
        instruction_pointer: usize,
    ) -> Result<&'a str, VmError> {
        match self.constant_at(chunk, index, instruction_pointer)? {
            Constant::String(value) => Ok(value),
            _ => Err(VmError::runtime(format!(
                "constant index {index} must refer to a string at instruction {instruction_pointer}"
            ))),
        }
    }

    fn pop_value(&mut self) -> Result<JsValue, VmError> {
        self.stack
            .pop()
            .ok_or_else(|| VmError::runtime("operand stack underflow"))
    }

    fn peek_value(&self) -> Result<&JsValue, VmError> {
        self.stack
            .last()
            .ok_or_else(|| VmError::runtime("operand stack underflow"))
    }

    fn pop_arguments(&mut self, count: u16) -> Result<Vec<JsValue>, VmError> {
        let mut arguments = Vec::with_capacity(count as usize);
        for _ in 0..count {
            arguments.push(self.pop_value()?);
        }
        arguments.reverse();
        Ok(arguments)
    }

    fn enter_handler(
        &mut self,
        chunk: &Chunk,
        instruction_offset: usize,
        completion: Completion,
        baseline: RunBaseline,
        context: &mut NativeContext,
        instruction_pointer: &mut usize,
    ) -> Result<bool, VmError> {
        let Some(handler) = find_handler(chunk, instruction_offset, &completion) else {
            return Ok(false);
        };

        let stack_depth = baseline.stack_depth + handler.stack_depth as usize;
        if stack_depth > self.stack.len() {
            return Err(VmError::runtime(format!(
                "handler restores stack depth {stack_depth} above current depth {}",
                self.stack.len()
            )));
        }
        self.stack.truncate(stack_depth);
        context.restore_environment_depth(
            baseline.environment_depth + handler.environment_depth as usize,
        )?;

        match handler.kind {
            HandlerKind::Catch => {
                let Completion::Throw(value) = completion else {
                    return Err(VmError::runtime(
                        "catch handler received a non-throw completion",
                    ));
                };
                self.pending_exception = Some(value);
            }
            HandlerKind::Finally => {
                self.finally_stack.push(completion);
            }
        }
        *instruction_pointer = handler.target;
        Ok(true)
    }

    fn create_function(
        &mut self,
        chunk: &Chunk,
        index: u16,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let template = chunk
            .functions
            .get(index as usize)
            .cloned()
            .ok_or_else(|| VmError::runtime(format!("function index {index} is out of bounds")))?;
        let environment = match template.environment_policy {
            EnvironmentCapturePolicy::None => None,
            EnvironmentCapturePolicy::CaptureCurrent => Some(context.current_environment()),
        };
        let is_async = template.is_async;
        let is_generator = template.is_generator;
        let is_arrow = template.is_arrow;
        let lexical_this = is_arrow.then(|| context.current_or_global_this());
        let lexical_new_target = is_arrow.then(|| context.current_new_target());
        let home_object = if is_arrow {
            context
                .current_function()
                .and_then(|function| context.function(function))
                .and_then(|function| function.home_object)
        } else {
            None
        };
        let id = context.allocate_function(JsFunction {
            name: template.name,
            params: template.params,
            rest_param: template.rest_param,
            length_override: template.length_override,
            chunk: template.chunk,
            environment,
            is_async,
            is_generator,
            is_arrow,
            binds_name_in_activation: template.binds_name_in_activation,
            is_derived_constructor: template.is_derived_constructor,
            is_constructable: template.is_constructable,
            has_own_prototype_property: template.has_own_prototype_property,
            prototype_writable: template.prototype_writable,
            uses_arguments: template.uses_arguments,
            lexical_this,
            lexical_new_target,
            home_object,
        })?;
        if is_generator && let Some(generator_prototype) = context.function_prototype(id) {
            let iterator_prototype = if is_async {
                intrinsic_async_iterator_prototype(context)
            } else {
                intrinsic_iterator_prototype(context)
            };
            if let Some(iterator_prototype) = iterator_prototype {
                if is_async {
                    let mut async_generator_prototype = JsObject::ordinary();
                    async_generator_prototype.prototype = Some(iterator_prototype);
                    let async_generator_prototype = context
                        .heap_mut()
                        .allocate_object(async_generator_prototype)
                        .ok_or_else(|| {
                            VmError::runtime_limit("async generator prototype arena exhausted")
                        })?;
                    context
                        .set_prototype_of(generator_prototype, Some(async_generator_prototype))?;
                } else {
                    context.set_prototype_of(generator_prototype, Some(iterator_prototype))?;
                }
            }
        }
        if template.is_strict || context.strict() {
            context.mark_strict_function(id);
            context.install_restricted_function_properties(id)?;
        }
        Ok(JsValue::Function(id))
    }

    fn create_generator_object(
        &mut self,
        function: FunctionId,
        environment: Option<EnvironmentId>,
        environment_stack: Vec<EnvironmentId>,
        current_environment: EnvironmentId,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        next_ip: usize,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let record = GeneratorRecord {
            function,
            environment,
            environment_stack,
            current_environment,
            this_value,
            arguments,
            next_ip,
            state: GeneratorState::SuspendedStart,
            stack: Vec::new(),
            delegate_values: Vec::new(),
            delegate_iterator: None,
            delegate_return: None,
        };
        let is_async = context
            .function(function)
            .is_some_and(|function| function.is_async);
        let mut object = JsObject::ordinary();
        object.prototype = context
            .function_prototype(function)
            .or_else(|| context.object_prototype());
        object.kind = ObjectKind::Generator { record };
        let object = context
            .heap_mut()
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;

        let (next, return_, throw) = if is_async {
            (
                context.register_builtin(
                    "AsyncGenerator.prototype.next",
                    1,
                    async_generator_next,
                    None,
                )?,
                context.register_builtin(
                    "AsyncGenerator.prototype.return",
                    1,
                    async_generator_return,
                    None,
                )?,
                context.register_builtin(
                    "AsyncGenerator.prototype.throw",
                    1,
                    async_generator_throw,
                    None,
                )?,
            )
        } else {
            (
                context.register_builtin("Generator.prototype.next", 1, generator_next, None)?,
                context.register_builtin(
                    "Generator.prototype.return",
                    1,
                    generator_return,
                    None,
                )?,
                context.register_builtin("Generator.prototype.throw", 1, generator_throw, None)?,
            )
        };
        let (iterator_name, iterator_symbol) = if is_async {
            (
                "AsyncGenerator.prototype[Symbol.asyncIterator]",
                context.well_known_symbols().async_iterator,
            )
        } else {
            (
                "Generator.prototype[Symbol.iterator]",
                context.well_known_symbols().iterator,
            )
        };
        let iterator = context.register_builtin(iterator_name, 0, generator_iterator, None)?;
        context.define_own_property(
            object,
            "next".into(),
            PropertyDescriptor::data_with(next, true, false, true),
        )?;
        context.define_own_property(
            object,
            "return".into(),
            PropertyDescriptor::data_with(return_, true, false, true),
        )?;
        context.define_own_property(
            object,
            "throw".into(),
            PropertyDescriptor::data_with(throw, true, false, true),
        )?;
        context.define_symbol_own_property(
            object,
            iterator_symbol,
            PropertyDescriptor::data_with(iterator, true, false, true),
        )?;
        Ok(JsValue::Object(object))
    }

    fn resume_generator(
        &mut self,
        generator: JsValue,
        sent_value: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        self.resume_generator_with_completion(generator, sent_value, None, context)
    }

    fn resume_generator_with_completion(
        &mut self,
        generator: JsValue,
        sent_value: JsValue,
        injected: Option<Completion>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let object = context.require_object(&generator, "Generator.prototype.next")?;
        let mut record = match context.heap().object(object).map(|object| &object.kind) {
            Some(ObjectKind::Generator { record }) => record.clone(),
            _ => {
                return Err(VmError::type_error(
                    "Generator method called on non-generator",
                ));
            }
        };

        match record.state {
            GeneratorState::Completed => {
                return generator_result(context, JsValue::Undefined, true);
            }
            GeneratorState::Executing => {
                return Err(VmError::type_error("generator is already executing"));
            }
            GeneratorState::SuspendedStart | GeneratorState::SuspendedYield => {}
        }

        if !record.delegate_values.is_empty() {
            let value = record.delegate_values.remove(0);
            self.write_generator_record(context, object, record)?;
            return generator_result(context, value, false);
        }
        if let Some(iterator) = record.delegate_iterator.clone() {
            match self.step_yield_star_iterator(iterator, sent_value.clone(), context)? {
                YieldStarStepResult::Yield(value) => {
                    self.write_generator_record(context, object, record)?;
                    return Ok(value);
                }
                YieldStarStepResult::Complete(value) => {
                    record.delegate_iterator = None;
                    record.delegate_return = Some(value);
                }
                YieldStarStepResult::Throw(value) => {
                    record.state = GeneratorState::Completed;
                    record.stack.clear();
                    record.delegate_values.clear();
                    record.delegate_iterator = None;
                    record.delegate_return = None;
                    self.write_generator_record(context, object, record)?;
                    self.pending_exception = Some(value);
                    return Err(VmError::runtime("generator delegate threw"));
                }
            }
        }

        let function = context
            .function(record.function)
            .cloned()
            .ok_or_else(|| VmError::runtime("missing generator function"))?;
        let (caller_environment_stack, caller_current_environment) = context.environment_state();
        let stack_base = self.stack.len();
        let frame_environment = if record.environment.is_some() {
            context.restore_environment_state(
                record.environment_stack.clone(),
                record.current_environment,
            )?;
            record.current_environment
        } else {
            let environment = context.push_environment(function.environment)?;
            if let Err(error) = self.bind_function_environment(
                record.function,
                &function,
                environment,
                &record.arguments,
                record.this_value.clone(),
                context,
            ) {
                let _ = context.restore_environment_state(
                    caller_environment_stack,
                    caller_current_environment,
                );
                return Err(error);
            }
            record.environment = Some(environment);
            let (environment_stack, current_environment) = context.environment_state();
            record.environment_stack = environment_stack;
            record.current_environment = current_environment;
            environment
        };

        let suspended_start = matches!(record.state, GeneratorState::SuspendedStart);
        record.state = GeneratorState::Executing;
        self.write_generator_record(context, object, record.clone())?;

        self.stack.extend(record.stack.iter().cloned());
        if let Some(return_value) = record.delegate_return.take() {
            self.stack.push(return_value);
        } else if !suspended_start && injected.is_none() {
            self.stack.push(sent_value);
        }

        let baseline_environment_depth = record
            .environment
            .and_then(|environment| {
                record
                    .environment_stack
                    .iter()
                    .position(|saved| *saved == environment)
            })
            .unwrap_or_else(|| context.environment_depth());
        let frame = CallFrame::new(
            Some(record.function),
            record.next_ip,
            frame_environment,
            record.this_value.clone(),
            JsValue::Undefined,
            stack_base,
        );
        context.push_call_frame(frame)?;
        let result = self.run_completion_from_with_initial(
            &function.chunk,
            context,
            record.next_ip,
            injected,
            Some(baseline_environment_depth),
        );
        let saved_stack = self.stack[stack_base..].to_vec();
        let (saved_environment_stack, saved_current_environment) = context.environment_state();
        self.stack.truncate(stack_base);
        let frame_result = context.pop_call_frame();
        let environment_result =
            context.restore_environment_state(caller_environment_stack, caller_current_environment);
        frame_result?;
        environment_result?;

        match result? {
            Completion::Yield { value, next_ip } => {
                record.next_ip = next_ip;
                record.state = GeneratorState::SuspendedYield;
                record.stack = saved_stack;
                record.environment_stack = saved_environment_stack;
                record.current_environment = saved_current_environment;
                self.write_generator_record(context, object, record)?;
                generator_result(context, value, false)
            }
            Completion::YieldDelegate {
                iterator,
                value,
                next_ip,
            } => {
                record.next_ip = next_ip;
                record.state = GeneratorState::SuspendedYield;
                record.stack = saved_stack;
                record.environment_stack = saved_environment_stack;
                record.current_environment = saved_current_environment;
                record.delegate_iterator = Some(iterator);
                self.write_generator_record(context, object, record)?;
                Ok(value)
            }
            Completion::Normal(value) | Completion::Return(value) => {
                record.state = GeneratorState::Completed;
                record.stack.clear();
                record.delegate_values.clear();
                record.delegate_iterator = None;
                record.delegate_return = None;
                record.environment_stack.clear();
                if let Some(environment) = record.environment {
                    record.current_environment = environment;
                }
                self.write_generator_record(context, object, record)?;
                generator_result(context, value, true)
            }
            Completion::Throw(value) => {
                record.state = GeneratorState::Completed;
                record.stack.clear();
                record.delegate_values.clear();
                record.delegate_iterator = None;
                record.delegate_return = None;
                record.environment_stack.clear();
                if let Some(environment) = record.environment {
                    record.current_environment = environment;
                }
                self.write_generator_record(context, object, record)?;
                self.pending_exception = Some(value);
                Err(VmError::runtime("generator body threw"))
            }
            Completion::Break(label) => Err(VmError::runtime(format!(
                "unhandled break completion{}",
                label_suffix(label.as_deref())
            ))),
            Completion::Continue(label) => Err(VmError::runtime(format!(
                "unhandled continue completion{}",
                label_suffix(label.as_deref())
            ))),
        }
    }

    fn write_generator_record(
        &mut self,
        context: &mut NativeContext,
        object: ObjectId,
        record: GeneratorRecord,
    ) -> Result<(), VmError> {
        let Some(object) = context.heap_mut().object_mut(object) else {
            return Err(VmError::runtime("missing generator object"));
        };
        object.kind = ObjectKind::Generator { record };
        Ok(())
    }

    fn bind_function_environment(
        &mut self,
        function_id: FunctionId,
        function: &JsFunction,
        environment: crate::runtime::EnvironmentId,
        arguments: &[JsValue],
        this_value: JsValue,
        context: &mut NativeContext,
    ) -> Result<(), VmError> {
        for (index, parameter) in function.params.iter().enumerate() {
            let value = arguments.get(index).cloned().unwrap_or(JsValue::Undefined);
            context.declare_binding(environment, parameter.clone(), value, true, true)?;
        }

        if let Some(rest_name) = &function.rest_param {
            let rest_values = arguments
                .get(function.params.len()..)
                .unwrap_or(&[])
                .to_vec();
            let rest_array = context.create_array(rest_values)?;
            context.declare_binding(environment, rest_name.clone(), rest_array, true, true)?;
        }

        if let Some(name) = function
            .name
            .as_ref()
            .filter(|_| Self::should_bind_function_name_in_activation(function))
        {
            context.declare_binding(
                environment,
                name.clone(),
                JsValue::Function(function_id),
                true,
                true,
            )?;
        }

        let has_explicit_arguments = function.is_arrow
            || function.params.iter().any(|p| p == "arguments")
            || function.rest_param.as_deref() == Some("arguments");
        if has_explicit_arguments || !function.uses_arguments {
            return Ok(());
        }

        let proto = context.object_prototype();
        let arguments_obj = context.ordinary_object_with_prototype(proto)?;
        let JsValue::Object(arguments_id) = arguments_obj else {
            unreachable!("ordinary_object_with_prototype always returns Object")
        };
        for (i, arg) in arguments.iter().enumerate() {
            context.define_own_property(
                arguments_id,
                i.to_string(),
                PropertyDescriptor::data(arg.clone()),
            )?;
        }
        context.define_own_property(
            arguments_id,
            "length".into(),
            PropertyDescriptor::data_with(
                JsValue::Number(arguments.len() as f64),
                true,
                false,
                true,
            ),
        )?;
        define_arguments_iterator(context, arguments_id)?;
        context.declare_binding(
            environment,
            "arguments",
            JsValue::Object(arguments_id),
            true,
            false,
        )?;

        let _ = this_value;
        Ok(())
    }

    fn instance_of_value(
        &mut self,
        value: JsValue,
        constructor: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        if let Some(object) = context.value_object(&constructor) {
            let symbol = context.well_known_symbols().has_instance;
            if let Some(method) = context.get_symbol_property_value(object, symbol)
                && !matches!(method, JsValue::Undefined | JsValue::Null)
            {
                if !is_callable_value(&method) {
                    return Ok(OperationResult::Throw(vm_error_to_value(
                        VmError::type_error("Symbol.hasInstance method is not callable"),
                    )));
                }
                return match self.call_value(method, constructor, vec![value], context)? {
                    OperationResult::Value(result) => Ok(OperationResult::Value(JsValue::Boolean(
                        result.to_boolean(),
                    ))),
                    OperationResult::Throw(value) => Ok(OperationResult::Throw(value)),
                };
            }
        }

        match self.ordinary_instance_of(value, constructor, context) {
            Ok(result) => Ok(OperationResult::Value(JsValue::Boolean(result))),
            Err(error) => self.error_to_operation_result(error),
        }
    }

    pub(crate) fn ordinary_instance_of(
        &mut self,
        value: JsValue,
        constructor: JsValue,
        context: &mut NativeContext,
    ) -> Result<bool, VmError> {
        if matches!(value, JsValue::Error(_)) {
            return context.ordinary_instance_of(value, constructor);
        }
        if let JsValue::BuiltinFunction(id) = &constructor
            && let Some(bound) = context
                .builtin(*id)
                .and_then(|builtin| builtin.bound.as_ref())
        {
            return self.ordinary_instance_of(value, bound.target.clone(), context);
        }
        if !context.is_constructable_value(&constructor) {
            return Err(VmError::type_error(
                "right-hand side of instanceof is not a constructor",
            ));
        }
        let Some(object) = context.value_object(&value) else {
            return Ok(false);
        };
        context.value_object(&constructor).ok_or_else(|| {
            VmError::type_error("right-hand side of instanceof is not a constructor")
        })?;
        let prototype_value = proxy::internal_get(
            self,
            context,
            constructor.clone(),
            &PropertyKey::String("prototype".into()),
            constructor,
        )?;
        let prototype = context
            .value_object(&prototype_value)
            .ok_or_else(|| VmError::type_error("constructor prototype is not an object"))?;

        let mut current =
            proxy::internal_get_prototype_of(self, context, context.object_value(object))?;
        let mut depth = 0usize;
        while let Some(object) = current {
            if depth > 1024 {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            if object == prototype {
                return Ok(true);
            }
            current =
                proxy::internal_get_prototype_of(self, context, context.object_value(object))?;
            depth += 1;
        }
        Ok(false)
    }

    pub(crate) fn get_prototype_from_constructor(
        &mut self,
        constructor: JsValue,
        context: &mut NativeContext,
    ) -> Result<Option<ObjectId>, VmError> {
        self.get_prototype_from_constructor_with_default(constructor, context, |context, value| {
            context.default_object_prototype_for_callable(value)
        })
    }

    pub(crate) fn get_array_prototype_from_constructor(
        &mut self,
        constructor: JsValue,
        context: &mut NativeContext,
    ) -> Result<Option<ObjectId>, VmError> {
        self.get_prototype_from_constructor_with_default(constructor, context, |context, value| {
            context.default_array_prototype_for_callable(value)
        })
    }

    pub(crate) fn get_boolean_prototype_from_constructor(
        &mut self,
        constructor: JsValue,
        context: &mut NativeContext,
    ) -> Result<Option<ObjectId>, VmError> {
        self.get_prototype_from_constructor_with_default(constructor, context, |context, value| {
            context.default_boolean_prototype_for_callable(value)
        })
    }

    fn get_prototype_from_constructor_with_default(
        &mut self,
        constructor: JsValue,
        context: &mut NativeContext,
        default: fn(&NativeContext, &JsValue) -> Option<ObjectId>,
    ) -> Result<Option<ObjectId>, VmError> {
        context.require_object(&constructor, "value is not a constructor")?;
        let prototype_value = proxy::internal_get(
            self,
            context,
            constructor.clone(),
            &PropertyKey::String("prototype".into()),
            constructor.clone(),
        )?;
        Ok(context
            .value_object(&prototype_value)
            .or_else(|| default(context, &constructor)))
    }

    fn call_value(
        &mut self,
        callee: JsValue,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        match callee {
            JsValue::Object(object) if context.is_function_prototype_object(object) => {
                Ok(OperationResult::Value(JsValue::Undefined))
            }
            JsValue::Object(object) if context.proxy_record(object).is_some() => {
                match proxy::internal_call(
                    self,
                    context,
                    JsValue::Object(object),
                    this_value,
                    arguments,
                ) {
                    Ok(value) => Ok(OperationResult::Value(value)),
                    Err(error) => self.error_to_operation_result_in_context(error, context),
                }
            }
            JsValue::Function(function) => {
                let activation = match context.realm_for_function(function) {
                    Some(realm) if !context.is_current_realm(realm) => {
                        Some(context.enter_realm(realm)?)
                    }
                    None => None,
                    Some(_) => None,
                };
                let operation = self.call_user_function(
                    function,
                    this_value,
                    arguments,
                    JsValue::Undefined,
                    context,
                );
                match (
                    operation,
                    activation.map(|activation| context.leave_realm(activation)),
                ) {
                    (Ok(value), None) => Ok(value),
                    (Err(error), None) => Err(error),
                    (Ok(value), Some(Ok(()))) => Ok(value),
                    (Err(error), Some(Ok(()))) => Err(error),
                    (Ok(_), Some(Err(error))) | (Err(error), Some(Err(_))) => Err(error),
                }
            }
            JsValue::BuiltinFunction(id) => {
                context.consume_call_depth()?;
                let result: Result<OperationResult, VmError> = (|| {
                    let activation = match context.realm_for_builtin(id) {
                        Some(realm) if !context.is_current_realm(realm) => {
                            Some(context.enter_realm(realm)?)
                        }
                        None => None,
                        Some(_) => None,
                    };
                    let operation = (|| {
                        let def = context
                            .builtin(id)
                            .ok_or_else(|| VmError::runtime("invalid builtin id"))?
                            .clone();
                        // A bound function forwards to its target with the bound `this`
                        // and bound arguments prepended.
                        if let Some(bound) = &def.bound {
                            let mut forwarded = bound.args.clone();
                            forwarded.extend(arguments);
                            let target = bound.target.clone();
                            let this_value = bound.this_value.clone();
                            return self.call_value(target, this_value, forwarded, context);
                        }
                        if context.is_function_prototype_call(id) {
                            let target = this_value;
                            if !context.is_callable_value(&target) {
                                return Ok(OperationResult::Throw(vm_error_to_value_with_realm(
                                    VmError::type_error(
                                        "Function.prototype.call receiver is not callable",
                                    ),
                                    context.global_object(),
                                )));
                            }
                            let call_this =
                                arguments.first().cloned().unwrap_or(JsValue::Undefined);
                            let forwarded = arguments.into_iter().skip(1).collect();
                            return self.call_value(target, call_this, forwarded, context);
                        }
                        if context.is_function_prototype_apply(id) {
                            let target = this_value;
                            // IsCallable is checked before reading `argArray`; getters on an
                            // argument list must not run for a non-callable receiver.
                            if !context.is_callable_value(&target) {
                                return Ok(OperationResult::Throw(vm_error_to_value_with_realm(
                                    VmError::type_error(
                                        "Function.prototype.apply receiver is not callable",
                                    ),
                                    context.global_object(),
                                )));
                            }
                            let apply_this =
                                arguments.first().cloned().unwrap_or(JsValue::Undefined);
                            let arg_array = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
                            let forwarded = match self.function_apply_arguments(arg_array, context)
                            {
                                Ok(values) => values,
                                Err(error) => match self
                                    .error_to_operation_result_in_context(error, context)?
                                {
                                    OperationResult::Throw(value) => {
                                        return Ok(OperationResult::Throw(value));
                                    }
                                    OperationResult::Value(_) => unreachable!(),
                                },
                            };
                            return self.call_value(target, apply_this, forwarded, context);
                        }
                        context.push_current_builtin(id);
                        let call_result = (def.call)(self, context, this_value, &arguments);
                        context.pop_current_builtin();
                        match call_result {
                            Ok(value) => Ok(OperationResult::Value(value)),
                            Err(error) => match self.pending_exception.take() {
                                // A nested JavaScript callback threw; surface its value.
                                Some(value) => Ok(OperationResult::Throw(value)),
                                // ECMAScript error types raised directly by a builtin are
                                // catchable throws; engine-internal failures are not.
                                None => match error.kind {
                                    VmErrorKind::Reference
                                    | VmErrorKind::Type
                                    | VmErrorKind::Syntax
                                    | VmErrorKind::Range
                                    | VmErrorKind::Test262 => {
                                        Ok(OperationResult::Throw(vm_error_to_value(error)))
                                    }
                                    _ => Err(error),
                                },
                            },
                        }
                    })();
                    match (
                        operation,
                        activation.map(|activation| context.leave_realm(activation)),
                    ) {
                        (Ok(value), None) => Ok(value),
                        (Err(error), None) => Err(error),
                        (Ok(value), Some(Ok(()))) => Ok(value),
                        (Err(error), Some(Ok(()))) => Err(error),
                        (Ok(_), Some(Err(error))) | (Err(error), Some(Err(_))) => Err(error),
                    }
                })();
                context.release_call_depth();
                result
            }
            other => Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error(format!("{other} is not callable")),
            ))),
        }
    }

    /// Constructor-call half of `super()`: unlike an ordinary call, preserve
    /// the derived constructor's `new.target` while using its current receiver.
    fn call_super_constructor(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let new_target = context.current_new_target();
        match self.construct_value_with_new_target(constructor, arguments, new_target, context)? {
            OperationResult::Value(value) if is_object_like(&value) => {
                context.initialize_derived_this(value.clone())?;
                Ok(OperationResult::Value(value))
            }
            OperationResult::Value(_) => Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("super constructor did not return an object"),
            ))),
            OperationResult::Throw(value) => Ok(OperationResult::Throw(value)),
        }
    }

    fn function_apply_arguments(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<Vec<JsValue>, VmError> {
        if matches!(value, JsValue::Undefined | JsValue::Null) {
            return Ok(Vec::new());
        }
        let object = context.require_object(&value, "Function.prototype.apply")?;
        let object_value = context.object_value(object);
        let length_value =
            match self.get_property_value_completion(object_value.clone(), "length", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => {
                    self.pending_exception = Some(value);
                    return Err(VmError::runtime(
                        "Function.prototype.apply length getter threw",
                    ));
                }
            };
        let length_number = self.to_number(length_value, context)?;
        let length = if length_number.is_nan() || length_number <= 0.0 {
            0
        } else if !length_number.is_finite() {
            return Err(VmError::range("argument list is too large"));
        } else {
            length_number.floor() as usize
        };
        if length > 1_000_000 {
            return Err(VmError::range("argument list is too large"));
        }
        let mut values = Vec::with_capacity(length);
        for index in 0..length {
            let value = match self.get_property_value_completion(
                object_value.clone(),
                &index.to_string(),
                context,
            )? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => {
                    self.pending_exception = Some(value);
                    return Err(VmError::runtime(
                        "Function.prototype.apply element getter threw",
                    ));
                }
            };
            values.push(value);
        }
        Ok(values)
    }

    /// Collect all values from an iterable using the iterator protocol.
    /// Returns `Ok(Ok(values))` on success or `Ok(Err(throw_val))` when the iterator throws.
    fn collect_iterable_spread(
        &mut self,
        iterable: JsValue,
        context: &mut NativeContext,
    ) -> Result<Result<Vec<JsValue>, JsValue>, VmError> {
        let iterator = match self.create_iterator_object(iterable, context)? {
            OperationResult::Value(iter) => iter,
            OperationResult::Throw(throw_val) => return Ok(Err(throw_val)),
        };
        let mut values = Vec::new();
        loop {
            context.consume_loop_iteration()?;
            match self.step_iterator_object(iterator.clone(), context)? {
                IteratorStepResult::Value { done: true, .. } => return Ok(Ok(values)),
                IteratorStepResult::Value { value, done: false } => values.push(value),
                IteratorStepResult::Throw(throw_val) => return Ok(Err(throw_val)),
            }
        }
    }

    fn create_iterator_object(
        &mut self,
        iterable: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        // Fast paths: already an iterator, generator, or string — bypass Symbol.iterator lookup.
        let is_already_handled = match &iterable {
            JsValue::String(_) => true,
            JsValue::Object(id) => context.heap().object(*id).is_some_and(|o| {
                matches!(
                    o.kind,
                    ObjectKind::Iterator { .. } | ObjectKind::Generator { .. }
                )
            }),
            _ => false,
        };
        if is_already_handled {
            return match context.create_iterator_object(iterable.clone()) {
                Ok(iterator) => Ok(OperationResult::Value(iterator)),
                Err(error) if matches!(error.kind, VmErrorKind::Type) => {
                    Ok(OperationResult::Throw(vm_error_to_value(
                        VmError::type_error("value is not iterable"),
                    )))
                }
                Err(error) => Err(error),
            };
        }

        // Check whether the iterable is an Array object (which MUST use Symbol.iterator).
        let is_array = matches!(&iterable, JsValue::Object(id) if context
            .heap()
            .object(*id)
            .is_some_and(|o| matches!(o.kind, ObjectKind::Array { .. })));

        // Spec-correct: look up Symbol.iterator first.
        let iterator_symbol = context.well_known_symbols().iterator;
        let method = match self.get_symbol_property_value_completion(
            iterable.clone(),
            iterator_symbol,
            context,
        )? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
        };

        if matches!(method, JsValue::Undefined | JsValue::Null) {
            if is_array {
                // Arrays without Symbol.iterator are not iterable (e.g., after deletion).
                return Ok(OperationResult::Throw(vm_error_to_value(
                    VmError::type_error("value is not iterable"),
                )));
            }
            // Non-array objects without Symbol.iterator: fall back to the native fast path
            // (handles array-like objects such as `arguments` that lack Symbol.iterator in our impl).
            return match context.create_iterator_object(iterable.clone()) {
                Ok(iterator) => Ok(OperationResult::Value(iterator)),
                Err(error) if matches!(error.kind, VmErrorKind::Type) => {
                    Ok(OperationResult::Throw(vm_error_to_value(
                        VmError::type_error("value is not iterable"),
                    )))
                }
                Err(error) => Err(error),
            };
        }

        if !is_callable_value(&method) {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("iterator method is not callable"),
            )));
        }
        let iterator = match self.call_value(method, iterable, Vec::new(), context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
        };
        if !is_object_like(&iterator) {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("iterator method returned a non-object"),
            )));
        }
        // If Symbol.iterator returned a native iterator object (e.g., from Array.prototype.values),
        // use it directly without wrapping.
        if let JsValue::Object(id) = &iterator {
            if context
                .heap()
                .object(*id)
                .is_some_and(|o| matches!(o.kind, ObjectKind::Iterator { .. }))
            {
                return Ok(OperationResult::Value(iterator));
            }
        }
        match self.wrap_js_iterator(iterator, context) {
            Ok(iterator) => Ok(OperationResult::Value(iterator)),
            Err(error) => self.error_to_operation_result_in_context(error, context),
        }
    }

    fn create_async_iterator_object(
        &mut self,
        iterable: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let method = match self.get_symbol_property_value_completion(
            iterable.clone(),
            context.well_known_symbols().async_iterator,
            context,
        )? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
        };
        if matches!(method, JsValue::Undefined | JsValue::Null) {
            return self.create_iterator_object(iterable, context);
        }
        if !is_callable_value(&method) {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator method is not callable"),
            )));
        }
        let iterator = match self.call_value(method, iterable, Vec::new(), context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
        };
        if !is_object_like(&iterator) {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator method returned a non-object"),
            )));
        }
        let object = JsObject::iterator(IteratorRecord::js_async(iterator));
        let id = context
            .heap_mut()
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime("heap full: cannot allocate async iterator object"))?;
        Ok(OperationResult::Value(JsValue::Object(id)))
    }

    fn step_iterator_object(
        &mut self,
        iterator_val: JsValue,
        context: &mut NativeContext,
    ) -> Result<IteratorStepResult, VmError> {
        let id = match &iterator_val {
            JsValue::Object(id) => *id,
            _ => {
                return Ok(IteratorStepResult::Throw(vm_error_to_value(
                    VmError::type_error("value is not an iterator object"),
                )));
            }
        };
        let kind = {
            let object = context
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &object.kind {
                ObjectKind::Iterator { record } => record.kind.clone(),
                _ => {
                    return Ok(IteratorStepResult::Throw(vm_error_to_value(
                        VmError::type_error("object is not an iterator"),
                    )));
                }
            }
        };

        match kind {
            IteratorKind::Array { .. } | IteratorKind::String { .. } => {
                let (value, done) = match self.step_native_iterator_object(iterator_val, context) {
                    Ok(result) => result,
                    Err(error) => match self.error_to_operation_result(error)? {
                        OperationResult::Throw(value) => {
                            return Ok(IteratorStepResult::Throw(value));
                        }
                        OperationResult::Value(_) => unreachable!(),
                    },
                };
                Ok(IteratorStepResult::Value { value, done })
            }
            IteratorKind::Js {
                iterator,
                next_method,
            } => {
                let result = self.step_js_iterator(iterator, next_method, context)?;
                // Update done flag in the stored record so IteratorClose knows if we're exhausted.
                if matches!(
                    &result,
                    IteratorStepResult::Value { done: true, .. } | IteratorStepResult::Throw(_)
                ) {
                    if let Some(obj) = context.heap_mut().object_mut(id) {
                        if let ObjectKind::Iterator { record } = &mut obj.kind {
                            record.done = true;
                        }
                    }
                }
                Ok(result)
            }
            IteratorKind::JsAsync { .. } => Ok(IteratorStepResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator requires AsyncIteratorNext"),
            ))),
        }
    }

    fn step_async_iterator_object(
        &mut self,
        iterator_val: JsValue,
        context: &mut NativeContext,
    ) -> Result<IteratorStepResult, VmError> {
        let id = context.require_object(&iterator_val, "async iterator")?;
        let kind = context
            .heap()
            .object(id)
            .and_then(|object| match &object.kind {
                ObjectKind::Iterator { record } => Some(record.kind.clone()),
                _ => None,
            })
            .ok_or_else(|| VmError::type_error("object is not an iterator"))?;
        if let IteratorKind::JsAsync { iterator } = kind {
            return self.step_js_async_iterator(iterator, context);
        }

        match self.step_iterator_object(iterator_val, context)? {
            IteratorStepResult::Value { value, done: false } => {
                match self.await_value_now(value, context)? {
                    OperationResult::Value(value) => {
                        Ok(IteratorStepResult::Value { value, done: false })
                    }
                    OperationResult::Throw(value) => Ok(IteratorStepResult::Throw(value)),
                }
            }
            result => Ok(result),
        }
    }

    fn step_js_async_iterator(
        &mut self,
        iterator: JsValue,
        context: &mut NativeContext,
    ) -> Result<IteratorStepResult, VmError> {
        let next = match self.get_property_value_completion(iterator.clone(), "next", context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        if !is_callable_value(&next) {
            return Ok(IteratorStepResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator next is not callable"),
            )));
        }
        let result = match self.call_value(next, iterator, Vec::new(), context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        let result = match self.await_value_now(result, context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        if !is_object_like(&result) {
            return Ok(IteratorStepResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator next returned a non-object"),
            )));
        }
        let done = match self.get_property_value_completion(result.clone(), "done", context)? {
            OperationResult::Value(value) => value.to_boolean(),
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        let value = match self.get_property_value_completion(result, "value", context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        Ok(IteratorStepResult::Value { value, done })
    }

    fn await_value_now(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let promise = if let Some(promise) = context.promise_id_from_value(&value) {
            promise
        } else {
            let promise = context.create_promise()?;
            crate::builtins::promise::resolve_promise_id(self, context, promise, value)?;
            promise
        };
        self.drain_jobs(context)?;
        match context.promise_state(promise) {
            Some(crate::runtime::PromiseState::Fulfilled(value)) => {
                Ok(OperationResult::Value(value))
            }
            Some(crate::runtime::PromiseState::Rejected(value)) => {
                Ok(OperationResult::Throw(value))
            }
            Some(crate::runtime::PromiseState::Pending) => Err(VmError::runtime(
                "awaited Promise is still pending; async continuation is not available",
            )),
            None => Err(VmError::runtime("invalid Promise id")),
        }
    }

    pub(crate) fn await_value_from_builtin(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.await_value_now(value, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => Err(self.throw_value_from_builtin(value)),
        }
    }

    pub(crate) fn step_native_iterator_object(
        &mut self,
        iterator_val: JsValue,
        context: &mut NativeContext,
    ) -> Result<(JsValue, bool), VmError> {
        let id = match &iterator_val {
            JsValue::Object(id) => *id,
            _ => return Err(VmError::type_error("value is not an iterator object")),
        };
        let mut record = {
            let object = context
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &object.kind {
                ObjectKind::Iterator { record } => record.clone(),
                _ => return Err(VmError::type_error("object is not an iterator")),
            }
        };
        let result = self.native_iterator_next(&mut record, context)?;
        if let Some(object) = context.heap_mut().object_mut(id)
            && let ObjectKind::Iterator {
                record: stored_record,
            } = &mut object.kind
        {
            *stored_record = record;
        }
        match result {
            Some(value) => Ok((value, false)),
            None => Ok((JsValue::Undefined, true)),
        }
    }

    fn native_iterator_next(
        &mut self,
        iterator: &mut IteratorRecord,
        context: &mut NativeContext,
    ) -> Result<Option<JsValue>, VmError> {
        if iterator.done {
            return Ok(None);
        }
        match &mut iterator.kind {
            IteratorKind::Array {
                object,
                index,
                length,
                mode,
            } => {
                let current_length = self
                    .array_like_iterator_length(object.clone(), *length, context)?
                    .min(ITERATOR_MAX_ARRAY_LENGTH);
                if *index >= current_length {
                    iterator.done = true;
                    return Ok(None);
                }
                let current_index = *index;
                *index += 1;
                let key = JsValue::Number(current_index as f64);
                match mode {
                    crate::runtime::IteratorMode::Key => Ok(Some(key)),
                    crate::runtime::IteratorMode::Value => self
                        .iterator_property_value(object.clone(), current_index, context)
                        .map(Some),
                    crate::runtime::IteratorMode::KeyAndValue => {
                        let value =
                            self.iterator_property_value(object.clone(), current_index, context)?;
                        context.create_array(vec![key, value]).map(Some)
                    }
                }
            }
            IteratorKind::String { chars, index } => {
                if *index >= chars.len() {
                    iterator.done = true;
                    return Ok(None);
                }
                let value = JsValue::String(chars[*index].clone());
                *index += 1;
                Ok(Some(value))
            }
            IteratorKind::Js { .. } => Err(VmError::runtime(
                "JS iterator records must be advanced by the VM",
            )),
            IteratorKind::JsAsync { .. } => Err(VmError::runtime(
                "async JS iterator records must be advanced by the VM",
            )),
        }
    }

    fn array_like_iterator_length(
        &mut self,
        object: JsValue,
        fallback_length: usize,
        context: &mut NativeContext,
    ) -> Result<usize, VmError> {
        let Some(object_id) = context.value_object(&object) else {
            return Ok(fallback_length);
        };
        if let Some((view, _)) = context.typed_array_indexed_view(object_id) {
            return context.validate_typed_array_view(view);
        }
        if let Some(length) = context
            .heap()
            .object(object_id)
            .and_then(JsObject::array_length)
        {
            return Ok(length);
        }
        let length = self.iterator_property_value_by_key(object, "length", context)?;
        let number = self.to_number(length, context)?;
        if !number.is_finite() || number <= 0.0 {
            Ok(0)
        } else {
            Ok(number.floor().min(ITERATOR_MAX_ARRAY_LENGTH as f64) as usize)
        }
    }

    fn iterator_property_value(
        &mut self,
        object: JsValue,
        index: usize,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        self.iterator_property_value_by_key(object, &index.to_string(), context)
    }

    fn iterator_property_value_by_key(
        &mut self,
        object: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.get_property_value_completion(object, key, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("iterator property access threw"))
            }
        }
    }

    fn step_js_iterator(
        &mut self,
        iterator: JsValue,
        next_method: Option<JsValue>,
        context: &mut NativeContext,
    ) -> Result<IteratorStepResult, VmError> {
        let next = match next_method {
            Some(value) => value,
            None => match self.get_property_value_completion(iterator.clone(), "next", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
            },
        };
        if !is_callable_value(&next) {
            return Ok(IteratorStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator next is not callable"),
            )));
        }
        let result = match self.call_value(next, iterator, Vec::new(), context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        if !is_object_like(&result) {
            return Ok(IteratorStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator next returned a non-object"),
            )));
        }
        let done = match self.get_property_value_completion(result.clone(), "done", context)? {
            OperationResult::Value(value) => value.to_boolean(),
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        let value = match self.get_property_value_completion(result, "value", context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(IteratorStepResult::Throw(value)),
        };
        Ok(IteratorStepResult::Value { value, done })
    }

    fn step_yield_star_iterator(
        &mut self,
        iterator_val: JsValue,
        sent_value: JsValue,
        context: &mut NativeContext,
    ) -> Result<YieldStarStepResult, VmError> {
        let id = match &iterator_val {
            JsValue::Object(id) => *id,
            _ => {
                return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                    VmError::type_error("value is not an iterator object"),
                )));
            }
        };
        let kind = {
            let object = context
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &object.kind {
                ObjectKind::Iterator { record } => record.kind.clone(),
                _ => {
                    return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                        VmError::type_error("object is not an iterator"),
                    )));
                }
            }
        };

        match kind {
            IteratorKind::Array { .. } | IteratorKind::String { .. } => {
                let (value, done) = match self.step_native_iterator_object(iterator_val, context) {
                    Ok(result) => result,
                    Err(error) => match self.error_to_operation_result(error)? {
                        OperationResult::Throw(value) => {
                            return Ok(YieldStarStepResult::Throw(value));
                        }
                        OperationResult::Value(_) => unreachable!(),
                    },
                };
                if done {
                    Ok(YieldStarStepResult::Complete(value))
                } else {
                    generator_result(context, value, false).map(YieldStarStepResult::Yield)
                }
            }
            IteratorKind::Js {
                iterator,
                next_method,
            } => self.step_js_yield_star_iterator(iterator, next_method, sent_value, context),
            IteratorKind::JsAsync { iterator } => {
                self.step_js_async_yield_star_iterator(iterator, sent_value, context)
            }
        }
    }

    fn step_js_async_yield_star_iterator(
        &mut self,
        iterator: JsValue,
        sent_value: JsValue,
        context: &mut NativeContext,
    ) -> Result<YieldStarStepResult, VmError> {
        let next = match self.get_property_value_completion(iterator.clone(), "next", context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if !is_callable_value(&next) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator next is not callable"),
            )));
        }
        let result = match self.call_value(next, iterator, vec![sent_value], context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        let result = match self.await_value_now(result, context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if !is_object_like(&result) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("async iterator next returned a non-object"),
            )));
        }
        let done = match self.get_property_value_completion(result.clone(), "done", context)? {
            OperationResult::Value(value) => value.to_boolean(),
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if done {
            let value = match self.get_property_value_completion(result, "value", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
            match self.await_value_now(value, context)? {
                OperationResult::Value(value) => Ok(YieldStarStepResult::Complete(value)),
                OperationResult::Throw(value) => Ok(YieldStarStepResult::Throw(value)),
            }
        } else {
            let value = match self.get_property_value_completion(result, "value", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
            let value = match self.await_value_now(value, context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
            generator_result(context, value, false).map(YieldStarStepResult::Yield)
        }
    }

    fn step_js_yield_star_iterator(
        &mut self,
        iterator: JsValue,
        next_method: Option<JsValue>,
        sent_value: JsValue,
        context: &mut NativeContext,
    ) -> Result<YieldStarStepResult, VmError> {
        let next = match next_method {
            Some(value) => value,
            None => match self.get_property_value_completion(iterator.clone(), "next", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => {
                    return Ok(YieldStarStepResult::Throw(value));
                }
            },
        };
        if !is_callable_value(&next) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator next is not callable"),
            )));
        }
        let result = match self.call_value(next, iterator, vec![sent_value], context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if !is_object_like(&result) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator next returned a non-object"),
            )));
        }
        let done = match self.get_property_value_completion(result.clone(), "done", context)? {
            OperationResult::Value(value) => value.to_boolean(),
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if done {
            let value = match self.get_property_value_completion(result, "value", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
            Ok(YieldStarStepResult::Complete(value))
        } else {
            Ok(YieldStarStepResult::Yield(result))
        }
    }

    fn close_iterator_object_completion(
        &mut self,
        iterator_val: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let id = match iterator_val {
            JsValue::Object(id) => id,
            _ => return Ok(OperationResult::Value(JsValue::Undefined)),
        };
        let (kind, already_done) = {
            let object = context
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &object.kind {
                ObjectKind::Iterator { record } => (record.kind.clone(), record.done),
                _ => return Ok(OperationResult::Value(JsValue::Undefined)),
            }
        };

        context.close_iterator_object(JsValue::Object(id))?;

        // Per spec BindingInitialization §13.3.3.5: only call return() if the iterator
        // was NOT already exhausted naturally (iteratorRecord.[[Done]] was false before close).
        if already_done {
            return Ok(OperationResult::Value(JsValue::Undefined));
        }

        let iterator = match kind {
            IteratorKind::Js { iterator, .. } | IteratorKind::JsAsync { iterator } => iterator,
            _ => return Ok(OperationResult::Value(JsValue::Undefined)),
        };

        let return_method =
            match self.get_property_value_completion(iterator.clone(), "return", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
            };
        if matches!(return_method, JsValue::Undefined | JsValue::Null) {
            return Ok(OperationResult::Value(JsValue::Undefined));
        }
        if !is_callable_value(&return_method) {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("iterator return is not callable"),
            )));
        }
        let result = match self.call_value(return_method, iterator, Vec::new(), context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
        };
        if !is_object_like(&result) {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("iterator return returned a non-object"),
            )));
        }
        Ok(OperationResult::Value(JsValue::Undefined))
    }

    fn return_yield_star_delegate(
        &mut self,
        iterator_val: JsValue,
        return_value: JsValue,
        context: &mut NativeContext,
    ) -> Result<YieldStarStepResult, VmError> {
        let id = match iterator_val.clone() {
            JsValue::Object(id) => id,
            _ => return Ok(YieldStarStepResult::Complete(return_value)),
        };
        let kind = {
            let object = context
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &object.kind {
                ObjectKind::Iterator { record } => record.kind.clone(),
                _ => return Ok(YieldStarStepResult::Complete(return_value)),
            }
        };

        context.close_iterator_object(JsValue::Object(id))?;

        let (iterator, is_async) = match kind {
            IteratorKind::Js { iterator, .. } => (iterator, false),
            IteratorKind::JsAsync { iterator } => (iterator, true),
            _ => return Ok(YieldStarStepResult::Complete(return_value)),
        };

        let return_method =
            match self.get_property_value_completion(iterator.clone(), "return", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
        if matches!(return_method, JsValue::Undefined | JsValue::Null) {
            return Ok(YieldStarStepResult::Complete(return_value));
        }
        if !is_callable_value(&return_method) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator return is not callable"),
            )));
        }
        let result = match self.call_value(return_method, iterator, vec![return_value], context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        let result = if is_async {
            match self.await_value_now(result, context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            }
        } else {
            result
        };
        if !is_object_like(&result) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator return returned a non-object"),
            )));
        }
        let done = match self.get_property_value_completion(result.clone(), "done", context)? {
            OperationResult::Value(value) => value.to_boolean(),
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if done {
            let value = match self.get_property_value_completion(result, "value", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
            if is_async {
                match self.await_value_now(value, context)? {
                    OperationResult::Value(value) => Ok(YieldStarStepResult::Complete(value)),
                    OperationResult::Throw(value) => Ok(YieldStarStepResult::Throw(value)),
                }
            } else {
                Ok(YieldStarStepResult::Complete(value))
            }
        } else {
            if is_async {
                let value = match self.get_property_value_completion(result, "value", context)? {
                    OperationResult::Value(value) => value,
                    OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
                };
                let value = match self.await_value_now(value, context)? {
                    OperationResult::Value(value) => value,
                    OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
                };
                generator_result(context, value, false).map(YieldStarStepResult::Yield)
            } else {
                Ok(YieldStarStepResult::Yield(result))
            }
        }
    }

    fn throw_yield_star_delegate(
        &mut self,
        iterator_val: JsValue,
        thrown: JsValue,
        context: &mut NativeContext,
    ) -> Result<YieldStarStepResult, VmError> {
        let id = match iterator_val.clone() {
            JsValue::Object(id) => id,
            _ => {
                return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                    VmError::type_error("value is not an iterator object"),
                )));
            }
        };
        let kind = {
            let object = context
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &object.kind {
                ObjectKind::Iterator { record } => record.kind.clone(),
                _ => {
                    return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                        VmError::type_error("object is not an iterator"),
                    )));
                }
            }
        };

        let (iterator, is_async) = match kind {
            IteratorKind::Js { iterator, .. } => (iterator, false),
            IteratorKind::JsAsync { iterator } => (iterator, true),
            _ => {
                match self.close_iterator_object_completion(iterator_val, context)? {
                    OperationResult::Value(_) => {}
                    OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
                }
                return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                    VmError::type_error("iterator throw is not callable"),
                )));
            }
        };

        let throw_method =
            match self.get_property_value_completion(iterator.clone(), "throw", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
        if matches!(throw_method, JsValue::Undefined | JsValue::Null) {
            match self.close_iterator_object_completion(iterator_val, context)? {
                OperationResult::Value(_) => {}
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            }
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator throw is not callable"),
            )));
        }
        if !is_callable_value(&throw_method) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator throw is not callable"),
            )));
        }
        let result = match self.call_value(throw_method, iterator, vec![thrown], context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        let result = if is_async {
            match self.await_value_now(result, context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            }
        } else {
            result
        };
        if !is_object_like(&result) {
            return Ok(YieldStarStepResult::Throw(vm_error_to_value(
                VmError::type_error("iterator throw returned a non-object"),
            )));
        }
        let done = match self.get_property_value_completion(result.clone(), "done", context)? {
            OperationResult::Value(value) => value.to_boolean(),
            OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
        };
        if done {
            let value = match self.get_property_value_completion(result, "value", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
            };
            if is_async {
                match self.await_value_now(value, context)? {
                    OperationResult::Value(value) => Ok(YieldStarStepResult::Complete(value)),
                    OperationResult::Throw(value) => Ok(YieldStarStepResult::Throw(value)),
                }
            } else {
                Ok(YieldStarStepResult::Complete(value))
            }
        } else {
            if is_async {
                let value = match self.get_property_value_completion(result, "value", context)? {
                    OperationResult::Value(value) => value,
                    OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
                };
                let value = match self.await_value_now(value, context)? {
                    OperationResult::Value(value) => value,
                    OperationResult::Throw(value) => return Ok(YieldStarStepResult::Throw(value)),
                };
                generator_result(context, value, false).map(YieldStarStepResult::Yield)
            } else {
                Ok(YieldStarStepResult::Yield(result))
            }
        }
    }

    pub(crate) fn call_value_from_builtin(
        &mut self,
        callee: JsValue,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.call_value(callee, this_value, arguments, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("JavaScript callback threw"))
            }
        }
    }

    pub(crate) fn close_iterator_from_builtin(
        &mut self,
        iterator: JsValue,
        context: &mut NativeContext,
    ) -> Result<(), VmError> {
        self.close_iterator_from_builtin_with_throw(iterator, None, context)
    }

    pub(crate) fn close_iterator_preserving_throw_from_builtin(
        &mut self,
        iterator: JsValue,
        thrown: JsValue,
        context: &mut NativeContext,
    ) -> Result<(), VmError> {
        self.close_iterator_from_builtin_with_throw(iterator, Some(thrown), context)
    }

    fn close_iterator_from_builtin_with_throw(
        &mut self,
        iterator: JsValue,
        original_throw: Option<JsValue>,
        context: &mut NativeContext,
    ) -> Result<(), VmError> {
        let iterator = context
            .value_object(&iterator)
            .and_then(|object| context.heap().object(object))
            .and_then(|object| match &object.kind {
                ObjectKind::Iterator { record } => match &record.kind {
                    IteratorKind::Js { iterator, .. } | IteratorKind::JsAsync { iterator } => {
                        Some(iterator.clone())
                    }
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or(iterator);
        let return_method =
            match self.get_property_value_completion(iterator.clone(), "return", context)? {
                OperationResult::Value(value) => value,
                OperationResult::Throw(value) => {
                    self.pending_exception = original_throw.clone().or(Some(value));
                    return Err(VmError::runtime("iterator return getter threw"));
                }
            };
        if matches!(return_method, JsValue::Undefined | JsValue::Null) {
            if let Some(thrown) = original_throw {
                self.pending_exception = Some(thrown);
                return Err(VmError::runtime("iterator operation threw"));
            }
            return Ok(());
        }
        if !is_callable_value(&return_method) {
            if let Some(thrown) = original_throw {
                self.pending_exception = Some(thrown);
                return Err(VmError::runtime("iterator operation threw"));
            }
            return Err(VmError::type_error("iterator return is not callable"));
        }
        let inner_result = self.call_value(return_method, iterator, Vec::new(), context)?;
        if let Some(thrown) = original_throw {
            self.pending_exception = Some(thrown);
            return Err(VmError::runtime("iterator operation threw"));
        }
        match inner_result {
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("iterator return method threw"))
            }
            OperationResult::Value(value) if !is_object_like(&value) => {
                Err(VmError::type_error("iterator return returned a non-object"))
            }
            OperationResult::Value(_) => Ok(()),
        }
    }

    pub(crate) fn collect_iterable_values_from_builtin(
        &mut self,
        iterable: JsValue,
        context: &mut NativeContext,
    ) -> Result<Vec<JsValue>, VmError> {
        let iterator = match self.create_iterator_object(iterable, context)? {
            OperationResult::Value(iterator) => iterator,
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                return Err(VmError::runtime("GetIterator threw"));
            }
        };
        self.collect_iterator_values_from_builtin(iterator, context)
    }

    pub(crate) fn create_iterator_from_builtin(
        &mut self,
        iterable: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.create_iterator_object(iterable, context)? {
            OperationResult::Value(iterator) => Ok(iterator),
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("GetIterator threw"))
            }
        }
    }

    pub(crate) fn iterator_step_from_builtin(
        &mut self,
        iterator: JsValue,
        context: &mut NativeContext,
    ) -> Result<Option<JsValue>, VmError> {
        context.consume_loop_iteration()?;
        match self.step_iterator_object(iterator, context)? {
            IteratorStepResult::Value { done: true, .. } => Ok(None),
            IteratorStepResult::Value { value, done: false } => Ok(Some(value)),
            IteratorStepResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("IteratorNext threw"))
            }
        }
    }

    pub(crate) fn collect_iterator_values_from_builtin(
        &mut self,
        iterator: JsValue,
        context: &mut NativeContext,
    ) -> Result<Vec<JsValue>, VmError> {
        let iterator = if context.value_object(&iterator).is_some_and(|id| {
            context
                .heap()
                .object(id)
                .is_some_and(|object| matches!(object.kind, ObjectKind::Iterator { .. }))
        }) {
            iterator
        } else {
            self.wrap_js_iterator(iterator, context)?
        };
        let root_depth = self.stack.len();
        self.stack.push(iterator.clone());
        let result = (|| {
            let mut values = Vec::new();
            loop {
                context.consume_loop_iteration()?;
                match self.step_iterator_object(iterator.clone(), context)? {
                    IteratorStepResult::Value { done: true, .. } => return Ok(values),
                    IteratorStepResult::Value { value, done: false } => values.push(value),
                    IteratorStepResult::Throw(value) => {
                        self.pending_exception = Some(value);
                        return Err(VmError::runtime("IteratorNext threw"));
                    }
                }
            }
        })();
        self.stack.truncate(root_depth);
        result
    }

    fn wrap_js_iterator(
        &mut self,
        iterator: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        if !is_object_like(&iterator) {
            return Err(VmError::type_error("iterator method returned a non-object"));
        }
        let next = match self.get_property_value_completion(iterator.clone(), "next", context)? {
            OperationResult::Value(value) => value,
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                return Err(VmError::runtime("iterator next getter threw"));
            }
        };
        let object = JsObject::iterator(IteratorRecord::js_with_next(iterator, next));
        let id = context
            .heap_mut()
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime("heap full: cannot allocate iterator object"))?;
        Ok(JsValue::Object(id))
    }

    pub(crate) fn call_value_catching_from_builtin(
        &mut self,
        callee: JsValue,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<Result<JsValue, JsValue>, VmError> {
        match self.call_value(callee, this_value, arguments, context)? {
            OperationResult::Value(value) => Ok(Ok(value)),
            OperationResult::Throw(value) => Ok(Err(value)),
        }
    }

    pub(crate) fn construct_value_from_builtin(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.construct_value(constructor, arguments, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("JavaScript constructor threw"))
            }
        }
    }

    pub(crate) fn construct_value_from_builtin_with_new_target(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        new_target: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.construct_value_with_new_target(constructor, arguments, new_target, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => {
                self.pending_exception = Some(value);
                Err(VmError::runtime("JavaScript constructor threw"))
            }
        }
    }

    pub(crate) fn set_property_value_strict_from_builtin(
        &mut self,
        receiver: JsValue,
        key: &str,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<bool, VmError> {
        let object = context.require_object(&receiver, "write property")?;
        if let Some((_, descriptor)) = context.find_property_descriptor(object, key)? {
            match descriptor.kind {
                PropertyKind::Accessor {
                    set: Some(setter), ..
                } => match self.call_value(setter, receiver.clone(), vec![value], context)? {
                    OperationResult::Value(_) => return Ok(true),
                    OperationResult::Throw(thrown) => {
                        self.pending_exception = Some(thrown);
                        return Err(VmError::runtime("JavaScript setter threw"));
                    }
                },
                PropertyKind::Accessor { set: None, .. } => {
                    return Err(VmError::type_error("property setter is undefined"));
                }
                PropertyKind::Data {
                    writable: false, ..
                } => {
                    return Err(VmError::type_error("cannot write non-writable property"));
                }
                PropertyKind::Data { .. } => {}
            }
        }
        context.set(receiver, key, value, true)
    }

    pub(crate) fn get_property_value_with_receiver_from_builtin(
        &mut self,
        target: JsValue,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let object = self.property_lookup_object(&target, context)?;
        let Some((_, descriptor)) = context.find_property_descriptor(object, key)? else {
            return Ok(JsValue::Undefined);
        };
        match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(value),
            PropertyKind::Accessor { get: None, .. } => Ok(JsValue::Undefined),
            PropertyKind::Accessor {
                get: Some(getter), ..
            } => self.call_value_from_builtin(getter, receiver, Vec::new(), context),
        }
    }

    pub(crate) fn get_symbol_property_value_with_receiver_from_builtin(
        &mut self,
        target: JsValue,
        receiver: JsValue,
        symbol: SymbolId,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        if context.value_object(&target).is_some() {
            return proxy::internal_get(
                self,
                context,
                target,
                &PropertyKey::Symbol(symbol),
                receiver,
            );
        }
        let object = self.property_lookup_object(&target, context)?;
        let Some((_, descriptor)) = context.find_symbol_property_descriptor(object, symbol)? else {
            return Ok(JsValue::Undefined);
        };
        match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(value),
            PropertyKind::Accessor { get: None, .. } => Ok(JsValue::Undefined),
            PropertyKind::Accessor {
                get: Some(getter), ..
            } => self.call_value_from_builtin(getter, receiver, Vec::new(), context),
        }
    }

    pub(crate) fn set_symbol_property_value_with_receiver_from_builtin(
        &mut self,
        target: JsValue,
        receiver: JsValue,
        symbol: SymbolId,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<bool, VmError> {
        let target_object = context.require_object(&target, "Reflect.set")?;
        if let Some((_, descriptor)) =
            context.find_symbol_property_descriptor(target_object, symbol)?
        {
            match descriptor.kind {
                PropertyKind::Accessor {
                    set: Some(setter), ..
                } => {
                    let _ = self.call_value_from_builtin(setter, receiver, vec![value], context)?;
                    return Ok(true);
                }
                PropertyKind::Accessor { set: None, .. } => return Ok(false),
                PropertyKind::Data {
                    writable: false, ..
                } => return Ok(false),
                PropertyKind::Data { .. } => {}
            }
        }
        let Some(receiver_object) = context.value_object(&receiver) else {
            return Ok(false);
        };
        if let Some(current) = context.get_own_symbol_property_descriptor(receiver_object, symbol) {
            match current.kind {
                PropertyKind::Accessor { .. } => return Ok(false),
                PropertyKind::Data {
                    writable: false, ..
                } => return Ok(false),
                PropertyKind::Data { .. } => {}
            }
        }
        context.define_symbol_own_property(receiver_object, symbol, PropertyDescriptor::data(value))
    }

    pub(crate) fn to_property_key_from_builtin(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        // Symbols pass through unchanged (ToPropertyKey spec step 1).
        if matches!(value, JsValue::Symbol(_)) {
            return Ok(value);
        }
        let primitive = if is_object_like(&value) {
            self.to_primitive(value, PreferredType::String, context)?
        } else {
            value
        };
        if matches!(primitive, JsValue::Symbol(_)) {
            return Ok(primitive);
        }
        match to_property_key(&primitive)? {
            PropertyKey::String(key) => Ok(JsValue::String(key)),
            PropertyKey::Symbol(symbol) => Ok(JsValue::Symbol(symbol)),
        }
    }

    fn maybe_set_function_name(&mut self, inferred_name: &str, context: &mut NativeContext) {
        let Ok(func_val) = self.peek_value().cloned() else {
            return;
        };
        let obj_id_opt = match &func_val {
            JsValue::Function(func_id) => context.function_object(*func_id),
            JsValue::Object(obj_id) => Some(*obj_id),
            _ => None,
        };
        let Some(obj_id) = obj_id_opt else {
            return;
        };
        let should_set = match context.get_own_property(obj_id, "name") {
            None => true,
            Some(desc) => match &desc.kind {
                PropertyKind::Data { value, .. } => {
                    matches!(value, JsValue::String(s) if s.is_empty())
                }
                _ => false,
            },
        };
        if should_set && let Some(obj) = context.heap_mut().object_mut(obj_id) {
            obj.define_property(
                "name",
                PropertyDescriptor::data_with(
                    JsValue::String(inferred_name.to_string()),
                    false,
                    false,
                    true,
                ),
            );
        }
    }

    fn maybe_set_function_name_from_key(
        &mut self,
        func_val: &JsValue,
        key: &JsValue,
        prefix: Option<&str>,
        context: &mut NativeContext,
    ) {
        let inferred_name = match key {
            JsValue::String(name) => match prefix {
                Some(prefix) => format!("{prefix} {name}"),
                None => name.clone(),
            },
            JsValue::Symbol(symbol) => {
                let desc = context.symbol_description(*symbol).unwrap_or("");
                match prefix {
                    Some(prefix) if desc.is_empty() => format!("{prefix} "),
                    Some(prefix) => format!("{prefix} [{desc}]"),
                    None if desc.is_empty() => String::new(),
                    None => format!("[{desc}]"),
                }
            }
            _ => return,
        };
        let obj_id_opt = match func_val {
            JsValue::Function(func_id) => context.function_object(func_id.clone()),
            JsValue::Object(obj_id) => Some(obj_id.clone()),
            _ => None,
        };
        let Some(obj_id) = obj_id_opt else {
            return;
        };
        let should_set = match context.get_own_property(obj_id, "name") {
            None => true,
            Some(desc) => match &desc.kind {
                PropertyKind::Data { value, .. } => {
                    matches!(value, JsValue::String(s) if s.is_empty())
                }
                _ => false,
            },
        };
        if should_set && let Some(obj) = context.heap_mut().object_mut(obj_id) {
            obj.define_property(
                "name",
                PropertyDescriptor::data_with(JsValue::String(inferred_name), false, false, true),
            );
        }
    }

    fn define_computed_accessor(
        &mut self,
        object: ObjectId,
        key: JsValue,
        getter: Option<JsValue>,
        setter: Option<JsValue>,
        enumerable: bool,
        configurable: bool,
        context: &mut NativeContext,
    ) -> Result<(), VmError> {
        if let Some(value) = &getter {
            context.set_function_home_object(value, object)?;
            self.maybe_set_function_name_from_key(value, &key, Some("get"), context);
        }
        if let Some(value) = &setter {
            context.set_function_home_object(value, object)?;
            self.maybe_set_function_name_from_key(value, &key, Some("set"), context);
        }
        match self.to_property_key_from_builtin(key, context)? {
            JsValue::Symbol(symbol) => {
                let current = context.get_own_symbol_property_descriptor(object, symbol);
                let get = getter.or_else(|| {
                    current
                        .as_ref()
                        .and_then(|descriptor| match &descriptor.kind {
                            PropertyKind::Accessor { get, .. } => get.clone(),
                            PropertyKind::Data { .. } => None,
                        })
                });
                let set = setter.or_else(|| {
                    current
                        .as_ref()
                        .and_then(|descriptor| match &descriptor.kind {
                            PropertyKind::Accessor { set, .. } => set.clone(),
                            PropertyKind::Data { .. } => None,
                        })
                });
                context.define_symbol_own_property(
                    object,
                    symbol,
                    PropertyDescriptor::accessor(get, set, enumerable, configurable),
                )?;
            }
            JsValue::String(name) => {
                let get = getter.or_else(|| existing_accessor_getter(context, object, &name));
                let set = setter.or_else(|| existing_accessor_setter(context, object, &name));
                context.define_own_property(
                    object,
                    name,
                    PropertyDescriptor::accessor(get, set, enumerable, configurable),
                )?;
            }
            _ => unreachable!("ToPropertyKey returns a string or symbol"),
        }
        Ok(())
    }

    fn abstract_equals(
        &mut self,
        left: JsValue,
        right: JsValue,
        context: &mut NativeContext,
    ) -> Result<bool, VmError> {
        if same_ecmascript_type(&left, &right) {
            return Ok(left.strict_equals(&right));
        }
        if matches!(
            (&left, &right),
            (JsValue::Null, JsValue::Undefined) | (JsValue::Undefined, JsValue::Null)
        ) {
            return Ok(true);
        }
        match (&left, &right) {
            (JsValue::Number(left), JsValue::String(_)) => {
                return Ok(JsValue::Number(*left)
                    .strict_equals(&JsValue::Number(self.to_number(right, context)?)));
            }
            (JsValue::String(_), JsValue::Number(right)) => {
                return Ok(JsValue::Number(self.to_number(left, context)?)
                    .strict_equals(&JsValue::Number(*right)));
            }
            (JsValue::BigInt(left), JsValue::Number(right)) => {
                return Ok(bigint::number_equals_bigint(*right, left));
            }
            (JsValue::Number(left), JsValue::BigInt(right)) => {
                return Ok(bigint::number_equals_bigint(*left, right));
            }
            (JsValue::BigInt(left), JsValue::String(right)) => {
                return Ok(bigint::parse_bigint_string(right).is_some_and(|right| left == &right));
            }
            (JsValue::String(left), JsValue::BigInt(right)) => {
                return Ok(bigint::parse_bigint_string(left).is_some_and(|left| &left == right));
            }
            (JsValue::Boolean(_), _) => {
                let left = JsValue::Number(self.to_number(left, context)?);
                return self.abstract_equals(left, right, context);
            }
            (_, JsValue::Boolean(_)) => {
                let right = JsValue::Number(self.to_number(right, context)?);
                return self.abstract_equals(left, right, context);
            }
            _ => {}
        }

        if is_object_like(&left) && !is_object_like(&right) {
            let left = self.to_primitive(left, PreferredType::Default, context)?;
            return self.abstract_equals(left, right, context);
        }
        if !is_object_like(&left) && is_object_like(&right) {
            let right = self.to_primitive(right, PreferredType::Default, context)?;
            return self.abstract_equals(left, right, context);
        }
        Ok(false)
    }

    // ── ECMAScript abstract coercion operations ──────────────────────────────

    /// ECMAScript `ToPrimitive`. Returns `value` unchanged if it is already a
    /// primitive. For objects, first checks `Symbol.toPrimitive`, then falls
    /// back to `valueOf`/`toString` in the order dictated by `hint`.
    ///
    /// JavaScript exceptions raised by the conversion methods are stored in
    /// `pending_exception` and returned as `Err`, making them catchable by V5
    /// `try/catch` handlers.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_primitive(
        &mut self,
        value: JsValue,
        hint: PreferredType,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        if !matches!(
            value,
            JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)
        ) {
            return Ok(value);
        }

        // ECMAScript step 1: check @@toPrimitive.
        if let Some(object_id) = context.value_object(&value) {
            let to_primitive_sym = context.well_known_symbols().to_primitive;
            if let Some(method) = context.get_symbol_property_value(object_id, to_primitive_sym) {
                if matches!(method, JsValue::Undefined | JsValue::Null) {
                    // GetMethod treats null/undefined as absent.
                } else if !matches!(method, JsValue::Function(_) | JsValue::BuiltinFunction(_)) {
                    return Err(VmError::type_error(
                        "Symbol.toPrimitive method is not callable",
                    ));
                } else {
                    let hint_str = match hint {
                        PreferredType::Default => "default",
                        PreferredType::Number => "number",
                        PreferredType::String => "string",
                    };
                    let result = match self.call_value(
                        method,
                        value,
                        vec![JsValue::String(hint_str.into())],
                        context,
                    )? {
                        OperationResult::Value(v) => v,
                        OperationResult::Throw(thrown) => {
                            self.pending_exception = Some(thrown);
                            return Err(VmError::runtime(
                                "Symbol.toPrimitive method threw an exception",
                            ));
                        }
                    };
                    // ECMAScript: if the result is not primitive, throw TypeError.
                    if matches!(
                        result,
                        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)
                    ) {
                        return Err(VmError::type_error(
                            "Symbol.toPrimitive must return a primitive",
                        ));
                    }
                    return Ok(result);
                }
            }
        }

        // ECMAScript step 2: ordinary ToPrimitive via valueOf/toString.
        let (first, second) = match hint {
            PreferredType::String => ("toString", "valueOf"),
            PreferredType::Default | PreferredType::Number => ("valueOf", "toString"),
        };

        if let Some(prim) = self.try_coerce_method(value.clone(), first, context)? {
            return Ok(prim);
        }
        if let Some(prim) = self.try_coerce_method(value.clone(), second, context)? {
            return Ok(prim);
        }
        Err(VmError::type_error(
            "Cannot convert object to primitive value",
        ))
    }

    /// Try one conversion method (`valueOf` or `toString`). Returns:
    /// - `Ok(Some(prim))` if the method exists, is callable, and returned a primitive.
    /// - `Ok(None)` if the method is absent, not callable, or returned an object.
    /// - `Err` if the method threw a JavaScript exception (pending_exception is set).
    fn try_coerce_method(
        &mut self,
        value: JsValue,
        method: &str,
        context: &mut NativeContext,
    ) -> Result<Option<JsValue>, VmError> {
        let method_fn = match self.get_property_value_completion(value.clone(), method, context)? {
            OperationResult::Value(v) => v,
            OperationResult::Throw(thrown) => {
                self.pending_exception = Some(thrown);
                return Err(VmError::runtime("JavaScript callback threw"));
            }
        };
        if !matches!(
            method_fn,
            JsValue::Function(_) | JsValue::BuiltinFunction(_)
        ) {
            return Ok(None);
        }
        let result = match self.call_value(method_fn, value, vec![], context)? {
            OperationResult::Value(v) => v,
            OperationResult::Throw(thrown) => {
                self.pending_exception = Some(thrown);
                return Err(VmError::runtime("JavaScript callback threw"));
            }
        };
        // Only accept primitive results.
        if matches!(
            result,
            JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)
        ) {
            return Ok(None);
        }
        Ok(Some(result))
    }

    /// ECMAScript `ToNumber`. For objects, applies `ToPrimitive(Number)` first.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_number(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<f64, VmError> {
        match value {
            JsValue::Undefined => Ok(f64::NAN),
            JsValue::Null => Ok(0.0),
            JsValue::Boolean(b) => Ok(if b { 1.0 } else { 0.0 }),
            JsValue::Number(n) => Ok(n),
            JsValue::String(ref s) => Ok(coerce_string_to_number(s)),
            JsValue::BigInt(_) => Err(VmError::type_error(
                "Cannot convert a BigInt value to a number",
            )),
            // Symbols cannot be converted to numbers — ECMAScript raises a TypeError.
            JsValue::Symbol(_) => Err(VmError::type_error(
                "Cannot convert a Symbol value to a number",
            )),
            JsValue::BuiltinFunction(_) => Ok(f64::NAN),
            JsValue::Object(_) | JsValue::Function(_) => {
                let prim = self.to_primitive(value, PreferredType::Number, context)?;
                self.to_number(prim, context)
            }
            JsValue::Error(_) => Ok(f64::NAN),
        }
    }

    pub(crate) fn to_numeric(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let primitive = self.to_primitive(value, PreferredType::Number, context)?;
        if matches!(primitive, JsValue::BigInt(_)) {
            return Ok(primitive);
        }
        self.to_number(primitive, context).map(JsValue::Number)
    }

    fn to_numeric_operands(
        &mut self,
        left: JsValue,
        right: JsValue,
        context: &mut NativeContext,
    ) -> Result<(JsValue, JsValue), VmError> {
        let left = self.to_numeric(left, context)?;
        let right = self.to_numeric(right, context)?;
        Ok((left, right))
    }

    fn execute_numeric_binary_slow(
        &mut self,
        left: JsValue,
        right: JsValue,
        context: &mut NativeContext,
        bigint_operation: impl FnOnce(
            &crate::runtime::BigIntValue,
            &crate::runtime::BigIntValue,
        ) -> Result<crate::runtime::BigIntValue, VmError>,
        number_operation: impl FnOnce(f64, f64) -> f64,
    ) -> Result<(), VmError> {
        let (left, right) = self.to_numeric_operands(left, right, context)?;
        match bigint_binary(left.clone(), right.clone(), bigint_operation)? {
            Some(value) => self.stack.push(value),
            None => {
                let (left, right) = numeric_number_pair(left, right)?;
                self.stack
                    .push(JsValue::Number(number_operation(left, right)));
            }
        }
        Ok(())
    }

    fn throw_value_from_error(&mut self, error: VmError) -> JsValue {
        self.pending_exception
            .take()
            .unwrap_or_else(|| vm_error_to_value(error))
    }

    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_int32(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<i32, VmError> {
        let n = self.to_number(value, context)?;
        if n.is_nan() || n.is_infinite() || n == 0.0 {
            return Ok(0);
        }
        Ok(n.trunc() as i64 as i32)
    }

    /// ECMAScript `ToString`. For objects, applies `ToPrimitive(String)` first.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_string_coerce(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<String, VmError> {
        match value {
            JsValue::Undefined => Ok("undefined".into()),
            JsValue::Null => Ok("null".into()),
            JsValue::Boolean(b) => Ok(b.to_string()),
            JsValue::Number(n) => Ok(coerce_number_to_string(n)),
            JsValue::BigInt(n) => Ok(n.to_string()),
            JsValue::String(s) => Ok(s),
            // Symbols cannot be implicitly converted to strings — TypeError.
            JsValue::Symbol(_) => Err(VmError::type_error(
                "Cannot convert a Symbol value to a string",
            )),
            JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
                let prim = self.to_primitive(value, PreferredType::String, context)?;
                self.to_string_coerce(prim, context)
            }
            JsValue::Error(e) => Ok(e.message),
        }
    }

    /// ECMAScript `ToObject`. Wraps primitives in their corresponding wrapper objects.
    /// Fails with `TypeError` for `null` and `undefined`.
    #[allow(clippy::wrong_self_convention, dead_code)]
    pub(crate) fn to_object(
        &mut self,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<ObjectId, VmError> {
        match value {
            JsValue::Null | JsValue::Undefined => Err(VmError::type_error(
                "Cannot convert undefined or null to object",
            )),
            JsValue::Symbol(symbol) => {
                let proto = context
                    .get_global("Symbol")
                    .and_then(|ctor| context.value_object(&ctor))
                    .and_then(|ctor_obj| {
                        context
                            .find_property_descriptor(ctor_obj, "prototype")
                            .ok()
                            .flatten()
                            .and_then(|(_, descriptor)| descriptor.value_cloned())
                            .and_then(|value| context.value_object(&value))
                    })
                    .ok_or_else(|| VmError::runtime("Symbol prototype not installed"))?;
                let wrapper =
                    context.create_primitive_wrapper(PrimitiveValue::Symbol(symbol), proto)?;
                context.require_object(&wrapper, "ToObject")
            }
            JsValue::Boolean(b) => {
                let proto = context
                    .boolean_prototype()
                    .ok_or_else(|| VmError::runtime("Boolean prototype not installed"))?;
                let wrapper =
                    context.create_primitive_wrapper(PrimitiveValue::Boolean(b), proto)?;
                context.require_object(&wrapper, "ToObject")
            }
            JsValue::Number(n) => {
                let proto = context
                    .number_prototype()
                    .ok_or_else(|| VmError::runtime("Number prototype not installed"))?;
                let wrapper = context.create_primitive_wrapper(PrimitiveValue::Number(n), proto)?;
                context.require_object(&wrapper, "ToObject")
            }
            JsValue::BigInt(n) => {
                let proto = context
                    .get_global("BigInt")
                    .and_then(|ctor| context.value_object(&ctor))
                    .and_then(|ctor_obj| {
                        context
                            .find_property_descriptor(ctor_obj, "prototype")
                            .ok()
                            .flatten()
                            .and_then(|(_, descriptor)| descriptor.value_cloned())
                            .and_then(|value| context.value_object(&value))
                    })
                    .ok_or_else(|| VmError::runtime("BigInt prototype not installed"))?;
                let wrapper = context.create_primitive_wrapper(PrimitiveValue::BigInt(n), proto)?;
                context.require_object(&wrapper, "ToObject")
            }
            JsValue::String(s) => {
                let proto = context
                    .string_prototype()
                    .ok_or_else(|| VmError::runtime("String prototype not installed"))?;
                let wrapper = context.create_primitive_wrapper(PrimitiveValue::String(s), proto)?;
                context.require_object(&wrapper, "ToObject")
            }
            JsValue::Object(id) => Ok(id),
            JsValue::Function(id) => context
                .function_object(id)
                .ok_or_else(|| VmError::runtime("missing function object")),
            JsValue::BuiltinFunction(id) => context
                .builtin(id)
                .map(|b| b.object)
                .ok_or_else(|| VmError::runtime("invalid builtin id")),
            JsValue::Error(_) => Err(VmError::type_error("Cannot convert Error to object")),
        }
    }

    fn copy_data_properties_excluded(
        &mut self,
        source: JsValue,
        excluded: &[PropertyKey],
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let source_object = match self.to_object(source.clone(), context) {
            Ok(object) => object,
            Err(error) => return self.error_to_operation_result(error),
        };
        let source_value = if context.value_object(&source).is_some() {
            source.clone()
        } else {
            context.object_value(source_object)
        };
        let target = context.create_object(std::iter::empty::<(String, JsValue)>())?;
        let target_object = context.require_object(&target, "object rest target")?;
        let keys = match proxy::internal_own_property_keys(self, context, source_value.clone()) {
            Ok(keys) => keys,
            Err(error) => return self.error_to_operation_result(error),
        };

        for key in keys {
            if excluded.contains(&key) {
                continue;
            }
            let descriptor =
                match proxy::internal_get_own_property(self, context, source_value.clone(), &key) {
                    Ok(descriptor) => descriptor,
                    Err(error) => return self.error_to_operation_result(error),
                };
            let Some(descriptor) = descriptor else {
                continue;
            };
            if !descriptor.enumerable {
                continue;
            }
            let value = match proxy::internal_get(
                self,
                context,
                source_value.clone(),
                &key,
                source_value.clone(),
            ) {
                Ok(value) => value,
                Err(error) => return self.error_to_operation_result(error),
            };
            match key {
                PropertyKey::String(name) => {
                    context.define_own_property(
                        target_object,
                        name,
                        PropertyDescriptor::data(value),
                    )?;
                }
                PropertyKey::Symbol(symbol) => {
                    context.define_symbol_own_property(
                        target_object,
                        symbol,
                        PropertyDescriptor::data(value),
                    )?;
                }
            }
        }

        Ok(OperationResult::Value(target))
    }

    pub(crate) fn get_property_value(
        &mut self,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.get_property_value_completion(receiver, key, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => Err(throw_value(value, context)),
        }
    }

    pub(crate) fn get_property_value_catching_from_builtin(
        &mut self,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<Result<JsValue, JsValue>, VmError> {
        match self.get_property_value_completion(receiver, key, context)? {
            OperationResult::Value(value) => Ok(Ok(value)),
            OperationResult::Throw(value) => Ok(Err(value)),
        }
    }

    /// Fast path: numeric key on Array/TypedArray without string conversion.
    /// Returns `Some(value)` if the fast path succeeded, `None` to fall through.
    fn fast_get_element(
        obj_id: ObjectId,
        idx: usize,
        context: &NativeContext,
    ) -> Option<Result<JsValue, VmError>> {
        enum FastKind {
            ArrayValue(Option<JsValue>),
            TypedArray(TypedArrayViewId),
        }
        let fast_kind = {
            let obj = context.heap().object(obj_id)?;
            match &obj.kind {
                ObjectKind::Array { .. } => FastKind::ArrayValue(obj.array_element_value(idx)),
                ObjectKind::TypedArray { view, .. } => FastKind::TypedArray(*view),
                _ => return None,
            }
        };
        match fast_kind {
            FastKind::ArrayValue(Some(v)) => Some(Ok(v)),
            FastKind::ArrayValue(None) => None, // hole or out-of-bounds: fall through to prototype chain
            FastKind::TypedArray(view_id) => {
                Some(context.typed_array_load_element(view_id, idx).or_else(|e| {
                    if matches!(e.kind, VmErrorKind::Range) {
                        Ok(JsValue::Undefined)
                    } else {
                        Err(e)
                    }
                }))
            }
        }
    }

    /// Fast path: numeric key write on Array/TypedArray without string conversion.
    /// Returns `Some(Ok(()))` on success, `Some(Err(...))` on fatal error, `None` to fall through.
    fn fast_set_element(
        obj_id: ObjectId,
        idx: usize,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Option<Result<(), VmError>> {
        // Extract what we need in a short borrow so we can call mutable methods after.
        enum FastKind {
            Array,
            TypedArray(TypedArrayViewId),
        }
        let fast_kind = {
            let obj = context.heap().object(obj_id)?;
            match &obj.kind {
                ObjectKind::Array { .. } => FastKind::Array,
                ObjectKind::TypedArray { view, .. } => FastKind::TypedArray(*view),
                _ => return None,
            }
        };
        match fast_kind {
            FastKind::TypedArray(view_id) => Some(
                context
                    .typed_array_store_element(view_id, idx, value)
                    .or_else(|e| {
                        if matches!(e.kind, VmErrorKind::Range) {
                            Ok(())
                        } else {
                            Err(e)
                        }
                    }),
            ),
            FastKind::Array => {
                // Keep very large indices on the full property path for range checks.
                if idx >= 1_000_000 {
                    return None;
                }
                let desc = crate::runtime::PropertyDescriptor::data_with(value, true, true, true);
                match context.define_own_element(obj_id, idx, desc) {
                    Ok(true) => Some(Ok(())),
                    Ok(false) => None, // e.g. non-writable length; fall through
                    Err(e) => Some(Err(e)),
                }
            }
        }
    }

    fn error_to_operation_result(&mut self, error: VmError) -> Result<OperationResult, VmError> {
        if let Some(value) = self.pending_exception.take() {
            return Ok(OperationResult::Throw(value));
        }
        if matches!(
            error.kind,
            VmErrorKind::Reference
                | VmErrorKind::Type
                | VmErrorKind::Syntax
                | VmErrorKind::Range
                | VmErrorKind::Test262
        ) {
            Ok(OperationResult::Throw(vm_error_to_value(error)))
        } else {
            Err(error)
        }
    }

    fn error_to_operation_result_in_context(
        &mut self,
        error: VmError,
        context: &NativeContext,
    ) -> Result<OperationResult, VmError> {
        if let Some(value) = self.pending_exception.take() {
            return Ok(OperationResult::Throw(value));
        }
        if matches!(
            error.kind,
            VmErrorKind::Reference
                | VmErrorKind::Type
                | VmErrorKind::Syntax
                | VmErrorKind::Range
                | VmErrorKind::Test262
        ) {
            Ok(OperationResult::Throw(vm_error_to_value_with_realm(
                error,
                context.global_object(),
            )))
        } else {
            Err(error)
        }
    }

    /// `ToPropertyKey` through the shared `ToPrimitive` implementation.
    /// Symbols remain symbols; every other primitive becomes a string key.
    /// Returns a string or symbol key on success,
    /// or `Ok(OperationResult::Throw(...))` when conversion fails.
    fn coerce_to_property_key(
        &mut self,
        key: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        match self.to_property_key_from_builtin(key, context) {
            Ok(key) => Ok(OperationResult::Value(key)),
            Err(error) => self.error_to_operation_result(error),
        }
    }

    fn get_property_value_completion(
        &mut self,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        // null/undefined property access must be a catchable TypeError so that
        // try-catch blocks in userland (and assert.throws) can intercept it.
        match &receiver {
            JsValue::Null => {
                return Ok(OperationResult::Throw(vm_error_to_value(
                    VmError::type_error(format!(
                        "Cannot read properties of null (reading '{key}')"
                    )),
                )));
            }
            JsValue::Undefined => {
                return Ok(OperationResult::Throw(vm_error_to_value(
                    VmError::type_error(format!(
                        "Cannot read properties of undefined (reading '{key}')"
                    )),
                )));
            }
            _ => {}
        }
        if let JsValue::Error(error) = &receiver {
            let value = match key {
                "message" => JsValue::String(error.message.clone()),
                "name" => JsValue::String(native_error_constructor_name(&error.kind).into()),
                "constructor" => context.error_constructor_value(error),
                "stack" | "cause" => JsValue::Undefined,
                _ => JsValue::Undefined,
            };
            return Ok(OperationResult::Value(value));
        }

        if let Some(name) = key.strip_prefix("\0#")
            && let Some(brand) = Self::private_brand_for_name(context, name)?
        {
            let object = match context.require_object(&receiver, "read private field") {
                Ok(object) => object,
                Err(error) => return self.error_to_operation_result(error),
            };
            return match context.get_private_slot_with_brand(object, name, brand) {
                Ok(value) => Ok(OperationResult::Value(value)),
                Err(error) => self.error_to_operation_result(error),
            };
        }

        if key.starts_with("\0#") {
            let object = match context.require_object(&receiver, "read private field") {
                Ok(object) => object,
                Err(error) => return self.error_to_operation_result(error),
            };
            match context.has_property(object, key) {
                Ok(true) => {}
                Ok(false) => {
                    return self.error_to_operation_result(VmError::type_error(
                        "private member is not declared on this receiver",
                    ));
                }
                Err(error) => return self.error_to_operation_result(error),
            }
        }

        if context.value_object(&receiver).is_some() {
            let key = PropertyKey::String(key.to_string());
            return match proxy::internal_get(
                self,
                context,
                receiver.clone(),
                &key,
                receiver.clone(),
            ) {
                Ok(value) => Ok(OperationResult::Value(value)),
                Err(error) => self.error_to_operation_result(error),
            };
        }

        // Primitive strings expose `length` and UTF-16 code-unit indexing
        // without boxing; method lookups fall through to String.prototype.
        if let JsValue::String(value) = &receiver {
            if key == "length" {
                return Ok(OperationResult::Value(JsValue::Number(
                    string::utf16_length(value) as f64,
                )));
            }
            if let Some(index) = array_index(key) {
                return Ok(OperationResult::Value(
                    string::utf16_code_unit_at(value, index).map_or(JsValue::Undefined, |unit| {
                        JsValue::String(string::decode_utf16(&[unit]))
                    }),
                ));
            }
        }
        // Primitive String/Number/Boolean receivers resolve property lookups on
        // their wrapper prototype while keeping the primitive as the `this`
        // value passed to any accessor.
        let object = self.property_lookup_object(&receiver, context)?;
        let Some((_, descriptor)) = context.find_property_descriptor(object, key)? else {
            return Ok(OperationResult::Value(JsValue::Undefined));
        };
        match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(OperationResult::Value(value)),
            PropertyKind::Accessor { get: None, .. } => {
                Ok(OperationResult::Value(JsValue::Undefined))
            }
            PropertyKind::Accessor {
                get: Some(getter), ..
            } => self.call_value(getter, receiver, Vec::new(), context),
        }
    }

    fn get_symbol_property_value_completion(
        &mut self,
        receiver: JsValue,
        symbol: SymbolId,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        if context.value_object(&receiver).is_some() {
            let key = PropertyKey::Symbol(symbol);
            return match proxy::internal_get(
                self,
                context,
                receiver.clone(),
                &key,
                receiver.clone(),
            ) {
                Ok(value) => Ok(OperationResult::Value(value)),
                Err(error) => self.error_to_operation_result(error),
            };
        }

        match &receiver {
            JsValue::Null => {
                return Ok(OperationResult::Throw(vm_error_to_value(
                    VmError::type_error("Cannot read properties of null (reading symbol)"),
                )));
            }
            JsValue::Undefined => {
                return Ok(OperationResult::Throw(vm_error_to_value(
                    VmError::type_error("Cannot read properties of undefined (reading symbol)"),
                )));
            }
            _ => {}
        }
        let object = self.property_lookup_object(&receiver, context)?;
        let Some((_, descriptor)) = context.find_symbol_property_descriptor(object, symbol)? else {
            return Ok(OperationResult::Value(JsValue::Undefined));
        };
        match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(OperationResult::Value(value)),
            PropertyKind::Accessor { get: None, .. } => {
                Ok(OperationResult::Value(JsValue::Undefined))
            }
            PropertyKind::Accessor {
                get: Some(getter), ..
            } => self.call_value(getter, receiver, Vec::new(), context),
        }
    }

    fn get_super_property_value(
        &mut self,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let Some(function) = context.current_function() else {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::reference("super property access outside function"),
            )));
        };
        let Some(home_object) = context.function(function).and_then(|f| f.home_object) else {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::reference("super property access without home object"),
            )));
        };
        let Some(super_base) = context.get_prototype_of(home_object) else {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("super base is null"),
            )));
        };
        let receiver = context.current_or_global_this();
        let property_key = PropertyKey::String(key.to_string());
        match proxy::internal_get(
            self,
            context,
            JsValue::Object(super_base),
            &property_key,
            receiver,
        ) {
            Ok(value) => Ok(OperationResult::Value(value)),
            Err(error) => self.error_to_operation_result(error),
        }
    }

    fn get_super_symbol_property_value(
        &mut self,
        symbol: crate::runtime::SymbolId,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let Some(function) = context.current_function() else {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::reference("super property access outside function"),
            )));
        };
        let Some(home_object) = context.function(function).and_then(|f| f.home_object) else {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::reference("super property access without home object"),
            )));
        };
        let Some(super_base) = context.get_prototype_of(home_object) else {
            return Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("super base is null"),
            )));
        };
        let receiver = context.current_or_global_this();
        let property_key = PropertyKey::Symbol(symbol);
        match proxy::internal_get(
            self,
            context,
            JsValue::Object(super_base),
            &property_key,
            receiver,
        ) {
            Ok(value) => Ok(OperationResult::Value(value)),
            Err(error) => self.error_to_operation_result(error),
        }
    }

    fn type_of_value(&self, value: &JsValue, context: &NativeContext) -> &'static str {
        if context.is_callable_value(value) {
            "function"
        } else {
            value.type_of()
        }
    }

    /// Resolves the object whose property table backs a receiver's reads. For
    /// the primitive wrapper types this is the corresponding intrinsic
    /// prototype, so `"abc".charAt` and `(5).toFixed` find their methods.
    fn property_lookup_object(
        &self,
        receiver: &JsValue,
        context: &NativeContext,
    ) -> Result<ObjectId, VmError> {
        match receiver {
            JsValue::String(_) => context
                .string_prototype()
                .ok_or_else(|| VmError::type_error("cannot read property on string")),
            JsValue::Number(_) => context
                .number_prototype()
                .ok_or_else(|| VmError::type_error("cannot read property on number")),
            JsValue::BigInt(_) => context
                .get_global("BigInt")
                .and_then(|ctor| context.value_object(&ctor))
                .and_then(|ctor_obj| {
                    context
                        .find_property_descriptor(ctor_obj, "prototype")
                        .ok()
                        .flatten()
                        .and_then(|(_, d)| d.value_cloned())
                        .and_then(|v| context.value_object(&v))
                })
                .ok_or_else(|| VmError::type_error("cannot read property on bigint")),
            JsValue::Boolean(_) => context
                .boolean_prototype()
                .ok_or_else(|| VmError::type_error("cannot read property on boolean")),
            JsValue::Symbol(_) => context
                .get_global("Symbol")
                .and_then(|ctor| context.value_object(&ctor))
                .and_then(|ctor_obj| {
                    context
                        .find_property_descriptor(ctor_obj, "prototype")
                        .ok()
                        .flatten()
                        .and_then(|(_, d)| d.value_cloned())
                        .and_then(|v| context.value_object(&v))
                })
                .ok_or_else(|| VmError::type_error("cannot read property on symbol")),
            JsValue::Error(error) => {
                // Resolve the prototype of the corresponding error constructor so that
                // property reads like `thrown.constructor` and `thrown.message` work.
                let name = native_error_constructor_name(&error.kind);
                context
                    .get_global(name)
                    .and_then(|ctor| context.value_object(&ctor))
                    .and_then(|ctor_obj| {
                        context
                            .find_property_descriptor(ctor_obj, "prototype")
                            .ok()
                            .flatten()
                            .and_then(|(_, d)| d.value_cloned())
                            .and_then(|v| context.value_object(&v))
                    })
                    .ok_or_else(|| {
                        VmError::type_error(format!("cannot read property on {receiver}"))
                    })
            }
            _ => context.require_object(receiver, "read property"),
        }
    }

    fn set_property_value(
        &mut self,
        receiver: JsValue,
        key: &str,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let object = match context.require_object(&receiver, "write property") {
            Ok(object) => object,
            Err(error)
                if matches!(
                    error.kind,
                    VmErrorKind::Type | VmErrorKind::Range | VmErrorKind::Reference
                ) =>
            {
                return Ok(OperationResult::Throw(vm_error_to_value(error)));
            }
            Err(error) => return Err(error),
        };
        if let Some(name) = key.strip_prefix("\0#fieldinit#") {
            let update = PropertyDescriptorUpdate {
                value: Some(value.clone()),
                writable: Some(true),
                enumerable: Some(true),
                configurable: Some(true),
                get: None,
                set: None,
            };
            let descriptor = proxy::descriptor_object_from_update(context, &update)?;
            let key = PropertyKey::String(name.to_string());
            return match proxy::internal_define_own_property(
                self, context, receiver, &key, descriptor, update,
            ) {
                Ok(true) => Ok(OperationResult::Value(value)),
                Ok(false) => self.error_to_operation_result(VmError::type_error(format!(
                    "cannot define class field {name}"
                ))),
                Err(error) => self.error_to_operation_result(error),
            };
        }
        if let Some(name) = key.strip_prefix("\0#init#") {
            if let Some(brand) = Self::private_brand_for_name(context, name)? {
                return match context.define_private_slot(
                    object,
                    name.to_string(),
                    brand,
                    value.clone(),
                ) {
                    Ok(()) => Ok(OperationResult::Value(value)),
                    Err(error) => self.error_to_operation_result(error),
                };
            }
            return self.error_to_operation_result(VmError::reference(format!(
                "private name #{name} is not in scope"
            )));
        }
        if let Some(name) = key.strip_prefix("\0#")
            && let Some(brand) = Self::private_brand_for_name(context, name)?
        {
            return match context.set_private_slot_with_brand(object, name, brand, value.clone()) {
                Ok(()) => Ok(OperationResult::Value(value)),
                Err(error) => self.error_to_operation_result(error),
            };
        }
        if key.starts_with("\0#") {
            match context.has_property(object, key) {
                Ok(true) => {}
                Ok(false) => {
                    return self.error_to_operation_result(VmError::type_error(
                        "private member is not declared on this receiver",
                    ));
                }
                Err(error) => return self.error_to_operation_result(error),
            }
        }
        let property_key = PropertyKey::String(key.to_string());
        match proxy::internal_set(
            self,
            context,
            receiver.clone(),
            &property_key,
            value.clone(),
            receiver,
        ) {
            Ok(true) => Ok(OperationResult::Value(value)),
            Ok(false) if context.is_strict_code() => Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("cannot write property"),
            ))),
            Ok(false) => Ok(OperationResult::Value(value)),
            Err(error) => self.error_to_operation_result(error),
        }
    }

    fn private_brand_for_name(
        context: &NativeContext,
        name: &str,
    ) -> Result<Option<PrivateBrandId>, VmError> {
        let binding = format!("\0#brand#{name}");
        let Some((_, value)) = context.resolve_binding_value(&binding)? else {
            return Ok(None);
        };
        let JsValue::Number(value) = value else {
            return Err(VmError::runtime("invalid private brand binding"));
        };
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u32::MAX as f64 {
            return Err(VmError::runtime("invalid private brand identity"));
        }
        Ok(Some(PrivateBrandId(value as u32)))
    }

    fn set_symbol_property_value(
        &mut self,
        receiver: JsValue,
        symbol: SymbolId,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        match context.require_object(&receiver, "write property") {
            Ok(object) => object,
            Err(error)
                if matches!(
                    error.kind,
                    VmErrorKind::Type | VmErrorKind::Range | VmErrorKind::Reference
                ) =>
            {
                return Ok(OperationResult::Throw(vm_error_to_value(error)));
            }
            Err(error) => return Err(error),
        };
        let property_key = PropertyKey::Symbol(symbol);
        match proxy::internal_set(
            self,
            context,
            receiver.clone(),
            &property_key,
            value.clone(),
            receiver,
        ) {
            Ok(true) => Ok(OperationResult::Value(value)),
            Ok(false) if context.is_strict_code() => Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error("cannot write property"),
            ))),
            Ok(false) => Ok(OperationResult::Value(value)),
            Err(error) => self.error_to_operation_result(error),
        }
    }

    fn set_element_value(
        &mut self,
        object: JsValue,
        key: JsValue,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        if let (JsValue::Number(index), JsValue::Object(object_id)) = (&key, &object)
            && index.fract() == 0.0
            && *index >= 0.0
            && *index < 4_294_967_295.0
            && let Some(result) =
                Self::fast_set_element(*object_id, *index as usize, value.clone(), context)
        {
            result?;
            return Ok(OperationResult::Value(value));
        }

        let key = match self.coerce_to_property_key(key, context)? {
            OperationResult::Value(key) => key,
            OperationResult::Throw(value) => return Ok(OperationResult::Throw(value)),
        };
        match key {
            JsValue::String(key) => self.set_property_value(object, &key, value, context),
            JsValue::Symbol(symbol) => {
                self.set_symbol_property_value(object, symbol, value, context)
            }
            _ => unreachable!("ToPropertyKey returns a string or symbol"),
        }
    }

    fn call_user_function(
        &mut self,
        function_id: FunctionId,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        new_target: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let function = context
            .function(function_id)
            .cloned()
            .ok_or_else(|| VmError::runtime("missing function value"))?;
        let this_value = if function.is_arrow {
            function
                .lexical_this
                .clone()
                .unwrap_or_else(|| context.global_this_value())
        } else if context.is_strict_function(function_id) {
            this_value
        } else {
            match this_value {
                JsValue::Undefined | JsValue::Null => context.global_this_value(),
                JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
                    this_value
                }
                primitive => match self.to_object(primitive, context) {
                    Ok(object) => JsValue::Object(object),
                    Err(error)
                        if matches!(
                            error.kind,
                            VmErrorKind::Reference
                                | VmErrorKind::Type
                                | VmErrorKind::Syntax
                                | VmErrorKind::Range
                        ) =>
                    {
                        return Ok(OperationResult::Throw(vm_error_to_value(error)));
                    }
                    Err(error) => return Err(error),
                },
            }
        };
        let new_target = if function.is_arrow {
            function
                .lexical_new_target
                .clone()
                .unwrap_or(JsValue::Undefined)
        } else {
            new_target
        };
        if function.is_generator {
            let stack_base = self.stack.len();
            let caller_environment_depth = context.environment_depth();
            let environment = context.push_environment(function.environment)?;
            if let Err(error) = self.bind_function_environment(
                function_id,
                &function,
                environment,
                &arguments,
                this_value.clone(),
                context,
            ) {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(error);
            }

            let frame = CallFrame::new(
                Some(function_id),
                0,
                environment,
                this_value.clone(),
                JsValue::Undefined,
                stack_base,
            );
            if let Err(error) = context.push_call_frame(frame) {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(error);
            }

            let preamble_end = function.chunk.function_body_start;
            let saved_finally_stack = self.finally_stack.clone();
            let result = if preamble_end == 0 {
                Ok(Completion::Normal(JsValue::Undefined))
            } else {
                self.run_completion_until(&function.chunk, context, 0, preamble_end)
            };
            self.finally_stack = saved_finally_stack;
            let (generator_environment_stack, generator_current_environment) =
                context.environment_state();
            self.stack.truncate(stack_base);
            let frame_result = context.pop_call_frame();
            let environment_result = context.restore_environment_depth(caller_environment_depth);
            frame_result?;
            environment_result?;

            match result? {
                Completion::Normal(_) => {
                    return self
                        .create_generator_object(
                            function_id,
                            Some(environment),
                            generator_environment_stack,
                            generator_current_environment,
                            this_value,
                            arguments,
                            preamble_end,
                            context,
                        )
                        .map(OperationResult::Value);
                }
                Completion::Throw(value) => return Ok(OperationResult::Throw(value)),
                Completion::Return(value) => return Ok(OperationResult::Value(value)),
                Completion::Yield { .. } | Completion::YieldDelegate { .. } => {
                    return Err(VmError::runtime(
                        "yield completion escaped from generator parameter initialization",
                    ));
                }
                Completion::Break(label) => {
                    return Err(VmError::runtime(format!(
                        "unhandled break completion{}",
                        label_suffix(label.as_deref())
                    )));
                }
                Completion::Continue(label) => {
                    return Err(VmError::runtime(format!(
                        "unhandled continue completion{}",
                        label_suffix(label.as_deref())
                    )));
                }
            }
        }
        let stack_base = self.stack.len();
        let caller_environment_depth = context.environment_depth();
        let environment = context.push_environment(function.environment)?;

        for (index, parameter) in function.params.iter().enumerate() {
            let value = arguments.get(index).cloned().unwrap_or(JsValue::Undefined);
            if let Err(error) =
                context.declare_binding(environment, parameter.clone(), value, true, true)
            {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(error);
            }
        }

        // Bind rest parameter: collect remaining arguments into an Array.
        if let Some(rest_name) = &function.rest_param {
            let rest_values = arguments
                .get(function.params.len()..)
                .unwrap_or(&[])
                .to_vec();
            let rest_array = match context.create_array(rest_values) {
                Ok(v) => v,
                Err(e) => {
                    let _ = context.restore_environment_depth(caller_environment_depth);
                    return Err(e);
                }
            };
            if let Err(error) =
                context.declare_binding(environment, rest_name.clone(), rest_array, true, true)
            {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(error);
            }
        }

        if let Some(name) = function
            .name
            .as_ref()
            .filter(|_| Self::should_bind_function_name_in_activation(&function))
            && let Err(error) = context.declare_binding(
                environment,
                name.clone(),
                JsValue::Function(function_id),
                true,
                true,
            )
        {
            let _ = context.restore_environment_depth(caller_environment_depth);
            return Err(error);
        }

        // Build the `arguments` exotic object only when the function does not
        // already declare an explicit `arguments` parameter (ES5 §10.6: if the
        // function has a formal parameter named "arguments" that binding wins).
        let has_explicit_arguments = function.is_arrow
            || function.params.iter().any(|p| p == "arguments")
            || function.rest_param.as_deref() == Some("arguments");

        if !has_explicit_arguments && function.uses_arguments {
            let proto = context.object_prototype();
            let arguments_obj = match context.ordinary_object_with_prototype(proto) {
                Ok(obj) => obj,
                Err(e) => {
                    let _ = context.restore_environment_depth(caller_environment_depth);
                    return Err(e);
                }
            };
            let arguments_id = match &arguments_obj {
                JsValue::Object(id) => *id,
                _ => unreachable!("ordinary_object_with_prototype always returns Object"),
            };
            for (i, arg) in arguments.iter().enumerate() {
                let key = i.to_string();
                if let Err(e) = context.define_own_property(
                    arguments_id,
                    key,
                    PropertyDescriptor::data(arg.clone()),
                ) {
                    let _ = context.restore_environment_depth(caller_environment_depth);
                    return Err(e);
                }
            }
            let length_val = JsValue::Number(arguments.len() as f64);
            if let Err(e) = context.define_own_property(
                arguments_id,
                "length".into(),
                PropertyDescriptor::data_with(length_val, true, false, true),
            ) {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(e);
            }
            if let Err(e) = define_arguments_iterator(context, arguments_id) {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(e);
            }
            if let Err(error) =
                context.declare_binding(environment, "arguments", arguments_obj, true, false)
            {
                let _ = context.restore_environment_depth(caller_environment_depth);
                return Err(error);
            }
        }

        let frame = CallFrame::new(
            Some(function_id),
            0,
            environment,
            this_value,
            new_target,
            stack_base,
        );
        if let Err(error) = context.push_call_frame(frame) {
            let _ = context.restore_environment_depth(caller_environment_depth);
            return Err(error);
        }

        let saved_finally_stack = self.finally_stack.clone();
        let result = self.run_completion(&function.chunk, context);
        self.finally_stack = saved_finally_stack;
        self.stack.truncate(stack_base);
        let frame_result = context.pop_call_frame();
        let environment_result = context.restore_environment_depth(caller_environment_depth);

        let operation = match result {
            Err(error) => Err(error),
            Ok(completion) => {
                frame_result?;
                environment_result?;
                match completion {
                    Completion::Normal(value) | Completion::Return(value) => {
                        Ok(OperationResult::Value(value))
                    }
                    Completion::Yield { .. } | Completion::YieldDelegate { .. } => Err(
                        VmError::runtime("yield completion escaped outside a generator"),
                    ),
                    Completion::Throw(value) => Ok(OperationResult::Throw(value)),
                    Completion::Break(label) => Err(VmError::runtime(format!(
                        "unhandled break completion{}",
                        label_suffix(label.as_deref())
                    ))),
                    Completion::Continue(label) => Err(VmError::runtime(format!(
                        "unhandled continue completion{}",
                        label_suffix(label.as_deref())
                    ))),
                }
            }
        };
        if function.is_async {
            self.wrap_async_function_result(operation?, context)
        } else {
            operation
        }
    }

    fn should_bind_function_name_in_activation(function: &JsFunction) -> bool {
        if !function.binds_name_in_activation {
            return false;
        }
        let Some(name) = &function.name else {
            return false;
        };
        name != "arguments"
            && !function.params.iter().any(|parameter| parameter == name)
            && function.rest_param.as_deref() != Some(name.as_str())
    }

    fn wrap_async_function_result(
        &mut self,
        operation: OperationResult,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let promise = context.create_promise()?;
        let prototype = context
            .get_global("Promise")
            .and_then(|constructor| context.value_object(&constructor))
            .and_then(|constructor| {
                context
                    .get_own_property_descriptor(constructor, "prototype")
                    .and_then(|descriptor| descriptor.value_cloned())
                    .and_then(|value| context.value_object(&value))
            });
        let promise_object = context.create_promise_object(promise, prototype)?;
        match operation {
            OperationResult::Value(value) => {
                // AsyncFunctionStart resolves its capability; it must adopt a
                // returned Promise/thenable instead of fulfilling with it as a
                // plain object.
                crate::builtins::promise::resolve_promise_id(self, context, promise, value)?;
            }
            OperationResult::Throw(value) => {
                context.reject_promise(promise, value)?;
            }
        }
        Ok(OperationResult::Value(promise_object))
    }

    fn construct_value(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        self.construct_value_with_new_target(constructor.clone(), arguments, constructor, context)
    }

    fn construct_value_with_new_target(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        new_target: JsValue,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        match constructor {
            JsValue::Object(object) if context.proxy_record(object).is_some() => {
                match proxy::internal_construct(
                    self,
                    context,
                    JsValue::Object(object),
                    arguments,
                    new_target,
                ) {
                    Ok(value) => Ok(OperationResult::Value(value)),
                    Err(error) => self.error_to_operation_result_in_context(error, context),
                }
            }
            JsValue::Function(function_id) => {
                let activation = match context.realm_for_function(function_id) {
                    Some(realm) if !context.is_current_realm(realm) => {
                        Some(context.enter_realm(realm)?)
                    }
                    None => None,
                    Some(_) => None,
                };
                let operation = (|| {
                    if context.function(function_id).is_some_and(|function| {
                        function.is_generator || function.is_async || function.is_arrow
                    }) {
                        let name = context
                            .function(function_id)
                            .and_then(|function| function.name.as_deref())
                            .filter(|name| !name.is_empty())
                            .unwrap_or("function");
                        return Ok(OperationResult::Throw(vm_error_to_value(
                            VmError::type_error(format!("{name} is not a constructor")),
                        )));
                    }
                    let prototype =
                        self.get_prototype_from_constructor(new_target.clone(), context)?;
                    let instance = context.ordinary_object_with_prototype(prototype)?;
                    let is_derived = context
                        .function(function_id)
                        .is_some_and(|function| function.is_derived_constructor);
                    if is_derived {
                        context.push_pending_derived_this(instance.clone());
                    }
                    match self.call_user_function(
                        function_id,
                        if is_derived {
                            JsValue::Undefined
                        } else {
                            instance.clone()
                        },
                        arguments,
                        new_target.clone(),
                        context,
                    )? {
                        OperationResult::Value(result) if is_object_like(&result) => {
                            if is_derived {
                                let _ = context.pop_pending_derived_this();
                            }
                            Ok(OperationResult::Value(result))
                        }
                        OperationResult::Value(result) => {
                            if is_derived {
                                let (derived_this, initialized) = context
                                    .pop_pending_derived_this()
                                    .unwrap_or((JsValue::Undefined, false));
                                if !matches!(result, JsValue::Undefined) {
                                    return Ok(OperationResult::Throw(vm_error_to_value(
                                        VmError::type_error(
                                            "derived constructor may only return an object or undefined",
                                        ),
                                    )));
                                }
                                if !initialized {
                                    return Ok(OperationResult::Throw(vm_error_to_value(
                                        VmError::reference(
                                            "derived constructor must call super() before returning",
                                        ),
                                    )));
                                }
                                return Ok(OperationResult::Value(derived_this));
                            }
                            Ok(OperationResult::Value(instance))
                        }
                        OperationResult::Throw(value) => {
                            if is_derived {
                                let _ = context.pop_pending_derived_this();
                            }
                            Ok(OperationResult::Throw(value))
                        }
                    }
                })();
                match (
                    operation,
                    activation.map(|activation| context.leave_realm(activation)),
                ) {
                    (Ok(value), None) => Ok(value),
                    (Err(error), None) => Err(error),
                    (Ok(value), Some(Ok(()))) => Ok(value),
                    (Err(error), Some(Ok(()))) => Err(error),
                    (Ok(_), Some(Err(error))) | (Err(error), Some(Err(_))) => Err(error),
                }
            }
            JsValue::BuiltinFunction(id) => {
                context.consume_call_depth()?;
                let result: Result<OperationResult, VmError> = (|| {
                    let activation = match context.realm_for_builtin(id) {
                        Some(realm) if !context.is_current_realm(realm) => {
                            Some(context.enter_realm(realm)?)
                        }
                        None => None,
                        Some(_) => None,
                    };
                    let operation = (|| {
                        let def = context
                            .builtin(id)
                            .ok_or_else(|| VmError::runtime("invalid builtin id"))?
                            .clone();
                        // `new boundFn(...)` constructs the target with the bound
                        // arguments prepended (the bound `this` is ignored for `new`).
                        if let Some(bound) = &def.bound {
                            if def.construct.is_none() {
                                return Ok(OperationResult::Throw(vm_error_to_value(
                                    VmError::type_error(format!(
                                        "{} is not a constructor",
                                        def.name
                                    )),
                                )));
                            }
                            let mut forwarded = bound.args.clone();
                            forwarded.extend(arguments);
                            let target = bound.target.clone();
                            let effective_new_target =
                                if new_target.same_value(&JsValue::BuiltinFunction(id)) {
                                    target.clone()
                                } else {
                                    new_target
                                };
                            return self.construct_value_with_new_target(
                                target,
                                forwarded,
                                effective_new_target,
                                context,
                            );
                        }
                        match def.construct {
                            Some(construct) => {
                                match construct(self, context, &arguments, new_target) {
                                    Ok(value) => Ok(OperationResult::Value(value)),
                                    Err(error) => match self.pending_exception.take() {
                                        Some(value) => Ok(OperationResult::Throw(value)),
                                        None => match error.kind {
                                            VmErrorKind::Reference
                                            | VmErrorKind::Type
                                            | VmErrorKind::Syntax
                                            | VmErrorKind::Range
                                            | VmErrorKind::Test262 => {
                                                Ok(OperationResult::Throw(vm_error_to_value(error)))
                                            }
                                            _ => Err(error),
                                        },
                                    },
                                }
                            }
                            None => Ok(OperationResult::Throw(vm_error_to_value(
                                VmError::type_error(format!("{} is not a constructor", def.name)),
                            ))),
                        }
                    })();
                    match (
                        operation,
                        activation.map(|activation| context.leave_realm(activation)),
                    ) {
                        (Ok(value), None) => Ok(value),
                        (Err(error), None) => Err(error),
                        (Ok(value), Some(Ok(()))) => Ok(value),
                        (Err(error), Some(Ok(()))) => Err(error),
                        (Ok(_), Some(Err(error))) | (Err(error), Some(Err(_))) => Err(error),
                    }
                })();
                context.release_call_depth();
                result
            }
            other => Ok(OperationResult::Throw(vm_error_to_value(
                VmError::type_error(format!("{other} is not a constructor")),
            ))),
        }
    }

    fn validate_jump_target(
        &self,
        target: usize,
        chunk: &Chunk,
        instruction_pointer: usize,
    ) -> Result<(), VmError> {
        if target >= chunk.instructions.len() {
            return Err(VmError::runtime(format!(
                "jump target {target} is out of bounds at instruction {instruction_pointer}"
            )));
        }
        Ok(())
    }

    fn jump_to(
        &self,
        target: usize,
        current_instruction: usize,
        context: &mut NativeContext,
        instruction_pointer: &mut usize,
    ) -> Result<(), VmError> {
        if target <= current_instruction {
            context.consume_loop_iteration()?;
        }
        *instruction_pointer = target;
        Ok(())
    }

    pub fn drain_jobs(&mut self, context: &mut NativeContext) -> Result<(), VmError> {
        while let Some(job) = context.pop_job() {
            match job {
                Job::HostCallback(crate::runtime::NativeJob::PushOutput(line)) => {
                    context.push_output(line);
                }
                Job::PromiseReaction(job) => match job.reaction {
                    PromiseReaction::Fulfill => {
                        context.fulfill_promise(job.promise, job.value)?;
                    }
                    PromiseReaction::Reject => {
                        context.reject_promise(job.promise, job.value)?;
                    }
                },
                Job::PromiseCallback(job) => self.run_promise_callback_job(context, job)?,
            }
        }
        Ok(())
    }

    fn run_promise_callback_job(
        &mut self,
        context: &mut NativeContext,
        job: PromiseCallbackJob,
    ) -> Result<(), VmError> {
        let handler = if job.fulfilled {
            job.on_fulfilled
        } else {
            job.on_rejected
        };
        let Some(handler) = handler else {
            if job.fulfilled {
                context.fulfill_promise(job.result_promise, job.value)?;
            } else {
                context.reject_promise(job.result_promise, job.value)?;
            }
            return Ok(());
        };

        let original_value = job.value;
        let args = if job.finally {
            Vec::new()
        } else {
            vec![original_value.clone()]
        };
        match self.call_value(handler, JsValue::Undefined, args, context)? {
            OperationResult::Value(value) => {
                if job.finally {
                    if job.fulfilled {
                        context.fulfill_promise(job.result_promise, original_value)?;
                    } else {
                        context.reject_promise(job.result_promise, original_value)?;
                    }
                    return Ok(());
                }
                crate::builtins::promise::resolve_promise_id(
                    self,
                    context,
                    job.result_promise,
                    value,
                )?;
            }
            OperationResult::Throw(value) => {
                context.reject_promise(job.result_promise, value)?;
            }
        }
        Ok(())
    }
}

fn same_ecmascript_type(left: &JsValue, right: &JsValue) -> bool {
    matches!(
        (left, right),
        (JsValue::Undefined, JsValue::Undefined)
            | (JsValue::Null, JsValue::Null)
            | (JsValue::Boolean(_), JsValue::Boolean(_))
            | (JsValue::Number(_), JsValue::Number(_))
            | (JsValue::String(_), JsValue::String(_))
            | (JsValue::Object(_), JsValue::Object(_))
            | (JsValue::Function(_), JsValue::Function(_))
            | (JsValue::BuiltinFunction(_), JsValue::BuiltinFunction(_))
            | (JsValue::Error(_), JsValue::Error(_))
    )
}

fn is_object_like(value: &JsValue) -> bool {
    matches!(
        value,
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)
    )
}

fn is_callable_value(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn intrinsic_iterator_prototype(context: &NativeContext) -> Option<ObjectId> {
    let constructor = context.get_global("Iterator")?;
    let constructor = context.value_object(&constructor)?;
    context
        .get_own_property_descriptor(constructor, "prototype")?
        .value_cloned()
        .and_then(|value| context.value_object(&value))
}

fn intrinsic_async_iterator_prototype(context: &NativeContext) -> Option<ObjectId> {
    let constructor = context.get_global("Iterator")?;
    let constructor = context.value_object(&constructor)?;
    context
        .get_own_property_descriptor(constructor, "__agentjs_async_iterator_prototype__")?
        .value_cloned()
        .and_then(|value| context.value_object(&value))
}

fn existing_accessor_getter(
    context: &NativeContext,
    object: ObjectId,
    key: &str,
) -> Option<JsValue> {
    let descriptor = context.get_own_property_descriptor(object, key)?;
    match descriptor.kind {
        PropertyKind::Accessor { get, .. } => get,
        PropertyKind::Data { .. } => None,
    }
}

fn existing_accessor_setter(
    context: &NativeContext,
    object: ObjectId,
    key: &str,
) -> Option<JsValue> {
    let descriptor = context.get_own_property_descriptor(object, key)?;
    match descriptor.kind {
        PropertyKind::Accessor { set, .. } => set,
        PropertyKind::Data { .. } => None,
    }
}

fn find_handler(
    chunk: &Chunk,
    instruction_offset: usize,
    completion: &Completion,
) -> Option<ExceptionHandler> {
    let accepts = |kind| match completion {
        Completion::Throw(_) => matches!(kind, HandlerKind::Catch | HandlerKind::Finally),
        Completion::Return(_)
        | Completion::Break(_)
        | Completion::Continue(_)
        | Completion::Normal(_) => kind == HandlerKind::Finally,
        Completion::Yield { .. } | Completion::YieldDelegate { .. } => false,
    };

    chunk
        .handlers
        .iter()
        .copied()
        .filter(|handler| {
            accepts(handler.kind)
                && handler.start <= instruction_offset
                && instruction_offset < handler.end
        })
        .min_by_key(|handler| {
            let range = handler.end - handler.start;
            let same_range_priority = match (completion, handler.kind) {
                (Completion::Throw(_), HandlerKind::Catch) => 0,
                (Completion::Throw(_), HandlerKind::Finally) => 1,
                (_, HandlerKind::Finally) => 0,
                (_, HandlerKind::Catch) => 1,
            };
            (range, same_range_priority, handler.start)
        })
}

fn constant_to_value(constant: &Constant) -> JsValue {
    match constant {
        Constant::Undefined => JsValue::Undefined,
        Constant::Null => JsValue::Null,
        Constant::Boolean(value) => JsValue::Boolean(*value),
        Constant::Number(value) => JsValue::Number(*value),
        Constant::BigInt(value) => JsValue::BigInt(value.clone()),
        Constant::String(value) => JsValue::String(value.clone()),
    }
}

fn add_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    left: JsValue,
    right: JsValue,
) -> Result<JsValue, VmError> {
    let left = vm.to_primitive(left, PreferredType::Default, context)?;
    let right = vm.to_primitive(right, PreferredType::Default, context)?;
    if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
        let left = vm.to_string_coerce(left, context)?;
        let right = vm.to_string_coerce(right, context)?;
        let mut result = String::with_capacity(left.len().saturating_add(right.len()));
        result.push_str(&left);
        result.push_str(&right);
        return Ok(JsValue::String(result));
    }
    if let (JsValue::BigInt(left), JsValue::BigInt(right)) = (&left, &right) {
        return Ok(JsValue::BigInt(bigint::add(left, right)));
    }
    if matches!(left, JsValue::BigInt(_)) || matches!(right, JsValue::BigInt(_)) {
        return Err(VmError::type_error(
            "Cannot mix BigInt and other types in arithmetic",
        ));
    }

    let left = vm.to_number(left, context)?;
    let right = vm.to_number(right, context)?;
    Ok(JsValue::Number(left + right))
}

fn generator_next(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let sent = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    vm.resume_generator(this_value, sent, context)
}

fn async_generator_next(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = generator_next(vm, context, this_value, arguments);
    wrap_async_generator_result(vm, context, result)
}

fn async_generator_return(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = generator_return(vm, context, this_value, arguments);
    wrap_async_generator_result(vm, context, result)
}

fn async_generator_throw(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = generator_throw(vm, context, this_value, arguments);
    wrap_async_generator_result(vm, context, result)
}

fn wrap_async_generator_result(
    vm: &mut Vm,
    context: &mut NativeContext,
    result: Result<JsValue, VmError>,
) -> Result<JsValue, VmError> {
    let operation = match result {
        Ok(value) => OperationResult::Value(value),
        Err(error) => vm.error_to_operation_result(error)?,
    };
    match vm.wrap_async_function_result(operation, context)? {
        OperationResult::Value(promise) => Ok(promise),
        OperationResult::Throw(_) => unreachable!("async result wrapping always returns a Promise"),
    }
}

fn generator_iterator(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn generator_return(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&this_value, "Generator.prototype.return")?;
    let mut record = match context.heap().object(object).map(|object| &object.kind) {
        Some(ObjectKind::Generator { record }) => record.clone(),
        _ => {
            return Err(VmError::type_error(
                "Generator method called on non-generator",
            ));
        }
    };
    if let Some(iterator) = record.delegate_iterator.clone() {
        match vm.return_yield_star_delegate(iterator, value.clone(), context)? {
            YieldStarStepResult::Yield(delegate_result) => {
                let Some(object) = context.heap_mut().object_mut(object) else {
                    return Err(VmError::runtime("missing generator object"));
                };
                object.kind = ObjectKind::Generator { record };
                return Ok(delegate_result);
            }
            YieldStarStepResult::Complete(delegate_value) => {
                record.delegate_iterator = None;
                record.delegate_return = None;
                vm.write_generator_record(context, object, record)?;
                return vm.resume_generator_with_completion(
                    this_value,
                    JsValue::Undefined,
                    Some(Completion::Return(delegate_value)),
                    context,
                );
            }
            YieldStarStepResult::Throw(value) => {
                record.delegate_iterator = None;
                record.delegate_return = None;
                vm.write_generator_record(context, object, record)?;
                return vm.resume_generator_with_completion(
                    this_value,
                    JsValue::Undefined,
                    Some(Completion::Throw(value)),
                    context,
                );
            }
        }
    }

    if matches!(record.state, GeneratorState::SuspendedYield) {
        return vm.resume_generator_with_completion(
            this_value,
            JsValue::Undefined,
            Some(Completion::Return(value)),
            context,
        );
    }
    if matches!(record.state, GeneratorState::Executing) {
        return Err(VmError::type_error("generator is already executing"));
    }

    record.state = GeneratorState::Completed;
    record.stack.clear();
    record.delegate_values.clear();
    record.delegate_iterator = None;
    record.delegate_return = None;
    let Some(object) = context.heap_mut().object_mut(object) else {
        return Err(VmError::runtime("missing generator object"));
    };
    object.kind = ObjectKind::Generator { record };
    generator_result(context, value, true)
}

fn generator_throw(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&this_value, "Generator.prototype.throw")?;
    let mut record = match context.heap().object(object).map(|object| &object.kind) {
        Some(ObjectKind::Generator { record }) => record.clone(),
        _ => {
            return Err(VmError::type_error(
                "Generator method called on non-generator",
            ));
        }
    };
    if let Some(iterator) = record.delegate_iterator.clone() {
        match vm.throw_yield_star_delegate(iterator, value.clone(), context)? {
            YieldStarStepResult::Yield(delegate_result) => {
                let Some(object) = context.heap_mut().object_mut(object) else {
                    return Err(VmError::runtime("missing generator object"));
                };
                object.kind = ObjectKind::Generator { record };
                return Ok(delegate_result);
            }
            YieldStarStepResult::Complete(delegate_value) => {
                record.delegate_iterator = None;
                record.delegate_return = Some(delegate_value);
                let Some(object_ref) = context.heap_mut().object_mut(object) else {
                    return Err(VmError::runtime("missing generator object"));
                };
                object_ref.kind = ObjectKind::Generator { record };
                return vm.resume_generator(this_value, JsValue::Undefined, context);
            }
            YieldStarStepResult::Throw(value) => {
                record.delegate_iterator = None;
                record.delegate_return = None;
                vm.write_generator_record(context, object, record)?;
                return vm.resume_generator_with_completion(
                    this_value,
                    JsValue::Undefined,
                    Some(Completion::Throw(value)),
                    context,
                );
            }
        }
    }
    if matches!(record.state, GeneratorState::SuspendedYield) {
        return vm.resume_generator_with_completion(
            this_value,
            JsValue::Undefined,
            Some(Completion::Throw(value)),
            context,
        );
    }
    if matches!(record.state, GeneratorState::Executing) {
        return Err(VmError::type_error("generator is already executing"));
    }
    record.state = GeneratorState::Completed;
    record.stack.clear();
    record.delegate_values.clear();
    record.delegate_iterator = None;
    record.delegate_return = None;
    let Some(object) = context.heap_mut().object_mut(object) else {
        return Err(VmError::runtime("missing generator object"));
    };
    object.kind = ObjectKind::Generator { record };
    vm.pending_exception = Some(value);
    Err(VmError::runtime("generator throw"))
}

fn generator_result(
    context: &mut NativeContext,
    value: JsValue,
    done: bool,
) -> Result<JsValue, VmError> {
    context.create_object([
        ("value".to_string(), value),
        ("done".to_string(), JsValue::Boolean(done)),
    ])
}

fn compare_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    left: JsValue,
    right: JsValue,
    predicate: impl FnOnce(std::cmp::Ordering) -> bool,
) -> Result<bool, VmError> {
    let left = vm.to_primitive(left, PreferredType::Number, context)?;
    let right = vm.to_primitive(right, PreferredType::Number, context)?;
    if let (JsValue::String(left), JsValue::String(right)) = (&left, &right) {
        return Ok(predicate(left.cmp(right)));
    }
    if let (JsValue::BigInt(left), JsValue::BigInt(right)) = (&left, &right) {
        return Ok(predicate(bigint::cmp(left, right)));
    }
    if let (JsValue::BigInt(left), JsValue::Number(right)) = (&left, &right) {
        return Ok(bigint::compare_bigint_number(left, *right).is_some_and(predicate));
    }
    if let (JsValue::Number(left), JsValue::BigInt(right)) = (&left, &right) {
        return Ok(bigint::compare_bigint_number(right, *left)
            .is_some_and(|ordering| predicate(ordering.reverse())));
    }
    if let (JsValue::BigInt(left), JsValue::String(right)) = (&left, &right) {
        let Some(right) = bigint::parse_bigint_string(right) else {
            return Ok(false);
        };
        return Ok(predicate(bigint::cmp(left, &right)));
    }
    if let (JsValue::String(left), JsValue::BigInt(right)) = (&left, &right) {
        let Some(left) = bigint::parse_bigint_string(left) else {
            return Ok(false);
        };
        return Ok(predicate(bigint::cmp(&left, right)));
    }

    let left = vm.to_number(left, context)?;
    let right = vm.to_number(right, context)?;

    let Some(ordering) = left.partial_cmp(&right) else {
        return Ok(false);
    };
    Ok(predicate(ordering))
}

fn bigint_binary(
    left: JsValue,
    right: JsValue,
    operation: impl FnOnce(
        &crate::runtime::BigIntValue,
        &crate::runtime::BigIntValue,
    ) -> Result<crate::runtime::BigIntValue, VmError>,
) -> Result<Option<JsValue>, VmError> {
    match (left, right) {
        (JsValue::BigInt(left), JsValue::BigInt(right)) => {
            operation(&left, &right).map(|value| Some(JsValue::BigInt(value)))
        }
        (JsValue::BigInt(_), _) | (_, JsValue::BigInt(_)) => Err(VmError::type_error(
            "Cannot mix BigInt and other types in arithmetic",
        )),
        _ => Ok(None),
    }
}

fn numeric_number_pair(left: JsValue, right: JsValue) -> Result<(f64, f64), VmError> {
    let (JsValue::Number(left), JsValue::Number(right)) = (left, right) else {
        return Err(VmError::type_error(
            "Cannot mix BigInt and other types in arithmetic",
        ));
    };
    Ok((left, right))
}

fn number_to_int32(value: f64) -> i32 {
    number_to_uint32(value) as i32
}

fn number_to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(4_294_967_296.0) as u32
}

fn bigint_divide(left: JsValue, right: JsValue) -> Result<Option<JsValue>, VmError> {
    bigint_binary(left, right, bigint::div)
}

fn bigint_remainder(left: JsValue, right: JsValue) -> Result<Option<JsValue>, VmError> {
    bigint_binary(left, right, bigint::rem)
}

fn bigint_exponentiation(left: JsValue, right: JsValue) -> Result<Option<JsValue>, VmError> {
    bigint_binary(left, right, bigint::pow)
}

fn bigint_shift(
    left: JsValue,
    right: JsValue,
    right_shift: bool,
) -> Result<Option<JsValue>, VmError> {
    let (JsValue::BigInt(left), JsValue::BigInt(right)) = (&left, &right) else {
        if matches!(left, JsValue::BigInt(_)) || matches!(right, JsValue::BigInt(_)) {
            return Err(VmError::type_error(
                "Cannot mix BigInt and other types in arithmetic",
            ));
        }
        return Ok(None);
    };
    let value = if right_shift {
        bigint::shr(left, right)?
    } else {
        bigint::shl(left, right)?
    };
    Ok(Some(JsValue::BigInt(value)))
}

fn increment_numeric(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<JsValue, VmError> {
    match value {
        JsValue::BigInt(value) => Ok(JsValue::BigInt(bigint::add(&value, &bigint::from_i64(1)))),
        value => Ok(JsValue::Number(vm.to_number(value, context)? + 1.0)),
    }
}

fn decrement_numeric(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<JsValue, VmError> {
    match value {
        JsValue::BigInt(value) => Ok(JsValue::BigInt(bigint::sub(&value, &bigint::from_i64(1)))),
        value => Ok(JsValue::Number(vm.to_number(value, context)? - 1.0)),
    }
}

fn throw_value(value: JsValue, context: &NativeContext) -> VmError {
    match value {
        JsValue::Error(error) if error.kind == NativeErrorKind::Test262 => {
            VmError::test262(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::Reference => {
            VmError::reference(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::Type => {
            VmError::type_error(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::Syntax => {
            VmError::syntax_error(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::Range => {
            VmError::range(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::RuntimeLimit => {
            VmError::runtime_limit(error.message)
        }
        // JS error objects created via `new EvalError(...)` etc. — include the
        // constructor name in the message so failure_matches can detect the type.
        JsValue::Object(id) if context.is_error_object(id) => {
            let name = context.error_object_name(id).unwrap_or("Error");
            // failure_matches checks error.message.contains(expected), so embedding
            // the name here lets the test262 runner recognise e.g. "EvalError".
            VmError::runtime(format!("uncaught {name}"))
        }
        value => VmError::runtime(format!("uncaught {value}")),
    }
}

fn vm_error_to_value(error: VmError) -> JsValue {
    let kind = match error.kind {
        VmErrorKind::Reference => NativeErrorKind::Reference,
        VmErrorKind::Type => NativeErrorKind::Type,
        VmErrorKind::Syntax => NativeErrorKind::Syntax,
        VmErrorKind::Range => NativeErrorKind::Range,
        VmErrorKind::Test262 => NativeErrorKind::Test262,
        VmErrorKind::RuntimeLimit => NativeErrorKind::RuntimeLimit,
        VmErrorKind::Runtime => NativeErrorKind::Error,
    };
    JsValue::Error(crate::runtime::NativeErrorValue::new(
        kind,
        error.to_string(),
    ))
}

fn vm_error_to_value_with_realm(error: VmError, realm_global: ObjectId) -> JsValue {
    let kind = match error.kind {
        VmErrorKind::Reference => NativeErrorKind::Reference,
        VmErrorKind::Type => NativeErrorKind::Type,
        VmErrorKind::Syntax => NativeErrorKind::Syntax,
        VmErrorKind::Range => NativeErrorKind::Range,
        VmErrorKind::Test262 => NativeErrorKind::Test262,
        VmErrorKind::RuntimeLimit => NativeErrorKind::RuntimeLimit,
        VmErrorKind::Runtime => NativeErrorKind::Error,
    };
    JsValue::Error(crate::runtime::NativeErrorValue::new_with_realm(
        kind,
        error.to_string(),
        realm_global,
    ))
}

fn native_error_constructor_name(kind: &NativeErrorKind) -> &'static str {
    match kind {
        NativeErrorKind::Reference => "ReferenceError",
        NativeErrorKind::Type => "TypeError",
        NativeErrorKind::Syntax => "SyntaxError",
        NativeErrorKind::Range => "RangeError",
        NativeErrorKind::RuntimeLimit => "RangeError",
        NativeErrorKind::Error => "Error",
        NativeErrorKind::Test262 => "Test262Error",
    }
}

fn label_suffix(label: Option<&str>) -> String {
    label.map_or_else(String::new, |label| format!(" to {label}"))
}

/// Pure string-to-number conversion (no object coercion).
fn coerce_string_to_number(s: &str) -> f64 {
    let trimmed = s.trim_matches(is_ecmascript_whitespace);
    if trimmed.is_empty() {
        return 0.0;
    }
    match trimmed {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => parse_prefixed_integer(trimmed)
            .or_else(|| parse_decimal_number(trimmed))
            .unwrap_or(f64::NAN),
    }
}

fn parse_prefixed_integer(input: &str) -> Option<f64> {
    let (digits, radix) = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .map(|digits| (digits, 16))
        .or_else(|| {
            input
                .strip_prefix("0b")
                .or_else(|| input.strip_prefix("0B"))
                .map(|digits| (digits, 2))
        })
        .or_else(|| {
            input
                .strip_prefix("0o")
                .or_else(|| input.strip_prefix("0O"))
                .map(|digits| (digits, 8))
        })?;
    if digits.is_empty() {
        return Some(f64::NAN);
    }
    let mut value = 0.0;
    for character in digits.chars() {
        let Some(digit) = character.to_digit(radix) else {
            return Some(f64::NAN);
        };
        value = value * f64::from(radix) + f64::from(digit);
    }
    Some(value)
}

fn parse_decimal_number(input: &str) -> Option<f64> {
    let mut chars = input.chars().peekable();
    if matches!(chars.peek(), Some('+') | Some('-')) {
        chars.next();
    }

    let mut digits = 0usize;
    while chars
        .peek()
        .is_some_and(|character| character.is_ascii_digit())
    {
        chars.next();
        digits += 1;
    }

    if matches!(chars.peek(), Some('.')) {
        chars.next();
        while chars
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            chars.next();
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }

    if matches!(chars.peek(), Some('e') | Some('E')) {
        chars.next();
        if matches!(chars.peek(), Some('+') | Some('-')) {
            chars.next();
        }
        let mut exponent_digits = 0usize;
        while chars
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            chars.next();
            exponent_digits += 1;
        }
        if exponent_digits == 0 {
            return None;
        }
    }

    if chars.next().is_some() {
        return None;
    }
    input.parse::<f64>().ok()
}
fn is_ecmascript_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{FEFF}'
            | '\u{000A}'
            | '\u{000D}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// Pure number-to-string conversion (no object coercion).
fn coerce_number_to_string(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else if value == 0.0 {
        "0".into()
    } else {
        let magnitude = value.abs();
        if !(1e-6..1e21).contains(&magnitude) {
            js_scientific_number_to_string(value)
        } else {
            value.to_string()
        }
    }
}

fn js_scientific_number_to_string(value: f64) -> String {
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let raw = format!("{:e}", value.abs());
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return format!("{sign}{raw}");
    };
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    format!("{sign}{mantissa}e{exponent:+}")
}

#[cfg(test)]
mod tests {
    use crate::{
        builtins,
        bytecode::{Chunk, Constant, Instruction},
        runtime::{JsValue, NativeContext},
        vm::VmErrorKind,
    };

    use super::{Vm, number_to_int32, number_to_uint32};

    fn constant(chunk: &mut Chunk, constant: Constant) -> u16 {
        chunk.add_constant(constant).unwrap()
    }

    #[test]
    fn executes_hand_written_addition_bytecode() {
        let mut chunk = Chunk::default();
        let left = constant(&mut chunk, Constant::Number(1.0));
        let right = constant(&mut chunk, Constant::Number(2.0));
        chunk.emit(Instruction::Constant(left));
        chunk.emit(Instruction::Constant(right));
        chunk.emit(Instruction::Add);
        chunk.emit(Instruction::Return);

        assert_eq!(Vm::default().execute(&chunk).unwrap(), JsValue::Number(3.0));
    }

    #[test]
    fn reports_operand_stack_underflow() {
        let chunk = Chunk {
            instructions: vec![Instruction::Pop, Instruction::ReturnUndefined],
            constants: Vec::new(),
            functions: Vec::new(),
            handlers: Vec::new(),
            function_body_start: 0,
        };
        let error = Vm::default().execute(&chunk).unwrap_err();

        assert_eq!(error.kind, VmErrorKind::Runtime);
        assert!(error.message.contains("invalid bytecode chunk"));
        assert!(error.message.contains("requires"));
    }

    #[test]
    fn executes_numeric_and_string_operations() {
        let mut chunk = Chunk::default();
        let empty = constant(&mut chunk, Constant::String(String::new()));
        let agent = constant(&mut chunk, Constant::String("agent".into()));
        let number = constant(&mut chunk, Constant::Number(262.0));

        chunk.emit(Instruction::Constant(empty));
        chunk.emit(Instruction::UnaryPlus);
        chunk.emit(Instruction::Pop);
        chunk.emit(Instruction::Constant(empty));
        chunk.emit(Instruction::Negate);
        chunk.emit(Instruction::Pop);
        chunk.emit(Instruction::Constant(agent));
        chunk.emit(Instruction::Constant(number));
        chunk.emit(Instruction::Add);
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&chunk).unwrap(),
            JsValue::String("agent262".into())
        );
    }

    #[test]
    fn preserves_negative_zero_from_unary_minus() {
        let mut chunk = Chunk::default();
        let empty = constant(&mut chunk, Constant::String(String::new()));
        chunk.emit(Instruction::Constant(empty));
        chunk.emit(Instruction::Negate);
        chunk.emit(Instruction::Return);

        let JsValue::Number(value) = Vm::default().execute(&chunk).unwrap() else {
            panic!("expected number");
        };
        assert_eq!(value, 0.0);
        assert!(value.is_sign_negative());
    }

    #[test]
    fn implements_strict_equality_number_edges() {
        let mut chunk = Chunk::default();
        let nan = constant(&mut chunk, Constant::Number(f64::NAN));
        chunk.emit(Instruction::Constant(nan));
        chunk.emit(Instruction::Constant(nan));
        chunk.emit(Instruction::StrictEqual);
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&chunk).unwrap(),
            JsValue::Boolean(false)
        );
    }

    #[test]
    fn implements_basic_abstract_equality_coercion() {
        let mut chunk = Chunk::default();
        let number = constant(&mut chunk, Constant::Number(1.0));
        let string = constant(&mut chunk, Constant::String("1".into()));
        chunk.emit(Instruction::Constant(number));
        chunk.emit(Instruction::Constant(string));
        chunk.emit(Instruction::Equal);
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&chunk).unwrap(),
            JsValue::Boolean(true)
        );
    }

    #[test]
    fn reads_and_writes_global_bindings_through_context() {
        let mut chunk = Chunk::default();
        let name = constant(&mut chunk, Constant::String("x".into()));
        let value = constant(&mut chunk, Constant::Number(18.0));
        let divisor = constant(&mut chunk, Constant::Number(3.0));

        chunk.emit(Instruction::Constant(value));
        chunk.emit(Instruction::DeclareGlobal(name));
        chunk.emit(Instruction::LoadGlobal(name));
        chunk.emit(Instruction::Constant(divisor));
        chunk.emit(Instruction::Divide);
        chunk.emit(Instruction::Return);

        assert_eq!(Vm::default().execute(&chunk).unwrap(), JsValue::Number(6.0));
    }

    #[test]
    fn short_circuits_without_loading_missing_globals() {
        let mut and_chunk = Chunk::default();
        let false_value = constant(&mut and_chunk, Constant::Boolean(false));
        let missing = constant(&mut and_chunk, Constant::String("missingName".into()));
        and_chunk.emit(Instruction::Constant(false_value));
        and_chunk.emit(Instruction::JumpIfFalse(4));
        and_chunk.emit(Instruction::Pop);
        and_chunk.emit(Instruction::LoadGlobal(missing));
        and_chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&and_chunk).unwrap(),
            JsValue::Boolean(false)
        );

        let mut or_chunk = Chunk::default();
        let true_value = constant(&mut or_chunk, Constant::Boolean(true));
        let missing = constant(&mut or_chunk, Constant::String("missingName".into()));
        or_chunk.emit(Instruction::Constant(true_value));
        or_chunk.emit(Instruction::JumpIfTrue(4));
        or_chunk.emit(Instruction::Pop);
        or_chunk.emit(Instruction::LoadGlobal(missing));
        or_chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&or_chunk).unwrap(),
            JsValue::Boolean(true)
        );
    }

    #[test]
    fn reports_invalid_constant_index() {
        let mut chunk = Chunk::default();
        chunk.emit(Instruction::Constant(0));
        chunk.emit(Instruction::Return);

        let error = Vm::default().execute(&chunk).unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Runtime);
    }

    #[test]
    fn clears_temporary_stack_between_runs() {
        let mut first = Chunk::default();
        let one = constant(&mut first, Constant::Number(1.0));
        first.emit(Instruction::Constant(one));
        first.emit(Instruction::Return);

        let mut second = Chunk::default();
        second.emit(Instruction::Add);
        second.emit(Instruction::Return);

        let mut vm = Vm::default();
        assert_eq!(vm.execute(&first).unwrap(), JsValue::Number(1.0));
        let error = vm.execute(&second).unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Runtime);
        assert!(error.message.contains("invalid bytecode chunk"));
    }

    #[test]
    fn test262_error_call_throws_test262_vmerror() {
        // Calling Test262Error("msg") as a function propagates a VmError::test262
        // so the test runner can detect assertion failures.
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let ctor = constant(&mut chunk, Constant::String("Test262Error".into()));
        let msg = constant(&mut chunk, Constant::String("boom".into()));
        chunk.emit(Instruction::LoadGlobal(ctor));
        chunk.emit(Instruction::Constant(msg));
        chunk.emit(Instruction::Call(1));
        chunk.emit(Instruction::Return);

        let error = Vm::default()
            .execute_with_context(&chunk, &mut context)
            .unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Test262);
        assert!(error.message.contains("boom"));
    }

    #[test]
    fn test262_error_construct_returns_error_value() {
        // `new Test262Error("msg")` should return a JsValue::Error with Test262 kind.
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let ctor = constant(&mut chunk, Constant::String("Test262Error".into()));
        let msg = constant(&mut chunk, Constant::String("oops".into()));
        chunk.emit(Instruction::LoadGlobal(ctor));
        chunk.emit(Instruction::Constant(msg));
        chunk.emit(Instruction::Construct(1));
        chunk.emit(Instruction::Return);

        let result = Vm::default()
            .execute_with_context(&chunk, &mut context)
            .unwrap();
        assert!(matches!(result, JsValue::Error(_)));
    }

    #[test]
    fn missing_property_on_builtin_reads_as_undefined() {
        // Reading a non-existent property on a builtin object returns undefined.
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let ctor = constant(&mut chunk, Constant::String("Test262Error".into()));
        let missing = constant(&mut chunk, Constant::String("missing".into()));
        chunk.emit(Instruction::LoadGlobal(ctor));
        chunk.emit(Instruction::GetProperty(missing));
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default()
                .execute_with_context(&chunk, &mut context)
                .unwrap(),
            JsValue::Undefined
        );
    }

    #[test]
    fn unconditional_jump_skips_unreachable_branch() {
        let mut chunk = Chunk::default();
        let one = constant(&mut chunk, Constant::Number(1.0));
        let two = constant(&mut chunk, Constant::Number(2.0));

        chunk.emit(Instruction::Constant(one));
        chunk.emit(Instruction::Jump(3));
        chunk.emit(Instruction::Constant(two));
        chunk.emit(Instruction::Return);

        assert_eq!(Vm::default().execute(&chunk).unwrap(), JsValue::Number(1.0));
    }

    #[test]
    fn backward_jump_consumes_loop_budget() {
        let mut context = NativeContext::default();
        context.reset_execution_budget(2);

        let chunk = Chunk {
            instructions: vec![Instruction::Jump(0), Instruction::ReturnUndefined],
            constants: Vec::new(),
            functions: Vec::new(),
            handlers: Vec::new(),
            function_body_start: 0,
        };

        let error = Vm::default()
            .execute_with_context(&chunk, &mut context)
            .unwrap_err();
        assert_eq!(error.kind, VmErrorKind::RuntimeLimit);
    }

    #[test]
    fn typeof_global_missing_name_returns_undefined() {
        let mut chunk = Chunk::default();
        let missing = constant(&mut chunk, Constant::String("missingName".into()));
        chunk.emit(Instruction::TypeOfGlobal(missing));
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&chunk).unwrap(),
            JsValue::String("undefined".into())
        );
    }

    #[test]
    fn typeof_reports_v2_value_types() {
        let mut chunk = Chunk::default();
        let null = constant(&mut chunk, Constant::Null);
        chunk.emit(Instruction::Constant(null));
        chunk.emit(Instruction::TypeOf);
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default().execute(&chunk).unwrap(),
            JsValue::String("object".into())
        );
    }

    #[test]
    fn constructs_and_throws_minimal_test262_error() {
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let constructor = constant(&mut chunk, Constant::String("Test262Error".into()));
        let message = constant(&mut chunk, Constant::String("expected".into()));
        chunk.emit(Instruction::LoadGlobal(constructor));
        chunk.emit(Instruction::Constant(message));
        chunk.emit(Instruction::Construct(1));
        chunk.emit(Instruction::Throw);
        chunk.emit(Instruction::ReturnUndefined);

        let error = Vm::default()
            .execute_with_context(&chunk, &mut context)
            .unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Test262);
        assert_eq!(error.message, "expected");
    }

    #[test]
    fn throwing_primitive_reports_runtime_error_and_clears_stack() {
        let mut chunk = Chunk::default();
        let leftover = constant(&mut chunk, Constant::Number(1.0));
        let thrown = constant(&mut chunk, Constant::String("boom".into()));
        chunk.emit(Instruction::Constant(leftover));
        chunk.emit(Instruction::Constant(thrown));
        chunk.emit(Instruction::Throw);
        chunk.emit(Instruction::ReturnUndefined);

        let mut vm = Vm::default();
        let error = vm.execute(&chunk).unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Runtime);
        assert!(vm.stack.is_empty());
    }

    #[test]
    fn int32_conversion_uses_ecmascript_modulo_semantics() {
        assert_eq!(number_to_uint32(f64::NAN), 0);
        assert_eq!(number_to_uint32(f64::INFINITY), 0);
        assert_eq!(number_to_uint32(-0.0), 0);
        assert_eq!(number_to_uint32(4_294_967_295.0), u32::MAX);
        assert_eq!(number_to_uint32(4_294_967_296.0), 0);
        assert_eq!(number_to_uint32(1e30), 0);
        assert_eq!(number_to_int32(4_294_967_295.0), -1);
        assert_eq!(number_to_int32(4_294_967_296.0), 0);
        assert_eq!(number_to_int32(1e30), 0);
    }
}

# Native V8 Shared Interface

Native V8 freezes the shared contracts needed for three parallel feature tracks:
frontend unlockers, module runner infrastructure, and first-batch builtin
skeletons.

This document supplements `interface-spec.md` and the V2-V7 interface
documents. If a V8 contract conflicts with an older placeholder, the V8 contract
is authoritative for the new behavior.

## 1. Frontend AST Contract

V8-A may add AST forms for syntax that is currently rejected before execution.
The initial contract is intentionally minimal: represent the source shape
without forcing full spec-perfect runtime behavior in the first patch.

Required new or extended expression/statement forms:

```rust
pub enum Expression {
    TemplateLiteral(TemplateLiteral),
    Spread(Box<Expression>),
    Class(ClassExpression),
    // existing variants...
}

pub enum Statement {
    ClassDeclaration(ClassDeclaration),
    // existing variants...
}

pub struct TemplateLiteral {
    pub quasis: Vec<String>,
    pub expressions: Vec<Expression>,
}

pub struct ClassExpression {
    pub name: Option<String>,
    pub super_class: Option<Box<Expression>>,
    pub elements: Vec<ClassElement>,
}

pub struct ClassDeclaration {
    pub name: String,
    pub super_class: Option<Expression>,
    pub elements: Vec<ClassElement>,
}

pub enum ClassElement {
    Constructor(FunctionExpression),
    Method {
        name: PropertyName,
        function: FunctionExpression,
        is_static: bool,
    },
}
```

The concrete names may differ if existing AST conventions already provide a
better shape. The behavior contract is:

- parser errors must remain deterministic `SyntaxError`/parse errors, not
  panics;
- unsupported class features may return a clear compile error, but simple
  declarations, constructors, prototype methods, and static methods should
  lower through the native pipeline;
- ordinary untagged template literals lower to string concatenation using
  existing `ToString` behavior;
- tagged templates are not required in V8.

## 2. Spread, Rest, and Destructuring Contract

V8-A and V8-B share the following minimal runtime-facing contract:

```rust
pub enum FunctionParam {
    Simple(String),
    Rest(String),
    // existing or future default/destructuring forms...
}

pub enum BindingPattern {
    Identifier(String),
    Array(Vec<Option<BindingPattern>>),
    Object(Vec<(PropertyName, BindingPattern)>),
}
```

Rules:

- call spread consumes an iterable or array-like value through a shared runtime
  helper;
- rest parameters allocate a native Array object containing remaining
  arguments;
- simple destructuring binding may be implemented before full assignment-pattern
  semantics;
- unsupported nested patterns must fail clearly rather than silently binding
  wrong values.

Runtime helper shape:

```rust
impl NativeContext {
    pub fn collect_spread_arguments(&mut self, value: JsValue) -> Result<Vec<JsValue>, VmError>;
    pub fn create_rest_array(&mut self, args: &[JsValue]) -> Result<JsValue, VmError>;
}
```

If existing helper names already cover this behavior, V8 should reuse them
instead of adding duplicates.

## 3. Module Runner Contract

V8-B owns the runtime and runner infrastructure for modules. V8 does not need
complete cyclic live-binding semantics, but it must define a stable boundary.

```rust
pub enum SourceKind {
    Script,
    Module,
}

pub struct ModuleId(pub u32);

pub struct ModuleRecord {
    pub id: ModuleId,
    pub specifier: String,
    pub source_path: std::path::PathBuf,
    pub dependencies: Vec<String>,
    pub imports: Vec<ModuleImportBinding>,
    pub exports: Vec<ModuleExportBinding>,
}

pub struct ModuleImportBinding {
    pub source: String,
    pub imported_name: String,
    pub local_name: String,
}

pub struct ModuleExportBinding {
    pub export_name: String,
    pub local_name: Option<String>,
    pub source: Option<String>,
}

pub enum ModuleStatus {
    Parsed,
    Linked,
    Evaluated,
    Failed,
}
```

Required behavior:

- Test262 `flags: [module]` enters the native module path; implemented in the
  first V8-B pass;
- module code is strict by default; implemented in the first V8-B pass;
- module top-level `this` is `undefined`; implemented in the first V8-B pass;
- module diagnostics are reported separately from script diagnostics via
  `module mode` Test262 labels;
- relative specifiers are resolved from the importing file; implemented for
  `./` and `../` specifiers;
- the module registry prevents duplicate evaluation of the same normalized path;
- acyclic dependency graphs are the V8 target; cyclic execution may return a
  clear unsupported/runtime error until V9+.

Compiler/runtime boundary:

```rust
impl NativeContext {
    pub fn eval_module_source(
        &mut self,
        source: &str,
        path: &std::path::Path,
    ) -> Result<JsValue, VmError>;
}
```

The exact API may be adjusted to match the current backend entry points, but
the source kind must be explicit. Do not infer module mode from source text.

Current first-stage implementation lives in `src/runtime/module.rs` and
`src/backend/native.rs`. Import/export binding storage is intentionally still a
connector task until A-group import/export AST forms are available.

## 4. Builtin Skeleton Contract

V8-C may install skeleton globals for large builtin families, but skeletons must
be honest and descriptor-correct for the members they expose.

Rules:

- constructors and prototypes must use normal object-model APIs;
- `name`, `length`, prototype, static methods, and prototype methods should have
  standard descriptor flags for implemented members;
- unsupported methods should either be absent or throw a deliberate TypeError /
  unsupported error when called; they must not silently return bogus success;
- skeletons must not special-case Test262 paths.

Initial V8-C globals:

```text
ArrayBuffer
Float64Array
Uint8Array
Int32Array
Intl
Intl.DateTimeFormat
Intl.NumberFormat
Intl.Collator
```

Optional if cheap and low-conflict:

```text
DataView
Map
Set
Iterator
```

TypedArray storage may start as a minimal runtime substrate if B exposes one.
If storage is not ready, constructors may still exist but must fail honestly for
operations that require real indexed element behavior.

## 5. Test262 Host Contract

V8-C may extend `$262`, but only for helpers observed in the current analysis or
V8 focused suites.

Rules:

- host helpers are installed only in Test262 mode;
- host helper failures must be explicit;
- missing host capabilities should be documented in `reports/test262-analysis.md`;
- `$262` support must not leak into normal user scripts unless already part of
  the existing Test262 host configuration.

## 6. Merge Compatibility Contract

Shared-file changes must follow this order:

```text
interface docs
 -> AST/source-kind data shape
 -> runtime helper signatures
 -> per-track implementation
 -> Test262 runner/report updates
```

If two tracks need the same file, the interface document is updated first and a
small connector patch merges before feature branches continue.

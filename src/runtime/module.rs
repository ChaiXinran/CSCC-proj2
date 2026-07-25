use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::{EnvironmentId, JsValue};

/// Lifecycle used by dynamic import while a local module is being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleLoadState {
    New,
    Loading,
    Loaded,
    Evaluating,
    Evaluated,
    Failed,
}

/// Request data preserved at the NativeRuntime boundary for `import()`.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicImportRequest {
    pub specifier: String,
    pub referrer: Option<PathBuf>,
    pub attributes: Vec<(String, JsValue)>,
}

/// Promise settlement outcome used by hosts which need to observe an import.
#[derive(Debug, Clone, PartialEq)]
pub enum DynamicImportOutcome {
    Fulfilled(JsValue),
    Rejected(JsValue),
}

/// Stable numeric identity for a module record inside one native isolate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

/// First-stage V8 module record.
///
/// V8-B intentionally stores only loader/runtime infrastructure here. Full
/// import/export AST lowering and live binding semantics are later connector
/// work with A group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRecord {
    pub id: ModuleId,
    pub specifier: String,
    pub source_path: PathBuf,
    pub dependencies: Vec<String>,
    pub imports: Vec<ModuleImportBinding>,
    pub exports: Vec<ModuleExportBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImportBinding {
    pub source: String,
    pub imported_name: String,
    pub local_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleExportBinding {
    pub export_name: String,
    pub local_name: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleStatus {
    Unlinked,
    Linking,
    Linked,
    Evaluating,
    Evaluated,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleEvaluationState {
    Pending,
    Fulfilled,
    Rejected(JsValue),
}

/// Track the evaluation promise for a module.
/// Used by dynamic import and TLA to signal completion.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleEvaluationPromise {
    pub promise: JsValue,
    pub promise_id: Option<u32>,
}

#[derive(Debug, Default)]
pub struct ModuleRegistry {
    next_id: u32,
    records: HashMap<PathBuf, ModuleRecord>,
    statuses: HashMap<ModuleId, ModuleStatus>,
    evaluation_states: HashMap<ModuleId, ModuleEvaluationState>,
    evaluation_promises: HashMap<ModuleId, ModuleEvaluationPromise>,
    environments: HashMap<ModuleId, EnvironmentId>,
    namespaces: HashMap<ModuleId, JsValue>,
}

impl ModuleRegistry {
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[must_use]
    pub fn status_for_path(&self, path: &Path) -> Option<ModuleStatus> {
        let normalized = normalize_module_path(path);
        self.records
            .get(&normalized)
            .and_then(|record| self.statuses.get(&record.id).copied())
    }

    #[must_use]
    pub fn record_for_path(&self, path: &Path) -> Option<&ModuleRecord> {
        let normalized = normalize_module_path(path);
        self.records.get(&normalized)
    }

    #[must_use]
    pub fn evaluation_state_for_path(&self, path: &Path) -> Option<&ModuleEvaluationState> {
        let normalized = normalize_module_path(path);
        let id = self.records.get(&normalized)?.id;
        self.evaluation_states.get(&id)
    }

    pub fn ensure_record(&mut self, path: &Path) -> ModuleId {
        let normalized = normalize_module_path(path);
        if let Some(record) = self.records.get(&normalized) {
            return record.id;
        }

        let id = ModuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.records.insert(
            normalized.clone(),
            ModuleRecord {
                id,
                specifier: normalized.to_string_lossy().replace('\\', "/"),
                source_path: normalized,
                dependencies: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
            },
        );
        self.statuses.insert(id, ModuleStatus::Unlinked);
        self.evaluation_states
            .insert(id, ModuleEvaluationState::Pending);
        id
    }

    pub fn set_status(&mut self, id: ModuleId, status: ModuleStatus) {
        self.statuses.insert(id, status);
    }

    pub fn set_evaluation_state(&mut self, id: ModuleId, state: ModuleEvaluationState) {
        self.evaluation_states.insert(id, state);
    }

    pub fn set_environment(&mut self, id: ModuleId, environment: EnvironmentId) {
        self.environments.insert(id, environment);
    }

    #[must_use]
    pub fn environment(&self, id: ModuleId) -> Option<EnvironmentId> {
        self.environments.get(&id).copied()
    }

    #[must_use]
    pub fn is_module_environment(&self, environment: EnvironmentId) -> bool {
        self.environments
            .values()
            .any(|candidate| *candidate == environment)
    }

    pub fn set_namespace(&mut self, id: ModuleId, namespace: JsValue) {
        self.namespaces.insert(id, namespace);
    }

    #[must_use]
    pub fn namespace(&self, id: ModuleId) -> Option<JsValue> {
        self.namespaces.get(&id).cloned()
    }

    pub(crate) fn environments(&self) -> impl Iterator<Item = EnvironmentId> + '_ {
        self.environments.values().copied()
    }

    pub(crate) fn namespaces(&self) -> impl Iterator<Item = &JsValue> {
        self.namespaces.values()
    }

    pub fn namespace_exports_for_binding(
        &self,
        environment: EnvironmentId,
        local_name: &str,
    ) -> Vec<(JsValue, String)> {
        let Some(module_id) = self
            .environments
            .iter()
            .find_map(|(module, candidate)| (*candidate == environment).then_some(*module))
        else {
            return Vec::new();
        };
        let Some(namespace) = self.namespaces.get(&module_id).cloned() else {
            return Vec::new();
        };
        let Some(record) = self.records.values().find(|record| record.id == module_id) else {
            return Vec::new();
        };
        record
            .exports
            .iter()
            .filter(|binding| {
                binding.source.is_none()
                    && binding
                        .local_name
                        .as_deref()
                        .unwrap_or(&binding.export_name)
                        == local_name
            })
            .map(|binding| (namespace.clone(), binding.export_name.clone()))
            .collect()
    }

    pub fn set_metadata(
        &mut self,
        id: ModuleId,
        dependencies: Vec<String>,
        imports: Vec<ModuleImportBinding>,
        exports: Vec<ModuleExportBinding>,
    ) {
        if let Some(record) = self.records.values_mut().find(|record| record.id == id) {
            record.dependencies = dependencies;
            record.imports = imports;
            record.exports = exports;
        }
    }

    /// Set the evaluation promise for a module.
    /// Used by dynamic import and TLA to signal completion.
    pub fn set_evaluation_promise(&mut self, id: ModuleId, promise: ModuleEvaluationPromise) {
        self.evaluation_promises.insert(id, promise);
    }

    /// Get the evaluation promise for a module.
    pub fn evaluation_promise(&self, id: ModuleId) -> Option<&ModuleEvaluationPromise> {
        self.evaluation_promises.get(&id)
    }

    /// Check if the module's status has transitioned to a final state.
    pub fn is_final_status(status: ModuleStatus) -> bool {
        matches!(status, ModuleStatus::Evaluated | ModuleStatus::Failed)
    }

    /// Transition a module through its lifecycle states.
    /// Returns an error if the transition is invalid.
    pub fn transition_to(&mut self, id: ModuleId, new_status: ModuleStatus) -> Result<(), String> {
        let current = self.statuses.get(&id).copied().unwrap_or(ModuleStatus::Unlinked);
        let valid = match (current, new_status) {
            (ModuleStatus::Unlinked, ModuleStatus::Linking) => true,
            (ModuleStatus::Linking, ModuleStatus::Linked) => true,
            (ModuleStatus::Linked, ModuleStatus::Evaluating) => true,
            (ModuleStatus::Evaluating, ModuleStatus::Evaluated) => true,
            (_, ModuleStatus::Failed) => true, // can fail from any state
            _ => false,
        };
        if !valid {
            return Err(format!(
                "invalid module state transition: {current:?} -> {new_status:?}"
            ));
        }
        self.statuses.insert(id, new_status);
        Ok(())
    }
}

#[must_use]
pub fn normalize_module_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn resolve_module_specifier(importer_path: &Path, specifier: &str) -> Result<PathBuf, String> {
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return Err(format!(
            "unsupported module specifier `{specifier}`; V8 only supports relative paths"
        ));
    }

    let base = importer_path.parent().unwrap_or_else(|| Path::new(""));
    Ok(normalize_module_path(&base.join(specifier)))
}

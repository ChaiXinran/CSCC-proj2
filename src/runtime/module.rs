use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleStatus {
    Parsed,
    Linked,
    Evaluated,
    Failed,
}

#[derive(Debug, Default)]
pub struct ModuleRegistry {
    next_id: u32,
    records: HashMap<PathBuf, ModuleRecord>,
    statuses: HashMap<ModuleId, ModuleStatus>,
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
        self.statuses.insert(id, ModuleStatus::Parsed);
        id
    }

    pub fn set_status(&mut self, id: ModuleId, status: ModuleStatus) {
        self.statuses.insert(id, status);
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

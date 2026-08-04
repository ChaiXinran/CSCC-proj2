//! Opt-in host services. The default runtime exposes no filesystem access.

use std::{
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::{
    runtime::{JsValue, NativeContext, PropertyDescriptor},
    vm::{Vm, VmError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostLoadError {
    Disabled,
    InvalidPath(String),
    EscapesRoot(PathBuf),
    NotFound(PathBuf),
    Io(String),
}

impl fmt::Display for HostLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => write!(f, "host file loading is disabled"),
            Self::InvalidPath(path) => write!(f, "invalid host file path `{path}`"),
            Self::EscapesRoot(path) => write!(
                f,
                "host file path escapes resource root: {}",
                path.display()
            ),
            Self::NotFound(path) => write!(f, "host file not found: {}", path.display()),
            Self::Io(message) => f.write_str(message),
        }
    }
}

pub trait HostFileLoader: Send + Sync {
    fn read_text(&self, path: &Path) -> Result<Arc<str>, HostLoadError>;
}

#[derive(Clone, Default)]
pub struct HostServices {
    pub file_loader: Option<Arc<dyn HostFileLoader>>,
}

impl fmt::Debug for HostServices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostServices")
            .field(
                "file_loader",
                &self.file_loader.as_ref().map(|_| "installed"),
            )
            .finish()
    }
}

#[derive(Debug)]
pub struct RootedFileLoader {
    root: PathBuf,
}

impl RootedFileLoader {
    pub fn new(root: PathBuf) -> Result<Self, HostLoadError> {
        let root = fs::canonicalize(&root).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                HostLoadError::NotFound(root.clone())
            } else {
                HostLoadError::Io(format!(
                    "cannot open resource root {}: {error}",
                    root.display()
                ))
            }
        })?;
        if !root.is_dir() {
            return Err(HostLoadError::InvalidPath(root.display().to_string()));
        }
        Ok(Self { root })
    }

    fn resolve(&self, requested: &Path) -> Result<PathBuf, HostLoadError> {
        let raw = requested.to_string_lossy().replace('\\', "/");
        if raw.is_empty() || Path::new(&raw).is_absolute() {
            return Err(HostLoadError::InvalidPath(raw));
        }
        let relative = raw.strip_prefix("./").unwrap_or(&raw);
        if relative.is_empty()
            || Path::new(relative).components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(HostLoadError::EscapesRoot(requested.to_path_buf()));
        }
        let candidate = self.root.join(relative);
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                HostLoadError::NotFound(candidate.clone())
            } else {
                HostLoadError::Io(format!(
                    "cannot open host file {}: {error}",
                    candidate.display()
                ))
            }
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(HostLoadError::EscapesRoot(canonical));
        }
        Ok(canonical)
    }
}

impl HostFileLoader for RootedFileLoader {
    fn read_text(&self, path: &Path) -> Result<Arc<str>, HostLoadError> {
        let resolved = self.resolve(path)?;
        fs::read_to_string(&resolved)
            .map(|text| Arc::<str>::from(normalize_text_line_endings(text)))
            .map_err(|error| {
                HostLoadError::Io(format!(
                    "cannot read host file {} as UTF-8: {error}",
                    resolved.display()
                ))
            })
    }
}

fn normalize_text_line_endings(text: String) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text
    }
}

pub(crate) fn install_jetstream_host(context: &mut NativeContext) -> Result<(), VmError> {
    let read_file = context.register_builtin("readFile", 1, read_file, None)?;
    context.declare_global("readFile", read_file.clone());
    context.define_own_property(
        context.global_object(),
        "readFile".into(),
        PropertyDescriptor::data_with(read_file, true, false, true),
    )?;
    Ok(())
}

fn read_file(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let path = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    context
        .read_host_text(&path)
        .map(|source| JsValue::String(source.as_ref().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooted_loader_rejects_parent_traversal() {
        let loader = RootedFileLoader::new(std::env::current_dir().unwrap()).unwrap();
        assert!(matches!(
            loader.read_text(Path::new("../Cargo.toml")),
            Err(HostLoadError::EscapesRoot(_))
        ));
    }

    #[test]
    fn host_text_normalizes_windows_and_legacy_line_endings() {
        assert_eq!(
            normalize_text_line_endings("a\r\nb\rc\n".to_string()),
            "a\nb\nc\n"
        );
    }
}

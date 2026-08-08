//! Opt-in host services. The default runtime exposes no filesystem access.

use std::{
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::{
    builtins::stringify_json,
    runtime::{JsValue, NativeContext, ObjectId, ObjectKind, PropertyDescriptor, PropertyKind},
    vm::{Vm, VmError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEvent {
    /// Canonical JSON payload emitted by `agent.render(tree)`.
    pub payload: String,
}

pub trait AgentHost {
    fn render(&mut self, payload: String);
}

impl AgentHost for NativeContext {
    fn render(&mut self, payload: String) {
        self.push_render_event(payload);
    }
}

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

pub(crate) fn install_agent_host(
    context: &mut NativeContext,
    byte_limit: usize,
    depth_limit: usize,
) -> Result<(), VmError> {
    context.configure_render_limits(byte_limit, depth_limit);
    let render = context.register_builtin("render", 1, agent_render, None)?;
    if let Some(backing) = context.value_object(&render) {
        context.prevent_extensions(backing)?;
    }
    let agent = context.create_object([("render".into(), render)])?;
    let JsValue::Object(agent_id) = agent.clone() else {
        unreachable!()
    };
    let render_value = context.get_property(agent.clone(), "render")?;
    context.define_own_property(
        agent_id,
        "render".into(),
        PropertyDescriptor::data_with(render_value, false, true, false),
    )?;
    context.prevent_extensions(agent_id)?;
    context.declare_global("agent", agent.clone());
    context.define_own_property(
        context.global_object(),
        "agent".into(),
        PropertyDescriptor::data_with(agent, false, false, false),
    )?;
    Ok(())
}

fn agent_render(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let tree = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let JsValue::Object(root) = tree else {
        return Err(VmError::type_error(
            "agent.render expects a RenderTree object",
        ));
    };
    validate_render_tree(context, root)?;
    let payload = stringify_json(JsValue::Object(root), context)?
        .ok_or_else(|| VmError::type_error("RenderTree is not JSON-serializable"))?;
    let (byte_limit, _) = context.render_limits();
    if payload.len() > byte_limit {
        return Err(VmError::runtime_limit(format!(
            "RenderTree exceeds {byte_limit} byte limit"
        )));
    }
    context.render(payload);
    Ok(JsValue::Undefined)
}

fn validate_render_tree(context: &mut NativeContext, root: ObjectId) -> Result<(), VmError> {
    let type_value = context.get_property(JsValue::Object(root), "type")?;
    let JsValue::String(tree_type) = type_value else {
        return Err(VmError::type_error("RenderTree.type must be a string"));
    };
    if !matches!(
        tree_type.as_str(),
        "panel" | "text" | "metrics" | "statuses" | "table" | "list"
    ) {
        return Err(VmError::type_error(format!(
            "unsupported RenderTree type `{tree_type}`"
        )));
    }
    let (_, depth_limit) = context.render_limits();
    let mut visiting = std::collections::HashSet::new();
    validate_json_depth(
        context,
        JsValue::Object(root),
        1,
        depth_limit,
        &mut visiting,
    )
}

fn validate_json_depth(
    context: &NativeContext,
    value: JsValue,
    depth: usize,
    depth_limit: usize,
    visiting: &mut std::collections::HashSet<ObjectId>,
) -> Result<(), VmError> {
    let JsValue::Object(object) = value else {
        return Ok(());
    };
    if depth > depth_limit {
        return Err(VmError::runtime_limit(format!(
            "RenderTree exceeds depth limit {depth_limit}"
        )));
    }
    if !visiting.insert(object) {
        return Err(VmError::type_error("RenderTree contains a cycle"));
    }
    let object_value = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing RenderTree object"))?;
    let keys = match object_value.kind {
        ObjectKind::Array { .. } => (0..object_value.array_length().unwrap_or(0))
            .map(|index| index.to_string())
            .collect::<Vec<_>>(),
        _ => object_value.own_property_keys(),
    };
    for key in keys {
        let Some(descriptor) = context.get_own_property_descriptor(object, &key) else {
            continue;
        };
        if !descriptor.enumerable {
            continue;
        }
        if let PropertyKind::Data { value, .. } = descriptor.kind {
            validate_json_depth(context, value, depth + 1, depth_limit, visiting)?;
        }
    }
    visiting.remove(&object);
    Ok(())
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
        .map(|source| JsValue::String(source.into()))
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

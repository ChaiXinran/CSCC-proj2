//! ECMAScript Realm abstraction.
//!
//! A Realm represents a self-contained ECMAScript execution environment with
//! its own intrinsics, global object, global environment, and associated state.
//! This module provides the canonical entry point for realm operations,
//! delegating to `NativeContext` for storage and lifecycle management.
//!
//! The primary Realm infrastructure lives in `context.rs`; this module
//! re-exports shared types and adds higher-level realm utilities that builtins
//! and the VM can use without reaching into context internals.

use super::{JsValue, NativeContext, ObjectId};
use crate::vm::{Vm, VmError};

/// Re-export the Realm identifier from context.
pub use super::context::RealmId;

/// ECMAScript `GetActiveScriptOrModule` — returns the script/module realm.
/// For now, returns the current realm's global object.
pub fn get_active_realm_global(context: &NativeContext) -> ObjectId {
    context.global_object()
}

/// Create a new Realm with fresh global object and environment.
/// Returns the new RealmId.
pub fn create_new_realm(
    _vm: &mut Vm,
    _context: &mut NativeContext,
) -> Result<RealmId, VmError> {
    Err(VmError::runtime(
        "ShadowRealm creation not yet fully implemented",
    ))
}

/// ShadowRealm: GetWrappedValue — wraps a value for cross-realm transfer.
#[allow(dead_code)]
pub fn get_wrapped_value(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    value: JsValue,
    _caller_realm: ObjectId,
    _target_realm: ObjectId,
) -> Result<JsValue, VmError> {
    match value {
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            Err(VmError::type_error(
                "ShadowRealm object wrapping not yet implemented",
            ))
        }
        // Primitives pass through unchanged
        _ => Ok(value),
    }
}

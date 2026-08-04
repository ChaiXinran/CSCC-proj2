//! Stable VM invocation and ECMAScript-function instantiation contracts.
//!
//! The dispatch implementation remains in `interpreter.rs`; this module owns
//! the request/completion types shared by builtins, class lowering, and the VM.

use crate::{
    bytecode::{EnvironmentCapturePolicy, FunctionTemplate},
    runtime::{JsFunction, JsObject, JsValue, NativeContext, ObjectId, PropertyDescriptor},
};

use super::{Vm, VmError, interpreter::OperationResult};

/// The JavaScript completion produced by the VM invocation boundary.
///
/// Engine failures remain `Err(VmError)`; catchable JavaScript exceptions are
/// represented by `Throw` independently of the thrown value's runtime type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InvocationOutcome {
    Value(JsValue),
    Throw(JsValue),
}

impl InvocationOutcome {
    pub(crate) fn into_value(self) -> Result<JsValue, JsValue> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Throw(value) => Err(value),
        }
    }
}

impl From<OperationResult> for InvocationOutcome {
    fn from(result: OperationResult) -> Self {
        match result {
            OperationResult::Value(value) => Self::Value(value),
            OperationResult::Throw(value) => Self::Throw(value),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CallRequest {
    pub callee: JsValue,
    pub this_value: JsValue,
    pub arguments: Vec<JsValue>,
}

impl CallRequest {
    pub(crate) fn new(callee: JsValue, this_value: JsValue, arguments: Vec<JsValue>) -> Self {
        Self {
            callee,
            this_value,
            arguments,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConstructRequest {
    pub constructor: JsValue,
    pub arguments: Vec<JsValue>,
    pub new_target: JsValue,
}

impl ConstructRequest {
    pub(crate) fn ordinary(constructor: JsValue, arguments: Vec<JsValue>) -> Self {
        Self {
            new_target: constructor.clone(),
            constructor,
            arguments,
        }
    }

    pub(crate) fn with_new_target(
        constructor: JsValue,
        arguments: Vec<JsValue>,
        new_target: JsValue,
    ) -> Self {
        Self {
            constructor,
            arguments,
            new_target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionEnvironmentMode {
    FollowTemplate,
    Global,
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionInstantiationRequest {
    pub template: FunctionTemplate,
    pub environment_mode: FunctionEnvironmentMode,
    pub name_override: Option<String>,
    pub function_object_prototype: Option<ObjectId>,
}

impl Vm {
    /// Stable VM boundary for all JavaScript calls.
    pub(crate) fn invoke_call(
        &mut self,
        request: CallRequest,
        context: &mut NativeContext,
    ) -> Result<InvocationOutcome, VmError> {
        self.call_value(
            request.callee,
            request.this_value,
            request.arguments,
            context,
        )
        .map(Into::into)
    }

    /// Stable VM boundary for all JavaScript constructor calls.
    pub(crate) fn invoke_construct(
        &mut self,
        request: ConstructRequest,
        context: &mut NativeContext,
    ) -> Result<InvocationOutcome, VmError> {
        self.construct_value_with_new_target(
            request.constructor,
            request.arguments,
            request.new_target,
            context,
        )
        .map(Into::into)
    }

    /// The single VM entry point for creating an ECMAScript function object.
    pub(crate) fn instantiate_function(
        &mut self,
        request: FunctionInstantiationRequest,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let FunctionInstantiationRequest {
            template,
            environment_mode,
            name_override,
            function_object_prototype,
        } = request;

        let environment = match environment_mode {
            FunctionEnvironmentMode::Global => Some(context.global_environment()),
            FunctionEnvironmentMode::FollowTemplate => match template.environment_policy {
                EnvironmentCapturePolicy::None => None,
                EnvironmentCapturePolicy::CaptureCurrent => Some(context.current_environment()),
            },
        };
        let is_arrow = template.is_arrow;
        let is_async = template.is_async;
        let is_generator = template.is_generator;
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
        let is_strict = template.is_strict
            || matches!(environment_mode, FunctionEnvironmentMode::FollowTemplate)
                && context.strict();
        let id = context.allocate_function(JsFunction {
            name: name_override.or(template.name),
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
            local_layout: template.local_layout,
            dynamic_scope: template.dynamic_scope,
            lexical_this,
            lexical_new_target,
            home_object,
        })?;

        if let Some(prototype) = function_object_prototype {
            let object = context
                .function_object(id)
                .ok_or_else(|| VmError::runtime("missing function object"))?;
            context.set_prototype_of(object, Some(prototype))?;
        }

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
                    let next = context.register_builtin(
                        "next",
                        1,
                        super::interpreter::async_generator_next,
                        None,
                    )?;
                    let return_ = context.register_builtin(
                        "return",
                        1,
                        super::interpreter::async_generator_return,
                        None,
                    )?;
                    let throw = context.register_builtin(
                        "throw",
                        1,
                        super::interpreter::async_generator_throw,
                        None,
                    )?;
                    for (name, method) in [("next", next), ("return", return_), ("throw", throw)] {
                        context.define_own_property(
                            async_generator_prototype,
                            name.into(),
                            PropertyDescriptor::data_with(method, true, false, true),
                        )?;
                    }
                    context.define_symbol_own_property(
                        async_generator_prototype,
                        context.well_known_symbols().to_string_tag,
                        PropertyDescriptor::data_with(
                            JsValue::String("AsyncGenerator".into()),
                            false,
                            false,
                            true,
                        ),
                    )?;
                    let constructor = context.register_builtin(
                        "AsyncGenerator",
                        0,
                        async_generator_function_prototype_call,
                        None,
                    )?;
                    let constructor_object = context
                        .value_object(&constructor)
                        .ok_or_else(|| VmError::runtime("missing async generator constructor"))?;
                    context.define_own_property(
                        constructor_object,
                        "prototype".into(),
                        PropertyDescriptor::data_with(
                            JsValue::Object(async_generator_prototype),
                            false,
                            false,
                            false,
                        ),
                    )?;
                    context.define_own_property(
                        async_generator_prototype,
                        "constructor".into(),
                        PropertyDescriptor::data_with(constructor.clone(), false, false, true),
                    )?;
                    let function_object = context
                        .function_object(id)
                        .ok_or_else(|| VmError::runtime("missing async generator function"))?;
                    context.set_prototype_of(function_object, Some(constructor_object))?;
                } else {
                    context.set_prototype_of(generator_prototype, Some(iterator_prototype))?;
                }
            }
        }
        if is_strict {
            context.mark_strict_function(id);
            context.install_restricted_function_properties(id)?;
        }
        Ok(JsValue::Function(id))
    }
}

fn async_generator_function_prototype_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Undefined)
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

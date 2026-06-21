//! Bytecode interpreter.

use std::fmt;

use crate::{
    bytecode::{Chunk, Constant, EnvironmentCapturePolicy, Instruction},
    runtime::{
        FunctionId, JsFunction, JsValue, NativeContext, NativeErrorKind, ObjectId,
        PropertyDescriptor, PropertyKind, to_property_key,
    },
    vm::CallFrame,
};

/// Native VM failure category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmErrorKind {
    Reference,
    Type,
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
}

impl Vm {
    pub fn execute(&mut self, chunk: &Chunk) -> Result<JsValue, VmError> {
        self.execute_with_context(chunk, &mut NativeContext::default())
    }

    pub fn execute_with_context(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        self.stack.clear();
        let result = self.run(chunk, context);
        if result.is_err() {
            self.stack.clear();
        }
        result
    }

    fn run(&mut self, chunk: &Chunk, context: &mut NativeContext) -> Result<JsValue, VmError> {
        let mut instruction_pointer = 0;
        while instruction_pointer < chunk.instructions.len() {
            let current_instruction = instruction_pointer;
            let instruction = chunk.instructions[current_instruction];
            instruction_pointer += 1;

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
                    if !context.declare_global(name, value.clone()) {
                        context.set_global(name, value);
                    }
                }
                Instruction::LoadGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = context.get_global(name).ok_or_else(|| {
                        VmError::reference(format!(
                            "{name} is not defined at instruction {current_instruction}"
                        ))
                    })?;
                    self.stack.push(value);
                }
                Instruction::StoreGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = self.pop_value()?;
                    if !context.set_global(name, value.clone()) {
                        return Err(VmError::reference(format!(
                            "{name} is not defined at instruction {current_instruction}"
                        )));
                    }
                    self.stack.push(value);
                }
                Instruction::UnaryPlus => {
                    let value = self.pop_number()?;
                    self.stack.push(JsValue::Number(value));
                }
                Instruction::Negate => {
                    let value = self.pop_number()?;
                    self.stack.push(JsValue::Number(-value));
                }
                Instruction::LogicalNot => {
                    let value = self.pop_value()?;
                    self.stack.push(JsValue::Boolean(!value.to_boolean()));
                }
                Instruction::Add => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    self.stack.push(add_values(left, right)?);
                }
                Instruction::Subtract => {
                    let right = self.pop_number()?;
                    let left = self.pop_number()?;
                    self.stack.push(JsValue::Number(left - right));
                }
                Instruction::Multiply => {
                    let right = self.pop_number()?;
                    let left = self.pop_number()?;
                    self.stack.push(JsValue::Number(left * right));
                }
                Instruction::Divide => {
                    let right = self.pop_number()?;
                    let left = self.pop_number()?;
                    self.stack.push(JsValue::Number(left / right));
                }
                Instruction::Remainder => {
                    let right = self.pop_number()?;
                    let left = self.pop_number()?;
                    self.stack.push(JsValue::Number(left % right));
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
                    self.stack
                        .push(JsValue::Boolean(compare_values(left, right, |ordering| {
                            ordering.is_lt()
                        })?));
                }
                Instruction::LessThanOrEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    self.stack
                        .push(JsValue::Boolean(compare_values(left, right, |ordering| {
                            ordering.is_le()
                        })?));
                }
                Instruction::GreaterThan => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    self.stack
                        .push(JsValue::Boolean(compare_values(left, right, |ordering| {
                            ordering.is_gt()
                        })?));
                }
                Instruction::GreaterThanOrEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    self.stack
                        .push(JsValue::Boolean(compare_values(left, right, |ordering| {
                            ordering.is_ge()
                        })?));
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
                    let value = self.get_property_value(object, name, context)?;
                    self.stack.push(value);
                }
                Instruction::Call(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let callee = self.pop_value()?;
                    let result = self.call_value(callee, JsValue::Undefined, arguments, context)?;
                    self.stack.push(result);
                }
                Instruction::Construct(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let callee = self.pop_value()?;
                    let result = self.construct_value(callee, arguments, context)?;
                    self.stack.push(result);
                }
                Instruction::TypeOf => {
                    let value = self.pop_value()?;
                    self.stack.push(JsValue::String(value.type_of().into()));
                }
                Instruction::TypeOfGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let type_name = context
                        .get_global(name)
                        .map_or("undefined", |value| value.type_of());
                    self.stack.push(JsValue::String(type_name.into()));
                }
                Instruction::Throw => {
                    let value = self.pop_value()?;
                    return Err(throw_value(value));
                }
                Instruction::Return => return self.pop_value(),
                Instruction::ReturnUndefined => return Ok(JsValue::Undefined),
                Instruction::CreateFunction(function) => {
                    let value = self.create_function(chunk, function, context)?;
                    self.stack.push(value);
                }
                Instruction::DeclareFunction { name, function } => {
                    let name = self
                        .constant_string(chunk, name, current_instruction)?
                        .to_string();
                    let value = self.create_function(chunk, function, context)?;
                    context.declare_binding(context.current_environment(), name, value, true)?;
                }
                Instruction::DeclareLocal(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    context.declare_binding(context.current_environment(), name, value, true)?;
                }
                Instruction::LoadName(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = context
                        .resolve_binding(name)
                        .map(|(_, value)| value)
                        .ok_or_else(|| {
                            VmError::reference(format!(
                                "{name} is not defined at instruction {current_instruction}"
                            ))
                        })?;
                    self.stack.push(value);
                }
                Instruction::TypeOfName(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let type_name = context
                        .resolve_binding(name)
                        .map_or("undefined", |(_, value)| value.type_of());
                    self.stack.push(JsValue::String(type_name.into()));
                }
                Instruction::StoreName(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    context.set_binding(&name, value.clone())?;
                    self.stack.push(value);
                }
                Instruction::LoadThis => {
                    self.stack.push(context.current_this());
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
                        properties.push((key, value));
                    }
                    properties.reverse();
                    self.stack.push(context.create_object(properties)?);
                }
                Instruction::GetElement => {
                    let key = self.pop_value()?;
                    let object = self.pop_value()?;
                    let key = to_property_key(&key)?;
                    let value = self.get_property_value(object, &key, context)?;
                    self.stack.push(value);
                }
                Instruction::GetMethod(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let object = self.pop_value()?;
                    let method = self.get_property_value(object.clone(), name, context)?;
                    self.stack.push(method);
                    self.stack.push(object);
                }
                Instruction::SetProperty(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    let object = self.pop_value()?;
                    let result = self.set_property_value(object, &name, value, context)?;
                    self.stack.push(result);
                }
                Instruction::SetElement => {
                    let value = self.pop_value()?;
                    let key = self.pop_value()?;
                    let object = self.pop_value()?;
                    let key = to_property_key(&key)?;
                    let result = self.set_property_value(object, &key, value, context)?;
                    self.stack.push(result);
                }
                Instruction::CallWithThis(argument_count) => {
                    let arguments = self.pop_arguments(argument_count)?;
                    let this_value = self.pop_value()?;
                    let callee = self.pop_value()?;
                    let result = self.call_value(callee, this_value, arguments, context)?;
                    self.stack.push(result);
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
                    context.define_own_property(object, name, PropertyDescriptor::data(value))?;
                }
                Instruction::DefineGetter(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let getter = self.pop_value()?;
                    let object = context.require_object(self.peek_value()?, "define getter")?;
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
                    let getter = existing_accessor_getter(context, object, &name);
                    context.define_own_property(
                        object,
                        name,
                        PropertyDescriptor::accessor(getter, Some(setter), true, true),
                    )?;
                }
                Instruction::SetObjectPrototype => {
                    let prototype = self.pop_value()?;
                    let object = context.require_object(self.peek_value()?, "set prototype")?;
                    match prototype {
                        JsValue::Null => {
                            context.set_prototype_of(object, None)?;
                        }
                        JsValue::Object(prototype) => {
                            context.set_prototype_of(object, Some(prototype))?;
                        }
                        _ => {}
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
                    let object = context.require_object(&value, "delete property")?;
                    let deleted = context.delete_property(object, &name, context.strict())?;
                    self.stack.push(JsValue::Boolean(deleted));
                }
                Instruction::DeleteElement => {
                    let key = to_property_key(&self.pop_value()?)?;
                    let value = self.pop_value()?;
                    let object = context.require_object(&value, "delete property")?;
                    let deleted = context.delete_property(object, &key, context.strict())?;
                    self.stack.push(JsValue::Boolean(deleted));
                }
                Instruction::HasProperty => {
                    let value = self.pop_value()?;
                    let object = context.require_object(&value, "test property")?;
                    let key = to_property_key(&self.pop_value()?)?;
                    self.stack
                        .push(JsValue::Boolean(context.has_property(object, &key)?));
                }
                Instruction::InstanceOf => {
                    let constructor = self.pop_value()?;
                    let value = self.pop_value()?;
                    self.stack
                        .push(JsValue::Boolean(context.instance_of(value, constructor)?));
                }
            }
        }

        Err(VmError::runtime(
            "bytecode ended without a return instruction",
        ))
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

    fn pop_number(&mut self) -> Result<f64, VmError> {
        let value = self.pop_value()?;
        value
            .to_number()
            .ok_or_else(|| VmError::type_error("value cannot be converted to number in V1"))
    }

    fn pop_arguments(&mut self, count: u16) -> Result<Vec<JsValue>, VmError> {
        let mut arguments = Vec::with_capacity(count as usize);
        for _ in 0..count {
            arguments.push(self.pop_value()?);
        }
        arguments.reverse();
        Ok(arguments)
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
        let id = context.allocate_function(JsFunction {
            name: template.name,
            params: template.params,
            chunk: template.chunk,
            environment,
        })?;
        Ok(JsValue::Function(id))
    }

    fn call_value(
        &mut self,
        callee: JsValue,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match callee {
            JsValue::Function(function) => {
                self.call_user_function(function, this_value, arguments, context)
            }
            JsValue::BuiltinFunction(id) => {
                let is_function_prototype_call = context
                    .intrinsics()
                    .and_then(|intrinsics| {
                        context.get_own_property_descriptor(intrinsics.function_prototype, "call")
                    })
                    .and_then(|descriptor| descriptor.value_cloned())
                    == Some(JsValue::BuiltinFunction(id));
                let def = context
                    .builtin(id)
                    .ok_or_else(|| VmError::runtime("invalid builtin id"))?
                    .clone();
                if is_function_prototype_call {
                    let target = this_value;
                    let call_this = arguments.first().cloned().unwrap_or(JsValue::Undefined);
                    let forwarded = arguments.into_iter().skip(1).collect();
                    return self.call_value(target, call_this, forwarded, context);
                }
                (def.call)(context, this_value, &arguments)
            }
            other => Err(VmError::type_error(format!("{other} is not callable"))),
        }
    }

    fn get_property_value(
        &mut self,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let object = context.require_object(&receiver, "read property")?;
        let Some((_, descriptor)) = context.find_property_descriptor(object, key)? else {
            return Ok(JsValue::Undefined);
        };
        match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(value),
            PropertyKind::Accessor { get: None, .. } => Ok(JsValue::Undefined),
            PropertyKind::Accessor {
                get: Some(getter), ..
            } => self.call_value(getter, receiver, Vec::new(), context),
        }
    }

    fn set_property_value(
        &mut self,
        receiver: JsValue,
        key: &str,
        value: JsValue,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let object = context.require_object(&receiver, "write property")?;
        if let Some((_, descriptor)) = context.find_property_descriptor(object, key)? {
            match descriptor.kind {
                PropertyKind::Accessor {
                    set: Some(setter), ..
                } => {
                    self.call_value(setter, receiver, vec![value.clone()], context)?;
                    return Ok(value);
                }
                PropertyKind::Accessor { set: None, .. } => {
                    return Err(VmError::type_error("property setter is undefined"));
                }
                PropertyKind::Data { .. } => {}
            }
        }
        context.set_property(receiver, key, value)
    }

    fn call_user_function(
        &mut self,
        function_id: FunctionId,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        let function = context
            .function(function_id)
            .cloned()
            .ok_or_else(|| VmError::runtime("missing function value"))?;
        let stack_base = self.stack.len();
        let environment = context.push_environment(function.environment)?;

        for (index, parameter) in function.params.iter().enumerate() {
            let value = arguments.get(index).cloned().unwrap_or(JsValue::Undefined);
            if let Err(error) = context.declare_binding(environment, parameter.clone(), value, true)
            {
                let _ = context.pop_environment();
                return Err(error);
            }
        }

        if let Some(name) = &function.name
            && let Err(error) = context.declare_binding(
                environment,
                name.clone(),
                JsValue::Function(function_id),
                true,
            )
        {
            let _ = context.pop_environment();
            return Err(error);
        }

        let frame = CallFrame::new(Some(function_id), 0, environment, this_value, stack_base);
        if let Err(error) = context.push_call_frame(frame) {
            let _ = context.pop_environment();
            return Err(error);
        }

        let result = self.run(&function.chunk, context);
        self.stack.truncate(stack_base);
        let frame_result = context.pop_call_frame();
        let environment_result = context.pop_environment();

        match result {
            Err(error) => Err(error),
            Ok(value) => {
                frame_result?;
                environment_result?;
                Ok(value)
            }
        }
    }

    fn construct_value(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match constructor {
            JsValue::Function(function_id) => {
                let prototype = context.constructor_prototype(&JsValue::Function(function_id))?;
                let instance = context.ordinary_object_with_prototype(prototype)?;
                let result =
                    self.call_user_function(function_id, instance.clone(), arguments, context)?;
                if matches!(result, JsValue::Object(_)) {
                    Ok(result)
                } else {
                    Ok(instance)
                }
            }
            JsValue::BuiltinFunction(id) => {
                let def = context
                    .builtin(id)
                    .ok_or_else(|| VmError::runtime("invalid builtin id"))?
                    .clone();
                match def.construct {
                    Some(construct) => construct(context, &arguments, JsValue::BuiltinFunction(id)),
                    None => Err(VmError::type_error(format!(
                        "{} is not a constructor",
                        def.name
                    ))),
                }
            }
            other => Err(VmError::type_error(format!("{other} is not a constructor"))),
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

fn constant_to_value(constant: &Constant) -> JsValue {
    match constant {
        Constant::Undefined => JsValue::Undefined,
        Constant::Null => JsValue::Null,
        Constant::Boolean(value) => JsValue::Boolean(*value),
        Constant::Number(value) => JsValue::Number(*value),
        Constant::String(value) => JsValue::String(value.clone()),
    }
}

fn add_values(left: JsValue, right: JsValue) -> Result<JsValue, VmError> {
    if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
        let left = left
            .to_js_string()
            .ok_or_else(|| VmError::type_error("left operand cannot be converted to string"))?;
        let right = right
            .to_js_string()
            .ok_or_else(|| VmError::type_error("right operand cannot be converted to string"))?;
        return Ok(JsValue::String(format!("{left}{right}")));
    }

    let left = left
        .to_number()
        .ok_or_else(|| VmError::type_error("left operand cannot be converted to number"))?;
    let right = right
        .to_number()
        .ok_or_else(|| VmError::type_error("right operand cannot be converted to number"))?;
    Ok(JsValue::Number(left + right))
}

fn compare_values(
    left: JsValue,
    right: JsValue,
    predicate: impl FnOnce(std::cmp::Ordering) -> bool,
) -> Result<bool, VmError> {
    if let (JsValue::String(left), JsValue::String(right)) = (&left, &right) {
        return Ok(predicate(left.cmp(right)));
    }

    let left = left
        .to_number()
        .ok_or_else(|| VmError::type_error("left operand cannot be converted to number"))?;
    let right = right
        .to_number()
        .ok_or_else(|| VmError::type_error("right operand cannot be converted to number"))?;

    let Some(ordering) = left.partial_cmp(&right) else {
        return Ok(false);
    };
    Ok(predicate(ordering))
}

fn throw_value(value: JsValue) -> VmError {
    match value {
        JsValue::Error(error) if error.kind == NativeErrorKind::Test262 => {
            VmError::test262(error.message)
        }
        value => VmError::runtime(format!("uncaught {value}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        builtins,
        bytecode::{Chunk, Constant, Instruction},
        runtime::{JsValue, NativeContext},
        vm::VmErrorKind,
    };

    use super::Vm;

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
        };
        let error = Vm::default().execute(&chunk).unwrap_err();

        assert_eq!(error.kind, VmErrorKind::Runtime);
        assert!(error.message.contains("underflow"));
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
        let two = constant(&mut first, Constant::Number(2.0));
        first.emit(Instruction::Constant(one));
        first.emit(Instruction::Constant(two));
        first.emit(Instruction::Return);

        let mut second = Chunk::default();
        second.emit(Instruction::Add);
        second.emit(Instruction::Return);

        let mut vm = Vm::default();
        assert_eq!(vm.execute(&first).unwrap(), JsValue::Number(2.0));
        let error = vm.execute(&second).unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Runtime);
    }

    #[test]
    fn calls_minimal_test262_assert_same_value() {
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let assert_name = constant(&mut chunk, Constant::String("assert".into()));
        let same_value = constant(&mut chunk, Constant::String("sameValue".into()));
        let one = constant(&mut chunk, Constant::Number(1.0));

        chunk.emit(Instruction::LoadGlobal(assert_name));
        chunk.emit(Instruction::GetProperty(same_value));
        chunk.emit(Instruction::Constant(one));
        chunk.emit(Instruction::Constant(one));
        chunk.emit(Instruction::Call(2));
        chunk.emit(Instruction::Return);

        assert_eq!(
            Vm::default()
                .execute_with_context(&chunk, &mut context)
                .unwrap(),
            JsValue::Undefined
        );
    }

    #[test]
    fn missing_object_properties_read_as_undefined() {
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let assert_name = constant(&mut chunk, Constant::String("assert".into()));
        let missing = constant(&mut chunk, Constant::String("missing".into()));

        chunk.emit(Instruction::LoadGlobal(assert_name));
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
    fn reports_test262_assertion_failures() {
        let mut context = NativeContext::default();
        builtins::install_test262_harness(&mut context);

        let mut chunk = Chunk::default();
        let assert_name = constant(&mut chunk, Constant::String("assert".into()));
        let same_value = constant(&mut chunk, Constant::String("sameValue".into()));
        let one = constant(&mut chunk, Constant::Number(1.0));
        let two = constant(&mut chunk, Constant::Number(2.0));

        chunk.emit(Instruction::LoadGlobal(assert_name));
        chunk.emit(Instruction::GetProperty(same_value));
        chunk.emit(Instruction::Constant(one));
        chunk.emit(Instruction::Constant(two));
        chunk.emit(Instruction::Call(2));
        chunk.emit(Instruction::Return);

        let error = Vm::default()
            .execute_with_context(&chunk, &mut context)
            .unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Test262);
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
}

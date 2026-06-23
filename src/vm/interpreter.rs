//! Bytecode interpreter.

use std::fmt;

use crate::{
    bytecode::{
        Chunk, Constant, EnvironmentCapturePolicy, ExceptionHandler, HandlerKind, Instruction,
    },
    runtime::{
        FunctionId, JsFunction, JsValue, NativeContext, NativeErrorKind, ObjectId, PreferredType,
        PrimitiveValue, PropertyDescriptor, PropertyKind, to_property_key,
    },
    vm::{CallFrame, Completion},
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
    pending_exception: Option<JsValue>,
    finally_stack: Vec<Completion>,
}

#[derive(Debug, Clone, PartialEq)]
enum OperationResult {
    Value(JsValue),
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

    pub fn execute_with_context(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        self.stack.clear();
        self.pending_exception = None;
        self.finally_stack.clear();
        let result = self.run_completion(chunk, context);
        if result.is_err() {
            self.stack.clear();
            self.pending_exception = None;
            self.finally_stack.clear();
        }
        match result? {
            Completion::Normal(value) | Completion::Return(value) => Ok(value),
            Completion::Throw(value) => {
                self.stack.clear();
                self.pending_exception = None;
                self.finally_stack.clear();
                Err(throw_value(value))
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

    fn run_completion(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<Completion, VmError> {
        let mut instruction_pointer = 0;
        let baseline = RunBaseline {
            stack_depth: self.stack.len(),
            environment_depth: context.environment_depth(),
        };
        while instruction_pointer < chunk.instructions.len() {
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
                    if !context.declare_global(name, value.clone()) {
                        context.set_global(name, value);
                    }
                }
                Instruction::LoadGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    match context.get_global(name) {
                        Some(value) => self.stack.push(value),
                        None => {
                            let error = VmError::reference(format!(
                                "{name} is not defined at instruction {current_instruction}"
                            ));
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::StoreGlobal(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let value = self.pop_value()?;
                    if !context.set_global(name, value.clone()) {
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
                    let value = self.to_number(value, context)?;
                    self.stack.push(JsValue::Number(value));
                }
                Instruction::Negate => {
                    let value = self.pop_value()?;
                    let value = self.to_number(value, context)?;
                    self.stack.push(JsValue::Number(-value));
                }
                Instruction::LogicalNot => {
                    let value = self.pop_value()?;
                    self.stack.push(JsValue::Boolean(!value.to_boolean()));
                }
                Instruction::Add => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value = add_values(self, context, left, right)?;
                    self.stack.push(value);
                }
                Instruction::Subtract => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let right = self.to_number(right, context)?;
                    let left = self.to_number(left, context)?;
                    self.stack.push(JsValue::Number(left - right));
                }
                Instruction::Multiply => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let right = self.to_number(right, context)?;
                    let left = self.to_number(left, context)?;
                    self.stack.push(JsValue::Number(left * right));
                }
                Instruction::Divide => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let right = self.to_number(right, context)?;
                    let left = self.to_number(left, context)?;
                    self.stack.push(JsValue::Number(left / right));
                }
                Instruction::Remainder => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let right = self.to_number(right, context)?;
                    let left = self.to_number(left, context)?;
                    self.stack.push(JsValue::Number(left % right));
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
                    let value =
                        compare_values(self, context, left, right, |ordering| ordering.is_lt())?;
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::LessThanOrEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value =
                        compare_values(self, context, left, right, |ordering| ordering.is_le())?;
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::GreaterThan => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value =
                        compare_values(self, context, left, right, |ordering| ordering.is_gt())?;
                    self.stack.push(JsValue::Boolean(value));
                }
                Instruction::GreaterThanOrEqual => {
                    let right = self.pop_value()?;
                    let left = self.pop_value()?;
                    let value =
                        compare_values(self, context, left, right, |ordering| ordering.is_ge())?;
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
                    abrupt = Some(Completion::Throw(value));
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
                            let type_name = value.map_or("undefined", |(_, value)| value.type_of());
                            self.stack.push(JsValue::String(type_name.into()));
                        }
                        Err(error) => {
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                    }
                }
                Instruction::StoreName(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    let value = self.pop_value()?;
                    match context.set_binding(&name, value.clone()) {
                        Ok(()) => self.stack.push(value),
                        Err(error) => {
                            abrupt = Some(Completion::Throw(vm_error_to_value(error)));
                            discard_saved_finally = true;
                        }
                    }
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
                    match self.get_property_value_completion(object, &key, context)? {
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
                        JsValue::String(text) => (0..text.encode_utf16().count())
                            .map(|index| index.to_string())
                            .collect(),
                        _ => match context.value_object(&value) {
                            Some(object) => context.for_in_keys(object),
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
                    let key = to_property_key(&key)?;
                    match self.set_property_value(object, &key, value, context)? {
                        OperationResult::Value(result) => self.stack.push(result),
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
                Instruction::CreateLexicalEnvironment => {
                    context.push_environment(Some(context.current_environment()))?;
                }
                Instruction::PopEnvironment => {
                    context.pop_environment()?;
                }
                Instruction::CreateMutableBinding(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    context.create_mutable_binding(context.current_environment(), name, false)?;
                }
                Instruction::CreateImmutableBinding(index) => {
                    let name = self
                        .constant_string(chunk, index, current_instruction)?
                        .to_string();
                    context.create_immutable_binding(context.current_environment(), name)?;
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
    ) -> Result<OperationResult, VmError> {
        match callee {
            JsValue::Function(function) => {
                self.call_user_function(function, this_value, arguments, context)
            }
            JsValue::BuiltinFunction(id) => {
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
                    let call_this = arguments.first().cloned().unwrap_or(JsValue::Undefined);
                    let forwarded = arguments.into_iter().skip(1).collect();
                    return self.call_value(target, call_this, forwarded, context);
                }
                match (def.call)(self, context, this_value, &arguments) {
                    Ok(value) => Ok(OperationResult::Value(value)),
                    Err(error) => match self.pending_exception.take() {
                        // A nested JavaScript callback threw; surface its value.
                        Some(value) => Ok(OperationResult::Throw(value)),
                        // ECMAScript error types raised directly by a builtin are
                        // catchable throws; engine-internal failures are not.
                        None => match error.kind {
                            VmErrorKind::Reference | VmErrorKind::Type | VmErrorKind::Range => {
                                Ok(OperationResult::Throw(vm_error_to_value(error)))
                            }
                            _ => Err(error),
                        },
                    },
                }
            }
            other => Err(VmError::type_error(format!("{other} is not callable"))),
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

    pub(crate) fn call_value_threw(
        &mut self,
        callee: JsValue,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> bool {
        !matches!(
            self.call_value(callee, this_value, arguments, context),
            Ok(OperationResult::Value(_))
        )
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
    /// primitive. For objects, invokes the `valueOf`/`toString` methods in the
    /// order dictated by `hint`.
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
            JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
                let prim = self.to_primitive(value, PreferredType::Number, context)?;
                self.to_number(prim, context)
            }
            JsValue::Error(_) => Ok(f64::NAN),
        }
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
            JsValue::String(s) => Ok(s),
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

    pub(crate) fn get_property_value(
        &mut self,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<JsValue, VmError> {
        match self.get_property_value_completion(receiver, key, context)? {
            OperationResult::Value(value) => Ok(value),
            OperationResult::Throw(value) => Err(throw_value(value)),
        }
    }

    fn get_property_value_completion(
        &mut self,
        receiver: JsValue,
        key: &str,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        // Primitive strings expose `length` and UTF-16 code-unit indexing
        // without boxing; method lookups fall through to String.prototype.
        if let JsValue::String(value) = &receiver {
            if key == "length" {
                return Ok(OperationResult::Value(JsValue::Number(
                    value.encode_utf16().count() as f64,
                )));
            }
            if let Ok(index) = key.parse::<usize>() {
                return Ok(OperationResult::Value(
                    value
                        .encode_utf16()
                        .nth(index)
                        .map_or(JsValue::Undefined, |unit| {
                            JsValue::String(String::from_utf16_lossy(&[unit]))
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
            JsValue::Boolean(_) => context
                .boolean_prototype()
                .ok_or_else(|| VmError::type_error("cannot read property on boolean")),
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
        let object = context.require_object(&receiver, "write property")?;
        if let Some((_, descriptor)) = context.find_property_descriptor(object, key)? {
            match descriptor.kind {
                PropertyKind::Accessor {
                    set: Some(setter), ..
                } => match self.call_value(setter, receiver, vec![value.clone()], context)? {
                    OperationResult::Value(_) => return Ok(OperationResult::Value(value)),
                    OperationResult::Throw(thrown) => return Ok(OperationResult::Throw(thrown)),
                },
                PropertyKind::Accessor { set: None, .. } => {
                    return Err(VmError::type_error("property setter is undefined"));
                }
                PropertyKind::Data { .. } => {}
            }
        }
        context
            .set_property(receiver, key, value)
            .map(OperationResult::Value)
    }

    fn call_user_function(
        &mut self,
        function_id: FunctionId,
        this_value: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        let function = context
            .function(function_id)
            .cloned()
            .ok_or_else(|| VmError::runtime("missing function value"))?;
        let stack_base = self.stack.len();
        let caller_environment_depth = context.environment_depth();
        let environment = context.push_environment(function.environment)?;

        for (index, parameter) in function.params.iter().enumerate() {
            let value = arguments.get(index).cloned().unwrap_or(JsValue::Undefined);
            if let Err(error) = context.declare_binding(environment, parameter.clone(), value, true)
            {
                let _ = context.restore_environment_depth(caller_environment_depth);
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
            let _ = context.restore_environment_depth(caller_environment_depth);
            return Err(error);
        }

        let frame = CallFrame::new(Some(function_id), 0, environment, this_value, stack_base);
        if let Err(error) = context.push_call_frame(frame) {
            let _ = context.restore_environment_depth(caller_environment_depth);
            return Err(error);
        }

        let result = self.run_completion(&function.chunk, context);
        self.stack.truncate(stack_base);
        let frame_result = context.pop_call_frame();
        let environment_result = context.restore_environment_depth(caller_environment_depth);

        match result {
            Err(error) => Err(error),
            Ok(completion) => {
                frame_result?;
                environment_result?;
                match completion {
                    Completion::Normal(value) | Completion::Return(value) => {
                        Ok(OperationResult::Value(value))
                    }
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
        }
    }

    fn construct_value(
        &mut self,
        constructor: JsValue,
        arguments: Vec<JsValue>,
        context: &mut NativeContext,
    ) -> Result<OperationResult, VmError> {
        match constructor {
            JsValue::Function(function_id) => {
                let prototype = context.constructor_prototype(&JsValue::Function(function_id))?;
                let instance = context.ordinary_object_with_prototype(prototype)?;
                match self.call_user_function(function_id, instance.clone(), arguments, context)? {
                    OperationResult::Value(result) if matches!(result, JsValue::Object(_)) => {
                        Ok(OperationResult::Value(result))
                    }
                    OperationResult::Value(_) => Ok(OperationResult::Value(instance)),
                    OperationResult::Throw(value) => Ok(OperationResult::Throw(value)),
                }
            }
            JsValue::BuiltinFunction(id) => {
                let def = context
                    .builtin(id)
                    .ok_or_else(|| VmError::runtime("invalid builtin id"))?
                    .clone();
                // `new boundFn(...)` constructs the target with the bound
                // arguments prepended (the bound `this` is ignored for `new`).
                if let Some(bound) = &def.bound {
                    let mut forwarded = bound.args.clone();
                    forwarded.extend(arguments);
                    let target = bound.target.clone();
                    return self.construct_value(target, forwarded, context);
                }
                match def.construct {
                    Some(construct) => {
                        construct(self, context, &arguments, JsValue::BuiltinFunction(id))
                            .map(OperationResult::Value)
                    }
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
        return Ok(JsValue::String(format!("{left}{right}")));
    }

    let left = vm.to_number(left, context)?;
    let right = vm.to_number(right, context)?;
    Ok(JsValue::Number(left + right))
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

    let left = vm.to_number(left, context)?;
    let right = vm.to_number(right, context)?;

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
        JsValue::Error(error) if error.kind == NativeErrorKind::Reference => {
            VmError::reference(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::Type => {
            VmError::type_error(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::Range => {
            VmError::range(error.message)
        }
        JsValue::Error(error) if error.kind == NativeErrorKind::RuntimeLimit => {
            VmError::runtime_limit(error.message)
        }
        value => VmError::runtime(format!("uncaught {value}")),
    }
}

fn vm_error_to_value(error: VmError) -> JsValue {
    let kind = match error.kind {
        VmErrorKind::Reference => NativeErrorKind::Reference,
        VmErrorKind::Type => NativeErrorKind::Type,
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
            .unwrap_or_else(|| trimmed.parse::<f64>().unwrap_or(f64::NAN)),
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
            handlers: Vec::new(),
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
            handlers: Vec::new(),
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

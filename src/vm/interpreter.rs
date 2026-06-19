//! Bytecode interpreter.

use std::fmt;

use crate::{
    builtins,
    bytecode::{Chunk, Constant, Instruction},
    runtime::{Heap, JsValue, NativeContext, ObjectId},
};

/// Native VM failure category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmErrorKind {
    Reference,
    Type,
    Range,
    Test262,
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
                    context.declare_global(name, JsValue::Undefined);
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
                        instruction_pointer = target;
                    }
                }
                Instruction::JumpIfTrue(target) => {
                    self.validate_jump_target(target, chunk, current_instruction)?;
                    if self.peek_value()?.to_boolean() {
                        instruction_pointer = target;
                    }
                }
                Instruction::GetProperty(index) => {
                    let name = self.constant_string(chunk, index, current_instruction)?;
                    let object = self.pop_value()?;
                    let value = get_property(context.heap(), object, name, current_instruction)?;
                    self.stack.push(value);
                }
                Instruction::Call(argument_count) => {
                    let mut arguments = Vec::with_capacity(argument_count as usize);
                    for _ in 0..argument_count {
                        arguments.push(self.pop_value()?);
                    }
                    arguments.reverse();

                    let callee = self.pop_value()?;
                    let result = call_value(callee, arguments)?;
                    self.stack.push(result);
                }
                Instruction::Return => return self.pop_value(),
                Instruction::ReturnUndefined => return Ok(JsValue::Undefined),
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

fn get_property(
    heap: &Heap,
    object: JsValue,
    name: &str,
    instruction_pointer: usize,
) -> Result<JsValue, VmError> {
    let JsValue::Object(id) = object else {
        return Err(VmError::type_error(format!(
            "cannot read property {name} at instruction {instruction_pointer}"
        )));
    };

    Ok(get_object_property(heap, id, name).unwrap_or(JsValue::Undefined))
}

fn get_object_property(heap: &Heap, id: ObjectId, name: &str) -> Option<JsValue> {
    let object = heap.object(id)?;
    object.get_own_property_value(name).or_else(|| {
        object
            .prototype
            .and_then(|prototype| get_object_property(heap, prototype, name))
    })
}

fn call_value(callee: JsValue, arguments: Vec<JsValue>) -> Result<JsValue, VmError> {
    match callee {
        JsValue::NativeFunction(function) => builtins::call_native(function, arguments),
        other => Err(VmError::type_error(format!("{other} is not callable"))),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        builtins,
        bytecode::{Chunk, Constant, Instruction},
        runtime::{JsValue, NativeContext},
        vm::{Vm, VmErrorKind},
    };

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
    #[ignore = "all V1 instructions are now implemented; this placeholder test is no longer valid"]
    fn reports_v1_instructions_not_implemented_by_vm_yet() {
        let chunk = Chunk {
            instructions: vec![Instruction::Pop, Instruction::ReturnUndefined],
            constants: Vec::new(),
        };
        let error = Vm::default().execute(&chunk).unwrap_err();

        assert!(error.message.contains("Pop"));
        assert!(error.message.contains("not implemented"));
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

        chunk.emit(Instruction::DeclareGlobal(name));
        chunk.emit(Instruction::Constant(value));
        chunk.emit(Instruction::StoreGlobal(name));
        chunk.emit(Instruction::Pop);
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
}

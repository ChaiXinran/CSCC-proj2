//! Bytecode interpreter.

use std::fmt;

use crate::{
    bytecode::{Chunk, Constant, Instruction},
    runtime::JsValue,
};

/// Native VM failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub message: String,
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
        self.stack.clear();

        for instruction in &chunk.instructions {
            match *instruction {
                Instruction::Constant(index) => {
                    let constant = chunk.constants.get(index as usize).ok_or_else(|| VmError {
                        message: format!("constant index {index} is out of bounds"),
                    })?;
                    self.stack.push(constant_to_value(constant));
                }
                Instruction::Add => {
                    let right = self.pop_number()?;
                    let left = self.pop_number()?;
                    self.stack.push(JsValue::Number(left + right));
                }
                Instruction::Return => {
                    return self.stack.pop().ok_or_else(|| VmError {
                        message: "return requires a value on the stack".into(),
                    });
                }
                Instruction::ReturnUndefined => return Ok(JsValue::Undefined),
                unsupported => {
                    return Err(VmError {
                        message: format!(
                            "instruction {unsupported:?} is not implemented by the VM"
                        ),
                    });
                }
            }
        }

        Err(VmError {
            message: "bytecode ended without a return instruction".into(),
        })
    }

    fn pop_number(&mut self) -> Result<f64, VmError> {
        match self.stack.pop() {
            Some(JsValue::Number(value)) => Ok(value),
            Some(_) => Err(VmError {
                message: "numeric instruction received a non-number value".into(),
            }),
            None => Err(VmError {
                message: "operand stack underflow".into(),
            }),
        }
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

#[cfg(test)]
mod tests {
    use crate::{
        bytecode::{Chunk, Constant, Instruction},
        runtime::JsValue,
    };

    use super::Vm;

    #[test]
    fn executes_hand_written_addition_bytecode() {
        let mut chunk = Chunk::default();
        let left = chunk.add_constant(Constant::Number(1.0)).unwrap();
        let right = chunk.add_constant(Constant::Number(2.0)).unwrap();
        chunk.emit(Instruction::Constant(left));
        chunk.emit(Instruction::Constant(right));
        chunk.emit(Instruction::Add);
        chunk.emit(Instruction::Return);

        assert_eq!(Vm::default().execute(&chunk).unwrap(), JsValue::Number(3.0));
    }

    #[test]
    fn reports_v1_instructions_not_implemented_by_vm_yet() {
        let chunk = Chunk {
            instructions: vec![Instruction::Pop, Instruction::ReturnUndefined],
            constants: Vec::new(),
        };
        let error = Vm::default().execute(&chunk).unwrap_err();

        assert!(error.message.contains("Pop"));
        assert!(error.message.contains("not implemented"));
    }
}

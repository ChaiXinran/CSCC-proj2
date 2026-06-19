//! Stable collaboration contracts for the self-developed engine.
//!
//! Cross-module code should import shared types from this module instead of
//! reaching into another team's implementation files. Keep this file small and
//! change its public interfaces only after team review.

use std::{error::Error, fmt};

pub use crate::{
    ast::{BinaryOperator, Expression, Literal, Program, Statement, UnaryOperator, VariableKind},
    bytecode::{
        Chunk, ChunkError, CompileError, Compiler, Constant, Instruction, StackAnalysis,
        StackEffect,
    },
    lexer::{Keyword, LexError, Span, Token, TokenKind},
    parser::{ParseError, Parser},
    runtime::{
        Binding, CollectionStats, Collector, Environment, EnvironmentId, Heap, JsObject, JsValue,
        NativeContext, ObjectId, PropertyDescriptor,
    },
    vm::{CallFrame, Vm, VmError},
};

/// Unified failure type passed between native engine stages.
#[derive(Debug)]
pub enum NativeError {
    Lex(LexError),
    Parse(ParseError),
    Compile(CompileError),
    Execute(VmError),
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lex(error) => write!(f, "lex error: {error}"),
            Self::Parse(error) => write!(f, "parse error: {error}"),
            Self::Compile(error) => write!(f, "compile error: {error}"),
            Self::Execute(error) => write!(f, "execution error: {error}"),
        }
    }
}

impl Error for NativeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Lex(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::Compile(error) => Some(error),
            Self::Execute(error) => Some(error),
        }
    }
}

impl From<LexError> for NativeError {
    fn from(error: LexError) -> Self {
        Self::Lex(error)
    }
}

impl From<ParseError> for NativeError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<CompileError> for NativeError {
    fn from(error: CompileError) -> Self {
        Self::Compile(error)
    }
}

impl From<VmError> for NativeError {
    fn from(error: VmError) -> Self {
        Self::Execute(error)
    }
}

/// Front-end contract owned by the lexer/parser team.
pub trait SourceParser {
    fn parse_source(&mut self, source: &str) -> Result<Program, NativeError>;
}

/// Compiler contract owned by the bytecode team.
pub trait ProgramCompiler {
    fn compile_program(&mut self, program: &Program) -> Result<Chunk, NativeError>;
}

/// Execution contract owned by the VM/runtime team.
pub trait ChunkExecutor {
    fn execute_chunk(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<JsValue, NativeError>;
}

/// Default adapter joining the native lexer and parser.
#[derive(Debug, Default)]
pub struct NativeFrontend;

impl SourceParser for NativeFrontend {
    fn parse_source(&mut self, source: &str) -> Result<Program, NativeError> {
        let tokens = crate::lexer::Lexer::new(source).tokenize()?;
        Ok(Parser::new(tokens).parse_program()?)
    }
}

impl ProgramCompiler for Compiler {
    fn compile_program(&mut self, program: &Program) -> Result<Chunk, NativeError> {
        Ok(Compiler::compile_program(self, program)?)
    }
}

impl ChunkExecutor for Vm {
    fn execute_chunk(
        &mut self,
        chunk: &Chunk,
        _context: &mut NativeContext,
    ) -> Result<JsValue, NativeError> {
        Ok(self.execute(chunk)?)
    }
}

/// Replaceable source-to-value pipeline used by `NativeRuntime`.
///
/// Generic stages let each team substitute a fake upstream or downstream
/// implementation in unit tests without depending on unfinished modules.
#[derive(Debug)]
pub struct NativePipeline<P = NativeFrontend, C = Compiler, E = Vm> {
    pub frontend: P,
    pub compiler: C,
    pub executor: E,
}

impl Default for NativePipeline {
    fn default() -> Self {
        Self {
            frontend: NativeFrontend,
            compiler: Compiler,
            executor: Vm::default(),
        }
    }
}

impl<P, C, E> NativePipeline<P, C, E>
where
    P: SourceParser,
    C: ProgramCompiler,
    E: ChunkExecutor,
{
    #[must_use]
    pub const fn from_stages(frontend: P, compiler: C, executor: E) -> Self {
        Self {
            frontend,
            compiler,
            executor,
        }
    }

    pub fn parse(&mut self, source: &str) -> Result<Program, NativeError> {
        self.frontend.parse_source(source)
    }

    pub fn compile(&mut self, program: &Program) -> Result<Chunk, NativeError> {
        self.compiler.compile_program(program)
    }

    pub fn execute(
        &mut self,
        chunk: &Chunk,
        context: &mut NativeContext,
    ) -> Result<JsValue, NativeError> {
        self.executor.execute_chunk(chunk, context)
    }

    pub fn evaluate(
        &mut self,
        source: &str,
        context: &mut NativeContext,
    ) -> Result<JsValue, NativeError> {
        let program = self.parse(source)?;
        let chunk = self.compile(&program)?;
        self.execute(&chunk, context)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Chunk, ChunkExecutor, Instruction, JsValue, NativeContext, NativeError, NativePipeline,
        Program, ProgramCompiler, SourceParser,
    };

    #[test]
    fn default_pipeline_executes_empty_program() {
        assert_eq!(
            NativePipeline::default()
                .evaluate("", &mut NativeContext::default())
                .unwrap(),
            JsValue::Undefined
        );
    }

    #[test]
    fn stages_can_be_replaced_for_isolated_tests() {
        struct FakeFrontend;
        struct FakeCompiler;
        struct FakeExecutor;

        impl SourceParser for FakeFrontend {
            fn parse_source(&mut self, _source: &str) -> Result<Program, NativeError> {
                Ok(Program::default())
            }
        }

        impl ProgramCompiler for FakeCompiler {
            fn compile_program(&mut self, _program: &Program) -> Result<Chunk, NativeError> {
                Ok(Chunk {
                    instructions: vec![Instruction::ReturnUndefined],
                    constants: Vec::new(),
                })
            }
        }

        impl ChunkExecutor for FakeExecutor {
            fn execute_chunk(
                &mut self,
                _chunk: &Chunk,
                context: &mut NativeContext,
            ) -> Result<JsValue, NativeError> {
                context.push_output("executed");
                Ok(JsValue::Number(42.0))
            }
        }

        let mut pipeline = NativePipeline::from_stages(FakeFrontend, FakeCompiler, FakeExecutor);
        let mut context = NativeContext::default();
        assert_eq!(
            pipeline.evaluate("ignored", &mut context).unwrap(),
            JsValue::Number(42.0)
        );
        assert_eq!(context.take_output(), ["executed"]);
    }
}

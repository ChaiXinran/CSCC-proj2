//! AgentJS virtual machine instructions.

/// Number of stack values consumed and produced by one instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackEffect {
    /// Minimum stack depth required before the instruction executes.
    pub required: u32,
    pub pops: u32,
    pub pushes: u32,
}

impl StackEffect {
    #[must_use]
    pub const fn new(pops: u32, pushes: u32) -> Self {
        Self {
            required: pops,
            pops,
            pushes,
        }
    }

    #[must_use]
    pub const fn with_required(required: u32, pops: u32, pushes: u32) -> Self {
        Self {
            required,
            pops,
            pushes,
        }
    }
}

/// One decoded bytecode instruction.
///
/// Constant and name operands are indexes into [`super::Chunk::constants`].
/// Function operands are indexes into [`super::Chunk::functions`].
/// Jump operands are absolute instruction offsets inside the same chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    // -----------------------------------------------------------------------
    // V1/V2 instructions
    // -----------------------------------------------------------------------
    Constant(u16),
    Pop,
    /// Duplicates the top stack value without consuming it.
    Duplicate,
    /// Duplicates the top two stack values while preserving their order.
    DuplicatePair,
    /// Swaps the top two stack values. [a, b] → [b, a].
    Swap,
    /// Sets the `name` property of the top-of-stack function to the given constant string,
    /// if the function's name is currently empty (anonymous inference). Value stays on stack.
    SetFunctionName(u16),

    DeclareGlobal(u16),
    LoadGlobal(u16),
    /// Stores the top value and leaves that value on the stack.
    StoreGlobal(u16),

    UnaryPlus,
    Increment,
    Decrement,
    Negate,
    LogicalNot,
    TypeOf,
    TypeOfGlobal(u16),

    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Exponentiation,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNot,
    LeftShift,
    RightShift,
    UnsignedRightShift,

    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    /// Observes, but does not remove, the top stack value.
    JumpIfFalse(usize),
    /// Observes, but does not remove, the top stack value.
    JumpIfTrue(usize),
    /// Observes, but does not remove, the top stack value; jumps if NOT null or undefined.
    JumpIfNotNullish(usize),
    /// Observes, but does not remove, the top stack value; jumps if NOT undefined (null does NOT trigger).
    JumpIfNotUndefined(usize),
    /// Unconditionally transfers control to an absolute instruction offset.
    Jump(usize),

    GetProperty(u16),
    /// Pops the callee and `argument_count` arguments, then pushes the result.
    Call(u16),
    /// Pops the constructor and `argument_count` arguments, then pushes the constructed value.
    Construct(u16),

    Throw,
    Return,
    ReturnUndefined,

    // -----------------------------------------------------------------------
    // V3 instructions
    // -----------------------------------------------------------------------
    /// Creates a function value from the function constant table and pushes it.
    /// Operand is an index into [`super::Chunk::functions`].
    /// Stack: [] → [fn_value]
    CreateFunction(u16),

    /// Declares a function binding in the current environment without leaving a
    /// value on the stack. Operand `name` is a string constant index;
    /// `function` is a function table index.
    /// Stack: [] → []
    DeclareFunction {
        name: u16,
        function: u16,
    },

    /// Declares a local `var` binding in the current function environment by
    /// popping the initializer off the stack.
    /// Stack: [value] → []
    DeclareLocal(u16),

    /// Looks up a name along the environment chain and pushes its value.
    /// Returns `ReferenceError` if not found.
    /// Stack: [] → [value]
    LoadName(u16),

    /// Like `LoadName` but never throws: used for `typeof undeclared`.
    /// Stack: [] → [typeof_string]
    TypeOfName(u16),

    /// Writes a value to an existing binding along the environment chain,
    /// leaving the value on the stack (like `StoreGlobal`).
    /// Stack: [value] → [value]
    StoreName(u16),

    /// Pushes the `this` value of the current call frame.
    /// Stack: [] → [this_value]
    LoadThis,

    /// Pushes the `new.target` value of the current call frame.
    /// Returns `undefined` in regular calls; the constructor function in `new` calls.
    /// Stack: [] → [new_target]
    LoadNewTarget,

    /// Pops `n` elements (left-to-right order on stack) and creates an array.
    /// Stack: [e0, e1, ..., en-1] → [array]
    ArrayCreate(u16),

    /// Pops `2n` values (key0, value0, key1, value1, ...) and creates an object.
    /// Stack: [k0, v0, k1, v1, ...] → [object]
    ObjectCreate(u16),

    /// Pops `object` and `key`, pushes `object[key]`.
    /// Stack: [object, key] → [value]
    GetElement,
    /// Reads a property by name from the top-of-stack object and rearranges
    /// the stack so the callee and `this` are in position for `CallWithThis`.
    /// Pops `object`, pushes `method_value` then `object` back.
    /// Stack: [object] → [method_value, object]
    GetMethod(u16),

    /// Reads a computed property and rearranges the stack so the callee and
    /// `this` are in position for `CallWithThis`.
    /// Stack: [object, key] -> [method_value, object]
    GetElementMethod,

    /// Sets a named property on an object, preserving the value as the
    /// assignment result. Stack layout: `[object, value]`.
    /// Stack: [object, value] → [value]
    SetProperty(u16),

    /// Sets a computed property, preserving the value as the assignment result.
    /// Stack layout: `[object, key, value]`.
    /// Stack: [object, key, value] → [value]
    SetElement,

    /// Calls a method with an explicit `this`. Stack layout:
    /// `[callee, this_value, arg0, ..., argN]`.
    /// Stack: [callee, this, args...] → [result]
    CallWithThis(u16),

    // V4 object-model instructions.
    ObjectCreateEmpty,
    ArrayCreateSparse(u32),
    DefineDataProperty(u16),
    DefineGetter(u16),
    DefineSetter(u16),
    DefineComputedGetter,
    DefineComputedSetter,
    /// Like `DefineDataProperty` but uses `writable=true, enumerable=false, configurable=true`.
    /// Used for class instance/static methods (spec: non-enumerable, configurable).
    DefineClassMethod(u16),
    /// Like `DefineGetter` but uses `enumerable=false, configurable=true` for class accessors.
    DefineClassGetter(u16),
    /// Like `DefineSetter` but uses `enumerable=false, configurable=true` for class accessors.
    DefineClassSetter(u16),
    /// Computed-key class method: Stack `[obj, key, fn]` → `[obj]`.
    /// Defines `obj[key] = fn` with class-method descriptor (non-enumerable, configurable).
    DefineClassMethodComputed,
    /// Computed-key class getter: Stack `[obj, key, fn]` → `[obj]`.
    DefineClassGetterComputed,
    /// Computed-key class setter: Stack `[obj, key, fn]` → `[obj]`.
    DefineClassSetterComputed,
    /// Computed-key data property: Stack `[obj, key, val]` → `[obj]`.
    /// Defines obj[key] = val with {writable, enumerable, configurable}.
    DefineDataPropertyComputed,
    SetObjectPrototype,
    DefineElement(u32),
    DeleteProperty(u16),
    DeleteElement,
    HasProperty,
    InstanceOf,

    // V5 structured completion and lexical environment instructions.
    CreateLexicalEnvironment,
    PopEnvironment,
    CreateMutableBinding(u16),
    CreateImmutableBinding(u16),
    InitializeBinding(u16),
    LoadException,
    EndFinally,

    // V6 iteration instruction.
    /// Pops an object and pushes an array of its `for-in` enumerable string
    /// keys (own + prototype chain, de-duplicated). `null`/`undefined` yield an
    /// empty array.
    /// Stack: [object] → [keys_array]
    ForInKeys,

    // V6 regex instruction.
    /// Pops `flags` (String) and `pattern` (String) from the stack and pushes a
    /// new RegExp object with those values. Emitted for `/pattern/flags` literals.
    /// Stack: [pattern, flags] → [regexp_object]
    CreateRegExp,

    // V8-A spread / rest / array-push instructions.
    /// Appends one value to the end of an array without removing the array.
    /// Stack: [array, value] → [array]
    ArrayPush,

    /// Iterates an array-like iterable and appends every element to the array
    /// sitting just below it on the stack.
    /// Stack: [array, iterable] → [array]
    SpreadIntoArray,

    /// Copies all enumerable own properties from the spread value into the
    /// object sitting just below it on the stack (Object.assign semantics).
    /// Stack: [object, spread_value] → [object]
    SpreadObject,

    /// Calls a function using a single trailing spread argument.
    /// `n` = number of regular arguments already pushed before the spread.
    /// Stack: [callee, arg0…argN-1, spread_iterable] → [result]
    SpreadCall(u16),

    /// Like `SpreadCall` but passes an explicit `this`.
    /// Stack: [callee, this, arg0…argN-1, spread_iterable] → [result]
    SpreadCallWithThis(u16),

    /// `new` constructor call with a single trailing spread argument.
    /// Stack: [callee, arg0…argN-1, spread_iterable] → [result]
    SpreadConstruct(u16),

    // -----------------------------------------------------------------------
    // V9-A: iterator protocol (runtime provided by V9-B)
    // -----------------------------------------------------------------------
    /// Calls `iterable[Symbol.iterator]()` and pushes the resulting iterator.
    /// Stack: [iterable] → [iterator]
    GetIterator,

    /// Calls `iterator.next()`, pushes `is_done` flag on top and `value` below.
    /// Stack: [iterator] → [value, is_done]
    IteratorNext,

    /// Calls `iterator.return()` if present, then discards the iterator.
    /// Stack: [iterator] → []
    IteratorClose,

    // -----------------------------------------------------------------------
    // V9-A: generator support (runtime provided by V9-B)
    // -----------------------------------------------------------------------
    /// Creates a suspended generator object from a function template.
    /// Operand is a function-table index (same as CreateFunction).
    /// Stack: [] → [generator]
    CreateGenerator(u16),

    /// Suspends the current generator frame, yielding a value to the caller.
    /// Stack: [value] → [sent_value]
    YieldValue,

    /// `yield*` — delegates to another iterable and resumes with its return.
    /// Stack: [iterable] → [delegate_return_value]
    YieldDelegate,

    // -----------------------------------------------------------------------
    // V9-A: async support (runtime provided by V9-B)
    // -----------------------------------------------------------------------
    /// Creates an async function wrapper from a function template.
    /// Operand is a function-table index.
    /// Stack: [] → [async_fn]
    CreateAsyncFunction(u16),

    /// Suspends the async function, awaiting a Promise resolution.
    /// Stack: [value] → [resolved_value]
    AwaitValue,
}

impl Instruction {
    /// Returns the instruction's fixed operand-stack contract.
    #[must_use]
    pub const fn stack_effect(self) -> StackEffect {
        match self {
            // push 1
            Self::Constant(_)
            | Self::LoadGlobal(_)
            | Self::TypeOfGlobal(_)
            | Self::CreateFunction(_)
            | Self::LoadName(_)
            | Self::TypeOfName(_)
            | Self::LoadThis
            | Self::LoadNewTarget
            | Self::ObjectCreateEmpty
            | Self::ArrayCreateSparse(_)
            | Self::LoadException => StackEffect::new(0, 1),

            Self::Duplicate => StackEffect::with_required(1, 0, 1),
            Self::DuplicatePair => StackEffect::with_required(2, 0, 2),
            Self::Swap => StackEffect::with_required(2, 2, 2),
            Self::SetFunctionName(_) => StackEffect::with_required(1, 0, 0),

            // pop 1, push 0
            Self::Pop
            | Self::DeclareGlobal(_)
            | Self::Throw
            | Self::Return
            | Self::DeclareLocal(_)
            | Self::InitializeBinding(_) => StackEffect::new(1, 0),

            // pop 1, push 1 (net 0)
            Self::StoreGlobal(_)
            | Self::StoreName(_)
            | Self::UnaryPlus
            | Self::Increment
            | Self::Decrement
            | Self::Negate
            | Self::LogicalNot
            | Self::BitwiseNot
            | Self::GetProperty(_)
            | Self::ForInKeys
            | Self::TypeOf => StackEffect::new(1, 1),

            // pop 2, push 1
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Remainder
            | Self::Exponentiation
            | Self::BitwiseAnd
            | Self::BitwiseOr
            | Self::BitwiseXor
            | Self::LeftShift
            | Self::RightShift
            | Self::UnsignedRightShift
            | Self::Equal
            | Self::NotEqual
            | Self::StrictEqual
            | Self::StrictNotEqual
            | Self::LessThan
            | Self::LessThanOrEqual
            | Self::GreaterThan
            | Self::GreaterThanOrEqual
            | Self::GetElement
            | Self::DeleteElement
            | Self::HasProperty
            | Self::InstanceOf
            | Self::CreateRegExp => StackEffect::new(2, 1),

            // observe top (no stack change)
            Self::JumpIfFalse(_)
            | Self::JumpIfTrue(_)
            | Self::JumpIfNotNullish(_)
            | Self::JumpIfNotUndefined(_) => StackEffect::with_required(1, 0, 0),

            // no stack effect
            Self::Jump(_)
            | Self::ReturnUndefined
            | Self::DeclareFunction { .. }
            | Self::CreateLexicalEnvironment
            | Self::PopEnvironment
            | Self::CreateMutableBinding(_)
            | Self::CreateImmutableBinding(_)
            | Self::EndFinally => StackEffect::new(0, 0),

            // call variants
            Self::Call(argument_count) | Self::Construct(argument_count) => {
                StackEffect::new(argument_count as u32 + 1, 1)
            }
            Self::CallWithThis(argument_count) => {
                StackEffect::with_required(argument_count as u32 + 2, argument_count as u32 + 2, 1)
            }

            // GetMethod: pops object, pushes (method, object) — net +1
            Self::GetMethod(_) => StackEffect::new(1, 2),

            // GetElementMethod: [object, key] -> [method, object]
            Self::GetElementMethod => StackEffect::with_required(2, 2, 2),

            // SetProperty: [object, value] → [value]  (net -1)
            Self::SetProperty(_) => StackEffect::with_required(2, 2, 1),

            // SetElement: [object, key, value] → [value]  (net -2)
            Self::SetElement => StackEffect::with_required(3, 3, 1),

            // ArrayCreate(n): pops n, pushes 1
            Self::ArrayCreate(n) => StackEffect::with_required(n as u32, n as u32, 1),

            // ObjectCreate(n): pops 2n (n key-value pairs), pushes 1
            Self::ObjectCreate(n) => StackEffect::with_required(n as u32 * 2, n as u32 * 2, 1),

            // The object remains below the consumed value.
            Self::DefineDataProperty(_)
            | Self::DefineGetter(_)
            | Self::DefineSetter(_)
            | Self::DefineClassMethod(_)
            | Self::DefineClassGetter(_)
            | Self::DefineClassSetter(_)
            | Self::SetObjectPrototype
            | Self::DefineElement(_) => StackEffect::with_required(2, 1, 0),

            // Computed-key class members and data: [obj, key, val] → [obj]  (pop val+key, peek obj)
            Self::DefineClassMethodComputed
            | Self::DefineClassGetterComputed
            | Self::DefineClassSetterComputed
            | Self::DefineDataPropertyComputed => StackEffect::with_required(3, 2, 0),

            Self::DefineComputedGetter | Self::DefineComputedSetter => {
                StackEffect::with_required(3, 3, 0)
            }

            Self::DeleteProperty(_) => StackEffect::new(1, 1),

            // ArrayPush: [array, value] → [array]
            Self::ArrayPush | Self::SpreadIntoArray | Self::SpreadObject => {
                StackEffect::with_required(2, 1, 0)
            }

            // SpreadCall/SpreadConstruct: variable pops, push 1
            Self::SpreadCall(n) | Self::SpreadConstruct(n) => {
                // callee + n regular args + 1 spread = n+2 consumed, 1 produced
                StackEffect::new(n as u32 + 2, 1)
            }
            Self::SpreadCallWithThis(n) => {
                // callee + this + n regular args + 1 spread = n+3 consumed, 1 produced
                StackEffect::new(n as u32 + 3, 1)
            }

            // V9-A iterator protocol
            Self::GetIterator => StackEffect::new(1, 1),
            Self::IteratorNext => StackEffect::new(1, 2),
            Self::IteratorClose => StackEffect::new(1, 0),

            // V9-A generator / async
            Self::CreateGenerator(_) | Self::CreateAsyncFunction(_) => StackEffect::new(0, 1),
            Self::YieldValue | Self::AwaitValue | Self::YieldDelegate => StackEffect::new(1, 1),
        }
    }

    #[must_use]
    pub const fn is_terminator(self) -> bool {
        matches!(self, Self::Return | Self::ReturnUndefined | Self::Throw)
    }

    #[must_use]
    pub const fn has_fallthrough(self) -> bool {
        !matches!(
            self,
            Self::Jump(_) | Self::Return | Self::ReturnUndefined | Self::Throw
        )
    }

    #[must_use]
    pub const fn jump_target(self) -> Option<usize> {
        match self {
            Self::JumpIfFalse(target)
            | Self::JumpIfTrue(target)
            | Self::JumpIfNotNullish(target)
            | Self::JumpIfNotUndefined(target)
            | Self::Jump(target) => Some(target),
            _ => None,
        }
    }
}

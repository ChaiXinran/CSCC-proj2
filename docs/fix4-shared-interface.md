# Fix4 Shared Interface

This document freezes the collaboration boundary for Fix4. Implementations stay
in their owning modules; consumers must reuse these contracts instead of adding
parallel iterator, Promise, or descriptor state.

## Function Kinds and Bytecode

`FunctionTemplate` and `JsFunction` carry `is_async` and `is_generator` flags.
The four combinations represent normal, async, generator, and async-generator
functions. A owns AST/parser flags; B owns propagation through
`src/bytecode/compiler.rs`, `src/bytecode/chunk.rs`, and the VM.

The frozen B opcodes are `CreateGenerator`, `YieldValue`, `YieldDelegate`,
`CreateAsyncFunction`, `AwaitValue`, `GetIterator`, `IteratorNext`, and
`IteratorClose`. Any new opcode must document operands, stack effect, abrupt
completion behavior, and saved-frame state in `src/bytecode/opcode.rs`.

## Iterator Boundary

`src/runtime/iterator.rs::IteratorRecord` is the single native iterator record.
Use these `NativeContext` methods for array/string runtime iteration:

```rust
get_iterator(value) -> Result<IteratorRecord, VmError>
iterator_next(&mut record) -> Result<Option<JsValue>, VmError>
iterator_close(&mut record) -> Result<(), VmError>
```

JavaScript protocol dispatch belongs to `Vm`: it must call `@@iterator`, cache
or read `next` as required, validate iterator-result objects, propagate getter
and callback throws, and root live iterator objects across allocation. Builtins
that only need values call `Vm::collect_iterable_values_from_builtin`; they must
not inspect array storage directly. A uses `GetIterator`/`IteratorClose` for
destructuring. C uses the VM helper for `Array.from`, `TypedArray.from`, and
Promise combinators.

## Promise and Async Boundary

`src/runtime/job.rs` owns `PromiseState`, `PromiseRecord`, reactions, and the
FIFO `JobQueue`. Producers enqueue through `NativeContext::enqueue_job`; the
backend drains through `Vm::drain_jobs`. Promise settlement is one-shot.

Async functions always return a native Promise. `AwaitValue` accepts ordinary
values and settled native Promises, draining queued native jobs before reading
state. A still owns syntax; C owns JS-visible constructor/prototype descriptors.
Pending-Promise continuation capture and async-generator resumption are not yet
part of the implemented contract and must not be claimed as supported.

## Errors, GC, and Changes

ECMAScript failures cross VM helpers as catchable throw values; engine invariant
failures remain `VmError`. Any iterator or callback-held `JsValue` that survives
an allocation must be present in a traced runtime record or on the VM root
stack. Shared-interface changes require a focused test and an entry below.

| Date | Owner | Change | Verification |
| --- | --- | --- | --- |
| 2026-06-27 | B | Freeze existing iterator/job contracts; add async flags, builtin iterable collection, and immediate-await behavior | `native_iteration`, `native_promise`, `parser_iteration` |

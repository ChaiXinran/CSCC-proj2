# Native V16 shared interface (B owner)

This document freezes the B-owned runtime protocol for V16. B owns object/property
semantics, abstract operations, iterator and promise/job protocols, and module
linking/evaluation. A owns grammar and language lowering; C owns Temporal, Intl,
RegExp, and binary-data algorithms.

## Shared operation rules

`runtime::abstract_ops` is the single implementation for `SameValueZero`,
callability checks, numeric `ToIntegerOrInfinity`/`ToLength`/`ToIndex`, and the
primitive `ToPropertyKey` path. Any operation that can invoke JavaScript must
remain on the VM/context path: `Get`, `Set`, `GetMethod`, `Call`, `Construct`,
accessors, proxy traps, iterator methods, species constructors, and promise
reactions.

The current API also exposes VM-mediated `get`, `set`, `call`, `construct`, and
`to_primitive`, plus `get_iterator`, `iterator_next`, `iterator_close`,
`close_iterator`, `enqueue_job`, and `drain_jobs`. `close_iterator` is the
required path for JavaScript iterator objects because it invokes observable
`return` methods; the record-only `iterator_close` is reserved for native
records that have no JavaScript callback.

Object internal slots use `ObjectKind` or typed runtime records. New observable
`__agentjs_*` properties are prohibited. Promise continuations, dynamic import,
top-level await, and async iterators share `NativeContext`'s `JobQueue`; builtins
must not drain it themselves.

## Error and Realm contract

Runtime protocol failures use the existing `VmError` categories. JavaScript
callbacks preserve their original abrupt completion. Constructors and prototypes
are resolved from the active Realm intrinsics, and cross-Realm checks use internal
identity/slots rather than constructor names.

# Native V16 shared-file locks

| file/function | owner | feature | start SHA | merge order | released |
|---|---|---|---|---|---|
| `src/runtime/abstract_ops.rs` | B | shared abstract operations | current baseline | first | no |
| `src/runtime/context.rs` (`get`, `set`, iterator, jobs) | B | VM callback and async protocol | current baseline | after abstract ops | no |
| `src/runtime/object.rs`, `property.rs` | B | object and descriptor model | current baseline | after abstract ops | no |
| `src/runtime/module.rs` | B | module linking/evaluation/dynamic import | current baseline | after job protocol | no |
| `src/builtins/array.rs`, `collections.rs` | B | shared helper consumers | current baseline | after abstract ops | no |
| `src/builtins/promise.rs` | B | Promise capability/reactions | current baseline | after job protocol | no |
| `src/builtins/mod.rs` | B | builtin installation contract | current baseline | last | no |

A and C may consume these interfaces but must not duplicate their semantics.

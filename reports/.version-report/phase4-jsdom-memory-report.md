# Phase 4 jsdom-d3-startup memory closeout

Date: 2026-08-08  
Baseline: `main@cd3592d`

## Root cause

`jsdom-d3-startup` was not a Track L host-script-session failure. Its startup harness runs a
global regular expression over the 3.5 MiB D3 bundle to find 10,028 cache-buster comments.

The RegExp symbol algorithms accepted an owned `String` and called `string.to_string()` for every
`RegExpExec` iteration. Each match therefore copied the entire bundle before calling
`RegExp.prototype.exec`. This native string churn was outside heap allocation counts and side
registries, so neither the 10k fixed threshold nor adaptive GC fired before process RSS crossed
1536 MiB.

## Fix

- `regexp_exec_value` and `regexp_exec_abstract` now accept `JsString`.
- Global match, replace, search, and split paths clone the shared string handle instead of copying
  the complete input for every match.
- Added a regression test covering global matching over a large input with many matches.

## Protected-run result

Configuration: release, one iteration, 32 MiB stack, 1536 MiB working-set limit, 150-second
timeout, adaptive GC, diagnostics.

| Revision | Status | Wall | Peak RSS |
|---|---:|---:|---:|
| `cd3592d` | MEMORY_LIMIT | 2.143 s | 1606.8 MiB |
| fixed | CALL_ERROR | 5.806 s | 88.0 MiB |

The memory failure is closed: peak RSS fell by 1518.8 MiB (94.5%). The newly exposed
`undefined is not callable` failure is a separate D3/jsdom semantic gap, not an OOM or GC-lifetime
failure.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo check --all-targets` — PASS
- `cargo test --test runtime` — PASS, 14/14
- `cargo test --all-targets` — PASS
- `cargo clippy --all-targets -- -D warnings` — PASS
- `cargo build --release --locked` — PASS

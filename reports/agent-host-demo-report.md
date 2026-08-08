# Agent Host Demo Report

## Scope

This demonstration batch adds the Rust-owned `agent.render(RenderTree)` host API. It does not add HTTP, model integration, browser execution, or conversation persistence.

## Contract

- `Runtime::eval_agent(source, options)` returns an `ExecutionReport`.
- `ExecutionReport.render_events` contains canonical JSON payloads in call order.
- Supported root types are `panel`, `text`, `metrics`, `statuses`, `table`, and `list`.
- Default limits are 256 KiB serialized JSON and 32 nested object/array levels.
- `agent` and `agent.render` are non-extensible and cannot be replaced by scripts.

## Files touched

- `src/engine.rs`
- `src/backend/mod.rs`
- `src/runtime/context.rs`
- `src/host/mod.rs`
- `src/builtins/mod.rs`
- `src/lib.rs`
- `src/main.rs`
- `src/test262.rs`
- `tests/agent_host.rs`
- `reports/agent-host-demo-report.md`

## Verification

- `cargo fmt --all -- --check` — passed.
- `cargo check --all-targets` — passed.
- `cargo test --all-targets` — passed, including the four new Agent Host tests and all fixed native Test262 selectors.
- `cargo clippy --all-targets -- -D warnings` — passed with zero warnings.
- `cargo test --no-default-features --test native_test262` — passed.

## Coordination notes

The orchestrator can pass `RenderEvent.payload` through as parsed JSON and map `ExecutionReport.output` to `execution.logs`. The frontend should render only the six supported node types and must not interpret the payload as HTML.

# V11 Part A Report — RegExp Parser / Static Errors

Owner: A group
Scope: lexer / parser / AST / RegExp static errors

This report must be updated by any worker or AI agent that changes V11-A code.
Do not wait for an explicit user request.

## Baseline

Baseline source: `reports/native-v11-scan-failures.txt`, generated from the
locked full direct Test262 output and filtered toward V11 hotspots.

| Metric | Baseline |
| --- | ---: |
| V11 scan total | 5,000 |
| V11 scan passed | pending |
| V11 scan failed | pending |
| V11 scan skipped | pending |

Primary directories:

- `test/language/literals/regexp`
- RegExp static-error and Unicode escape residual cases

## Current Status

Status: first V11-A RegExp literal static-error pass implemented.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-A report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partA-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-24 | A | Added RegExp literal static checks for quantifiers without atoms, Unicode-mode identity/control/decimal escapes, Unicode property escape syntax, class-escape range endpoints, quantified lookarounds, and regex-body `\u...` lexer fallback before parser re-read | `src/lexer/mod.rs`, `tests/frontend_v11.rs`, `reports/v11-partA-report.md` | `cargo test --no-default-features --test frontend_v11`; `cargo test --no-default-features --test frontend_v10`; `cargo test --no-default-features --test native_test262`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | focused regexp-literals scan now records 218/238 passed, 20 failed, 0 skipped, 91.60%; standard V11 scan still exceeded 600s local timeout and produced no JSON |

## Implemented Functionality

- Added A-owned RegExp body static validation helpers in `src/lexer/mod.rs`.
- Rejects quantifiers without a preceding atom, including `?`, `*`, `+`, and
  braced quantifiers such as `{2,3}` at pattern start.
- Rejects Unicode-mode invalid identity/control/legacy decimal escapes,
  including `\M`, `\a`, `\c0`, `\1`, `\8`, and `\01` when appropriate.
- Validates the syntactic shell of Unicode property escapes such as
  `\p{ASCII}` / `\P{General_Category=Letter}` without implementing runtime
  property matching.
- Rejects Unicode character class ranges whose endpoint is a multi-character
  class escape, such as `[\d-a]`, `[\s-\d]`, and `[%-\d]`.
- Rejects quantified lookahead in Unicode mode and quantified lookbehind in
  both Unicode and non-Unicode modes.
- Made lexer `\u...` identifier handling fall back to placeholder tokens when
  the escape cannot form an identifier, so the parser's RegExp re-reader can
  classify RegExp body escapes instead of failing early as an identifier error.
- Added `tests/frontend_v11.rs` with focused A-line parse success/failure
  coverage.

## Test Results and Delta Analysis

Initial V11 scan command:

```text
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Record focused A commands and future `--native-v11-scan` deltas here.

Setup validation:

- `native_test262`: 15 passed, 0 failed.
- `cargo check --no-default-features --all-targets`: passed.
- Initial `--native-v11-scan`: timed out after 300s in local tool execution;
  baseline pass/fail totals remain pending.

Current A validation:

- `cargo test --no-default-features --test frontend_v11`: 7 passed, 0 failed.
- `cargo test --no-default-features --test frontend_v10`: 7 passed, 0 failed.
- `cargo test --no-default-features --test native_test262`: 15 passed, 0 failed.
- Focused regexp literal scan:

```text
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json
```

Result:

| Metric | Current |
| --- | ---: |
| total | 238 |
| passed | 218 |
| failed | 20 |
| skipped | 0 |
| conformance | 91.60% |

- Standard V11 scan command attempted with a 600s local timeout:

```text
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json
```

Result: timed out after 600s; `reports/native-v11-scan-summary.json` was not
created.

## Open Risks / Coordination Notes

- Preserve V10-A in-progress numeric/source-text work.
- Do not implement RegExp runtime matching in A-owned files.
- Coordinate RegExp literal metadata shape with C before builtin algorithms
  depend on it.
- Remaining focused regexp-literal failures are mostly outside this A pass:
  runtime RegExp backend limitations for named backreferences, `\0`,
  surrogate-pair escapes, case mapping, and two runtime-limit cases. Several
  dynamic `Function` string-boundary failures remain parser/host integration
  follow-up candidates.

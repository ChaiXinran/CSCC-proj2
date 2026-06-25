# V11 Part A Report - RegExp Parser / Static Errors

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

Status: second V11-A RegExp literal lexer-boundary pass implemented.

## Change Log

| Date | Worker | Summary | Files changed | Tests run | Result delta |
| --- | --- | --- | --- | --- | --- |
| 2026-06-24 | setup | Created V11-A report template, locked V11 scan manifest, and installed `--native-v11-scan` selector | `reports/v11-partA-report.md`, `reports/native-v11-scan-failures.txt`, V11 docs/selector files | `cargo test --no-default-features --test native_test262`; `cargo check --no-default-features --all-targets`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | selector/gates pass; scan attempt exceeded 300s local timeout, no JSON summary produced |
| 2026-06-24 | A | Added RegExp literal static checks for quantifiers without atoms, Unicode-mode identity/control/decimal escapes, Unicode property escape syntax, class-escape range endpoints, quantified lookarounds, and regex-body `\u...` lexer fallback before parser re-read | `src/lexer/mod.rs`, `tests/parser_regexp_errors.rs`, `reports/v11-partA-report.md` | `cargo test --no-default-features --test parser_regexp_errors`; `cargo test --no-default-features --test native_test262`; `cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json`; attempted `cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json` | focused regexp-literals scan recorded 218/238 passed, 20 failed, 0 skipped, 91.60%; standard V11 scan exceeded 600s local timeout and produced no JSON |
| 2026-06-25 | Codex / A group | Added lexer-boundary fallback for quotes/backticks inside RegExp literal candidates and rejected backslash-line-terminator RegExp escapes during parser re-read | `src/lexer/mod.rs`, `tests/parser_regexp_errors.rs`, `reports/native-v11-a-regexp-literals-summary.json`, `reports/v11-partA-report.md` | `cargo fmt --all -- --check`; `cargo test --no-default-features --test parser_regexp_errors`; focused regexp-literals Test262 scan | `parser_regexp_errors`: 9/9; focused regexp-literals scan: 226/238 passed, 12 failed, 0 skipped, 94.9580%; current local retry baseline before this pass was 217/238 |

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
- Made unterminated quote and backtick scans fall back to placeholder tokens
  when they appear inside a same-line RegExp literal candidate, allowing the
  parser re-reader to preserve valid patterns such as `/"/`, `/''/`, and
  ``/`/``.
- Rejects RegExp backslash sequences whose escaped character is a line
  terminator, including LF, CR, LS, and PS, for both first-character and
  later-character positions.
- Added `tests/parser_regexp_errors.rs` with focused A-line parse
  success/failure coverage.

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

```text
cargo fmt --all -- --check
cargo test --no-default-features --test parser_regexp_errors
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test/language/literals/regexp --jobs 4 --progress --json reports/native-v11-a-regexp-literals-summary.json
```

Results:

- `cargo fmt --all -- --check`: passed.
- `parser_regexp_errors`: 9 passed, 0 failed.
- Focused regexp literal scan:

| Metric | Current |
| --- | ---: |
| total | 238 |
| passed | 226 |
| failed | 12 |
| skipped | 0 |
| conformance | 94.9580% |

The focused scan improved from the current local retry baseline of 217/238 to
226/238 after this pass. Compared with the previous report's 218/238 result,
this is a net +8 focused Test262 passes.

Large-suite commands to run externally when a longer local window is available:

```powershell
cargo run --release --no-default-features -- test262 --native-v11-scan --jobs 4 --json reports/native-v11-scan-summary.json *> reports/native-v11-scan-verbose.txt
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-v11-full-summary.json *> reports/native-v11-full-verbose.txt
```

Do not count skipped, timed-out, crashed, or uncaptured cases as passes.

## Open Risks / Coordination Notes

- Preserve V10-A in-progress numeric/source-text work.
- Do not implement RegExp runtime matching in A-owned files.
- Coordinate RegExp literal metadata shape with C before builtin algorithms
  depend on it.
- Remaining focused regexp-literal failures are now mostly outside A-owned
  parser/static-error work: runtime RegExp backend limitations for named
  backreferences, `\0`, surrogate-pair escapes, case mapping, and sticky `^`
  behavior; one legacy parse-negative ambiguity (`/a//.source`); and several
  65,536-iteration dynamic `eval` loops that can hit the harness wall-clock
  deadline after lexer-boundary errors are removed.
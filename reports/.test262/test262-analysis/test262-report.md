# Test262 Conformance Report

Test date: 2026-06-24 (UTC+08:00)

- AgentJS build: release, `--no-default-features`
- Rust: `rustc 1.91.0 (f8297e351 2025-10-28)`
- Test262 revision: `de8e621cdba4f40cff3cf244e6cfb8cb48746b4a`
- Binary: `target/release/agentjs.exe` (3,261,440 bytes)
- Platform: Windows, native backend

## Command

```powershell
cargo run --release --no-default-features -- test262 --backend native --root test262 --suite test --jobs 4 --progress --json reports/native-full-test262-summary.json
```

Stdout/stderr were captured to:

```text
reports/native-full-test262-output.txt
```

PowerShell wrote the captured output as UTF-16LE. Parse tools should read it
with UTF-16 decoding.

## Result

This is a direct full `test/` run. It includes `test/staging`.

| Suite | Total | Passed | Failed | Skipped | Pass rate |
| --- | ---: | ---: | ---: | ---: | ---: |
| `test/` direct full run | 53,379 | 14,035 | 38,507 | 837 | 26.29% |

The runner completed the scan and wrote `reports/native-full-test262-summary.json`.
The process exit code was non-zero because the suite has failing tests.

## Skip summary

| Skip reason | Count |
| --- | ---: |
| `module runner not implemented yet` | 821 |
| no detail | 14 |
| `non-blocking agent tests are not enabled` | 2 |

## Failure summary

Parsed failure lines: 38,507.

| Top-level area | Failures | Share of failures |
| --- | ---: | ---: |
| `language` | 16,660 | 43.27% |
| `built-ins` | 16,482 | 42.80% |
| `intl402` | 3,331 | 8.65% |
| `staging` | 1,199 | 3.11% |
| `annexB` | 748 | 1.94% |
| `harness` | 87 | 0.23% |

Detailed root-cause classification is in
`reports/test262-analysis.md`.

## Caveats

- This report records the exact command requested above. It is not directly
  comparable with older sharded/non-staging reports, because this command scans
  all of `test/`, including `test/staging`.
- `test/staging` contains experimental and stress-oriented cases. Keep it in
  this stress dashboard, but do not mix it with formal non-staging conformance
  unless the contest/evaluation rule explicitly requires it.
- Module tests are still mostly skipped because module execution is not yet
  implemented by the native runner.
- The current result is dominated by parser/modern-syntax gaps, missing
  builtin/global families, and unsupported template substitutions. These are
  analyzed in `reports/test262-analysis.md`.

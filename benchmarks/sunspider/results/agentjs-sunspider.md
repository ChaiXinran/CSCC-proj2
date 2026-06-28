# SunSpider 1.0.2 — agentjs vs boa

## Correctness Summary

| Category | Cases | Pass | Wrong | Error | Timeout |
|:---|---:|---:|---:|---:|---:|
| 3d | 3 | 3 | — | — | — |
| access | 4 | 2 | — | — | 2 |
| bitops | 4 | 4 | — | — | — |
| controlflow | 1 | — | — | — | 1 |
| crypto | 3 | 3 | — | — | — |
| date | 2 | 2 | — | — | — |
| math | 3 | 3 | — | — | — |
| regexp | 1 | 1 | — | — | — |
| string | 5 | 3 | — | — | 2 |
| **Total** | **26** | **21** | **0** | **0** | **5** |

## Per-Case Results

| Case | Status | Median time |
|:---|---:|---:|
| 3d-cube | [PASS] pass | 3920ms |
| 3d-morph | [PASS] pass | 290ms |
| 3d-raytrace | [PASS] pass | 12.5s |
| access-binary-trees | [TIMEOUT] timeout | — |
| access-fannkuch | [PASS] pass | 846ms |
| access-nbody | [TIMEOUT] timeout | — |
| access-nsieve | [PASS] pass | 346ms |
| bitops-3bit-bits-in-byte | [PASS] pass | 48.4s |
| bitops-bits-in-byte | [PASS] pass | 29.8s |
| bitops-bitwise-and | [PASS] pass | 292ms |
| bitops-nsieve-bits | [PASS] pass | 489ms |
| controlflow-recursive | [TIMEOUT] timeout | — |
| crypto-aes | [PASS] pass | 2282ms |
| crypto-md5 | [PASS] pass | 32.6s |
| crypto-sha1 | [PASS] pass | 38.0s |
| date-format-tofte | [PASS] pass | 24.1s |
| date-format-xparb | [PASS] pass | 8800ms |
| math-cordic | [PASS] pass | 45.8s |
| math-partial-sums | [PASS] pass | 168ms |
| math-spectral-norm | [PASS] pass | 30.4s |
| regexp-dna | [PASS] pass | 3362ms |
| string-base64 | [PASS] pass | 887ms |
| string-fasta | [PASS] pass | 27.2s |
| string-tagcloud | [TIMEOUT] timeout | — |
| string-unpack-code | [TIMEOUT] timeout | — |
| string-validate-input | [PASS] pass | 3138ms |

## Failure Details

| Case | Status | Error |
|:---|:---|:---|
| access-binary-trees | timeout | `timed out after 60s` |
| access-nbody | timeout | `timed out after 60s` |
| controlflow-recursive | timeout | `timed out after 60s` |
| string-tagcloud | timeout | `timed out after 60s` |
| string-unpack-code | timeout | `timed out after 60s` |

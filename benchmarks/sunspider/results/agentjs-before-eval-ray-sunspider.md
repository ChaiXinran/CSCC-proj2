# SunSpider 1.0.2 — agentjs-before-eval-ray

## Correctness Summary

| Category | Cases | Pass | Wrong | Error | Timeout |
|:---|---:|---:|---:|---:|---:|
| 3d | 3 | — | 1 | — | — |
| access | 4 | — | — | — | — |
| bitops | 4 | — | — | — | — |
| controlflow | 1 | — | — | — | — |
| crypto | 3 | — | — | — | — |
| date | 2 | — | — | 1 | — |
| math | 3 | — | — | — | — |
| regexp | 1 | — | — | — | — |
| string | 5 | — | — | — | — |
| **Total** | **26** | **0** | **1** | **1** | **0** |

## Per-Case Results

| Case | Status | Median time |
|:---|---:|---:|
| 3d-cube | [SKIP] skip | — |
| 3d-morph | [SKIP] skip | — |
| 3d-raytrace | [WRONG] wrong-result | — |
| access-binary-trees | [SKIP] skip | — |
| access-fannkuch | [SKIP] skip | — |
| access-nbody | [SKIP] skip | — |
| access-nsieve | [SKIP] skip | — |
| bitops-3bit-bits-in-byte | [SKIP] skip | — |
| bitops-bits-in-byte | [SKIP] skip | — |
| bitops-bitwise-and | [SKIP] skip | — |
| bitops-nsieve-bits | [SKIP] skip | — |
| controlflow-recursive | [SKIP] skip | — |
| crypto-aes | [SKIP] skip | — |
| crypto-md5 | [SKIP] skip | — |
| crypto-sha1 | [SKIP] skip | — |
| date-format-tofte | [ERROR] runtime-error | — |
| date-format-xparb | [SKIP] skip | — |
| math-cordic | [SKIP] skip | — |
| math-partial-sums | [SKIP] skip | — |
| math-spectral-norm | [SKIP] skip | — |
| regexp-dna | [SKIP] skip | — |
| string-base64 | [SKIP] skip | — |
| string-fasta | [SKIP] skip | — |
| string-tagcloud | [SKIP] skip | — |
| string-unpack-code | [SKIP] skip | — |
| string-validate-input | [SKIP] skip | — |

## Failure Details

| Case | Status | Error |
|:---|:---|:---|
| 3d-raytrace | wrong-result | `agentjs: Error: execution error: uncaught Error: bad result: expected length 20970 but got 24183` |
| date-format-tofte | runtime-error | `agentjs: ReferenceError: execution error: Y is not defined at instruction 0` |

# SunSpider 1.0.2 — agentjs-hot-after-cache vs node

## Correctness Summary

| Category | Cases | Pass | Wrong | Error | Timeout |
|:---|---:|---:|---:|---:|---:|
| 3d | 3 | — | — | — | — |
| access | 4 | — | — | — | — |
| bitops | 4 | — | — | — | — |
| controlflow | 1 | — | — | — | — |
| crypto | 3 | — | — | — | — |
| date | 2 | — | — | — | — |
| math | 3 | — | — | — | — |
| regexp | 1 | 1 | — | — | — |
| string | 5 | 2 | — | — | — |
| **Total** | **26** | **3** | **0** | **0** | **0** |

## Per-Case Results

| Case | agentjs-hot-after-cache status | agentjs-hot-after-cache median | node median | agentjs-hot-after-cache/node |
|:---|---:|---:|---:|---:|
| 3d-cube | [SKIP] skip | — | — | — |
| 3d-morph | [SKIP] skip | — | — | — |
| 3d-raytrace | [SKIP] skip | — | — | — |
| access-binary-trees | [SKIP] skip | — | — | — |
| access-fannkuch | [SKIP] skip | — | — | — |
| access-nbody | [SKIP] skip | — | — | — |
| access-nsieve | [SKIP] skip | — | — | — |
| bitops-3bit-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bitwise-and | [SKIP] skip | — | — | — |
| bitops-nsieve-bits | [SKIP] skip | — | — | — |
| controlflow-recursive | [SKIP] skip | — | — | — |
| crypto-aes | [SKIP] skip | — | — | — |
| crypto-md5 | [SKIP] skip | — | — | — |
| crypto-sha1 | [SKIP] skip | — | — | — |
| date-format-tofte | [SKIP] skip | — | — | — |
| date-format-xparb | [SKIP] skip | — | — | — |
| math-cordic | [SKIP] skip | — | — | — |
| math-partial-sums | [SKIP] skip | — | — | — |
| math-spectral-norm | [SKIP] skip | — | — | — |
| regexp-dna | [PASS] pass | 3464ms | 67ms | 51.9× |
| string-base64 | [SKIP] skip | — | — | — |
| string-fasta | [SKIP] skip | — | — | — |
| string-tagcloud | [PASS] pass | 3512ms | 93ms | 37.6× |
| string-unpack-code | [PASS] pass | 17.7s | 142ms | 124.6× |
| string-validate-input | [SKIP] skip | — | — | — |

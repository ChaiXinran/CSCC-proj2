# SunSpider 1.0.2 — agentjs-string-check vs node

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
| string | 5 | 5 | — | — | — |
| **Total** | **26** | **6** | **0** | **0** | **0** |

## Per-Case Results

| Case | agentjs-string-check status | agentjs-string-check median | node median | agentjs-string-check/node |
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
| regexp-dna | [PASS] pass | 3187ms | 72ms | 44.6× |
| string-base64 | [PASS] pass | 872ms | 67ms | 13.0× |
| string-fasta | [PASS] pass | 712ms | 54ms | 13.2× |
| string-tagcloud | [PASS] pass | 8249ms | 64ms | 128.3× |
| string-unpack-code | [PASS] pass | 12.5s | 59ms | 213.6× |
| string-validate-input | [PASS] pass | 1450ms | 54ms | 26.8× |

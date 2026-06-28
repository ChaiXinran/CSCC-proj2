# SunSpider 1.0.2 — agentjs-string-after-opt vs node

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

| Case | agentjs-string-after-opt status | agentjs-string-after-opt median | node median | agentjs-string-after-opt/node |
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
| regexp-dna | [PASS] pass | 3608ms | 52ms | 69.5× |
| string-base64 | [PASS] pass | 482ms | 130ms | 3.7× |
| string-fasta | [PASS] pass | 673ms | 107ms | 6.3× |
| string-tagcloud | [PASS] pass | 3363ms | 55ms | 61.0× |
| string-unpack-code | [PASS] pass | 9081ms | 60ms | 152.1× |
| string-validate-input | [PASS] pass | 568ms | 54ms | 10.5× |

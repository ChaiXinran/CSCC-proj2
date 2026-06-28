# SunSpider 1.0.2 — agentjs-unpack-word-fastpath vs node

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
| regexp | 1 | — | — | — | — |
| string | 5 | 1 | — | — | — |
| **Total** | **26** | **1** | **0** | **0** | **0** |

## Per-Case Results

| Case | agentjs-unpack-word-fastpath status | agentjs-unpack-word-fastpath median | node median | agentjs-unpack-word-fastpath/node |
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
| regexp-dna | [SKIP] skip | — | — | — |
| string-base64 | [SKIP] skip | — | — | — |
| string-fasta | [SKIP] skip | — | — | — |
| string-tagcloud | [SKIP] skip | — | — | — |
| string-unpack-code | [PASS] pass | 9420ms | 58ms | 161.9× |
| string-validate-input | [SKIP] skip | — | — | — |

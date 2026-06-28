# SunSpider 1.0.2 — agentjs vs boa

## Correctness Summary

| Category | Cases | Pass | Wrong | Error | Timeout |
|:---|---:|---:|---:|---:|---:|
| 3d | 3 | 1 | — | — | — |
| access | 4 | 2 | — | — | — |
| bitops | 4 | 2 | — | — | — |
| controlflow | 1 | — | — | — | — |
| crypto | 3 | — | — | — | — |
| date | 2 | — | — | — | — |
| math | 3 | 1 | — | — | — |
| regexp | 1 | 1 | — | — | — |
| string | 5 | 2 | — | — | — |
| **Total** | **26** | **9** | **0** | **0** | **0** |

## Per-Case Results

| Case | agentjs status | agentjs median | boa median | agentjs/boa |
|:---|---:|---:|---:|---:|
| 3d-cube | [SKIP] skip | — | — | — |
| 3d-morph | [PASS] pass | 176ms | 63ms | 2.8× |
| 3d-raytrace | [SKIP] skip | — | — | — |
| access-binary-trees | [SKIP] skip | — | — | — |
| access-fannkuch | [PASS] pass | 609ms | 127ms | 4.8× |
| access-nbody | [SKIP] skip | — | — | — |
| access-nsieve | [PASS] pass | 228ms | 111ms | 2.1× |
| bitops-3bit-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bitwise-and | [PASS] pass | 244ms | 266ms | 1.1× faster |
| bitops-nsieve-bits | [PASS] pass | 308ms | 76ms | 4.1× |
| controlflow-recursive | [SKIP] skip | — | — | — |
| crypto-aes | [SKIP] skip | — | — | — |
| crypto-md5 | [SKIP] skip | — | — | — |
| crypto-sha1 | [SKIP] skip | — | — | — |
| date-format-tofte | [SKIP] skip | — | — | — |
| date-format-xparb | [SKIP] skip | — | — | — |
| math-cordic | [SKIP] skip | — | — | — |
| math-partial-sums | [PASS] pass | 124ms | 100ms | 1.2× |
| math-spectral-norm | [SKIP] skip | — | — | — |
| regexp-dna | [PASS] pass | 2774ms | 99ms | 28.0× |
| string-base64 | [PASS] pass | 262ms | 56ms | 4.7× |
| string-fasta | [SKIP] skip | — | — | — |
| string-tagcloud | [SKIP] skip | — | — | — |
| string-unpack-code | [SKIP] skip | — | — | — |
| string-validate-input | [PASS] pass | 1119ms | 113ms | 9.9× |

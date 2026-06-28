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
| 3d-morph | [PASS] pass | 674ms | 327ms | 2.1× |
| 3d-raytrace | [SKIP] skip | — | — | — |
| access-binary-trees | [SKIP] skip | — | — | — |
| access-fannkuch | [PASS] pass | 1913ms | 618ms | 3.1× |
| access-nbody | [SKIP] skip | — | — | — |
| access-nsieve | [PASS] pass | 767ms | 428ms | 1.8× |
| bitops-3bit-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bitwise-and | [PASS] pass | 758ms | 860ms | 1.1× faster |
| bitops-nsieve-bits | [PASS] pass | 1126ms | 379ms | 3.0× |
| controlflow-recursive | [SKIP] skip | — | — | — |
| crypto-aes | [SKIP] skip | — | — | — |
| crypto-md5 | [SKIP] skip | — | — | — |
| crypto-sha1 | [SKIP] skip | — | — | — |
| date-format-tofte | [SKIP] skip | — | — | — |
| date-format-xparb | [SKIP] skip | — | — | — |
| math-cordic | [SKIP] skip | — | — | — |
| math-partial-sums | [PASS] pass | 379ms | 335ms | 1.1× |
| math-spectral-norm | [SKIP] skip | — | — | — |
| regexp-dna | [PASS] pass | 7318ms | 313ms | 23.4× |
| string-base64 | [PASS] pass | 2040ms | 231ms | 8.8× |
| string-fasta | [SKIP] skip | — | — | — |
| string-tagcloud | [SKIP] skip | — | — | — |
| string-unpack-code | [SKIP] skip | — | — | — |
| string-validate-input | [PASS] pass | 2182ms | 380ms | 5.7× |

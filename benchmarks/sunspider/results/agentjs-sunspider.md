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
| 3d-morph | [PASS] pass | 555ms | 324ms | 1.7× |
| 3d-raytrace | [SKIP] skip | — | — | — |
| access-binary-trees | [SKIP] skip | — | — | — |
| access-fannkuch | [PASS] pass | 1568ms | 666ms | 2.4× |
| access-nbody | [SKIP] skip | — | — | — |
| access-nsieve | [PASS] pass | 545ms | 454ms | 1.2× |
| bitops-3bit-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bits-in-byte | [SKIP] skip | — | — | — |
| bitops-bitwise-and | [PASS] pass | 647ms | 993ms | 1.5× faster |
| bitops-nsieve-bits | [PASS] pass | 856ms | 425ms | 2.0× |
| controlflow-recursive | [SKIP] skip | — | — | — |
| crypto-aes | [SKIP] skip | — | — | — |
| crypto-md5 | [SKIP] skip | — | — | — |
| crypto-sha1 | [SKIP] skip | — | — | — |
| date-format-tofte | [SKIP] skip | — | — | — |
| date-format-xparb | [SKIP] skip | — | — | — |
| math-cordic | [SKIP] skip | — | — | — |
| math-partial-sums | [PASS] pass | 356ms | 373ms | 1.0× faster |
| math-spectral-norm | [SKIP] skip | — | — | — |
| regexp-dna | [PASS] pass | 7758ms | 294ms | 26.4× |
| string-base64 | [PASS] pass | 1922ms | 243ms | 7.9× |
| string-fasta | [SKIP] skip | — | — | — |
| string-tagcloud | [SKIP] skip | — | — | — |
| string-unpack-code | [SKIP] skip | — | — | — |
| string-validate-input | [PASS] pass | 2692ms | 358ms | 7.5× |

# SunSpider 1.0.2 — agentjs-video vs boa

## Correctness Summary

| Category | Cases | Pass | Wrong | Error | Timeout |
|:---|---:|---:|---:|---:|---:|
| 3d | 3 | 3 | — | — | — |
| access | 4 | 4 | — | — | — |
| bitops | 4 | 4 | — | — | — |
| controlflow | 1 | 1 | — | — | — |
| crypto | 3 | 3 | — | — | — |
| date | 2 | 2 | — | — | — |
| math | 3 | 3 | — | — | — |
| regexp | 1 | 1 | — | — | — |
| string | 5 | 5 | — | — | — |
| **Total** | **26** | **26** | **0** | **0** | **0** |

## Per-Case Results

| Case | agentjs-video status | agentjs-video median | boa median | agentjs-video/boa |
|:---|---:|---:|---:|---:|
| 3d-cube | [PASS] pass | 298ms | 63ms | 4.8× |
| 3d-morph | [PASS] pass | 273ms | 65ms | 4.2× |
| 3d-raytrace | [PASS] pass | 423ms | 75ms | 5.7× |
| access-binary-trees | [PASS] pass | 564ms | 61ms | 9.2× |
| access-fannkuch | [PASS] pass | 380ms | 141ms | 2.7× |
| access-nbody | [PASS] pass | 174ms | 59ms | 2.9× |
| access-nsieve | [PASS] pass | 126ms | 124ms | 1.0× |
| bitops-3bit-bits-in-byte | [PASS] pass | 278ms | 40ms | 6.9× |
| bitops-bits-in-byte | [PASS] pass | 325ms | 54ms | 6.0× |
| bitops-bitwise-and | [PASS] pass | 300ms | 270ms | 1.1× |
| bitops-nsieve-bits | [PASS] pass | 233ms | 79ms | 3.0× |
| controlflow-recursive | [PASS] pass | 391ms | 40ms | 9.9× |
| crypto-aes | [PASS] pass | 208ms | 76ms | 2.7× |
| crypto-md5 | [PASS] pass | 332ms | 44ms | 7.5× |
| crypto-sha1 | [PASS] pass | 243ms | 41ms | 5.9× |
| date-format-tofte | [PASS] pass | 376ms | 88ms | 4.3× |
| date-format-xparb | [PASS] pass | 200ms | 54ms | 3.7× |
| math-cordic | [PASS] pass | 372ms | 68ms | 5.4× |
| math-partial-sums | [PASS] pass | 171ms | 106ms | 1.6× |
| math-spectral-norm | [PASS] pass | 237ms | 42ms | 5.6× |
| regexp-dna | [PASS] pass | 126ms | 107ms | 1.2× |
| string-base64 | [PASS] pass | 180ms | 61ms | 2.9× |
| string-fasta | [PASS] pass | 740ms | 143ms | 5.2× |
| string-tagcloud | [PASS] pass | 432ms | 146ms | 3.0× |
| string-unpack-code | [PASS] pass | 606ms | 282ms | 2.1× |
| string-validate-input | [PASS] pass | 566ms | 119ms | 4.8× |

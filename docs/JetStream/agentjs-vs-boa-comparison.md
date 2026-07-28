# SunSpider 1.0.2 — AgentJS vs Boa 性能对比

> 测试日期: 2026-07-25
> AgentJS: 3 次取中位数 (SunSpider 脚本) | Boa: 单次运行 (PowerShell 计时)

## 综合结果

| 测试 | AgentJS | Boa | AgentJS/Boa |
|:---|---:|---:|---:|
| 3d-cube | 582ms | 101ms | 5.8× |
| 3d-morph | 331ms | 99ms | 3.3× |
| 3d-raytrace | 711ms | 100ms | 7.1× |
| access-binary-trees | 821ms | 82ms | 10.0× |
| access-fannkuch | 1155ms | 180ms | 6.4× |
| access-nbody | 798ms | 102ms | 7.8× |
| access-nsieve | 422ms | 202ms | 2.1× |
| bitops-3bit-bits-in-byte | 622ms | 73ms | 8.5× |
| bitops-bits-in-byte | 752ms | 83ms | 9.1× |
| **bitops-bitwise-and** | **315ms** | **435ms** | **✅ 1.4× 更快** |
| bitops-nsieve-bits | 593ms | 117ms | 5.1× |
| controlflow-recursive | 586ms | 63ms | 9.3× |
| crypto-aes | 354ms | 121ms | 2.9× |
| crypto-md5 | 482ms | 75ms | 6.4× |
| crypto-sha1 | 456ms | 62ms | 7.4× |
| date-format-tofte | 595ms | 138ms | 4.3× |
| date-format-xparb | 252ms | 85ms | 3.0× |
| math-cordic | 902ms | 128ms | 7.0× |
| math-partial-sums | 214ms | 167ms | 1.3× |
| math-spectral-norm | 447ms | 58ms | 7.7× |
| regexp-dna | 4110ms | 160ms | 25.7× |
| string-base64 | 459ms | 78ms | 5.9× |
| string-fasta | 1114ms | 213ms | 5.2× |
| string-tagcloud | 4358ms | 221ms | 19.7× |
| string-unpack-code | 12.0s | 474ms | 25.3× |
| string-validate-input | 1161ms | 183ms | 6.3× |

## 按类别汇总

| 类别 | AgentJS 总耗时 | Boa 总耗时 | 平均倍数 |
|:---|---:|---:|---:|
| 3d | 1624ms | 300ms | 5.4× |
| access | 3196ms | 566ms | 5.6× |
| bitops | 2282ms | 708ms | 3.2× |
| controlflow | 586ms | 63ms | 9.3× |
| crypto | 1292ms | 258ms | 5.0× |
| date | 847ms | 223ms | 3.8× |
| math | 1563ms | 353ms | 4.4× |
| regexp | 4110ms | 160ms | 25.7× |
| string | 19.1s | 1169ms | 16.3× |

## 关键发现

1. **唯一超越 Boa 的测试**: `bitops-bitwise-and` — AgentJS 比 Boa 快 **1.4×**
2. **性能接近的测试** (3× 以内): `access-nsieve` (2.1×), `math-partial-sums` (1.3×), `crypto-aes` (2.9×)
3. **最大差距**: `regexp-dna` (25.7×) 和 `string-unpack-code` (25.3×) — 字符串/正则操作为主要瓶颈
4. **整体评估**: AgentJS 作为轻量级研究运行时，在大多数测试中比 Boa 慢 3-10×，符合预期

---

> ⚠️ 注意: Boa 数据为单次运行（未取中位数），可能略有波动。AgentJS 数据来自 SunSpider 标准脚本（3 次取中位数）。

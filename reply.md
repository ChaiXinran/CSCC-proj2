# 尝试跑 SunSpider 基准测试

## 1. 下载 SunSpider 测试文件

方式一：从 GitHub 镜像拉取
```sh
git clone https://github.com/kangax/sunspider.git /tmp/sunspider
```

方式二：如果镜像不行，手动下载
```sh
# 网上搜索 "sunspider benchmark js files download"
# 或从 WebKit 源码里提取 PerformanceTests/SunSpider/
```

## 2. 用我们的引擎跑单个测试文件

```sh
cargo build --release --no-default-features
./target/release/agentjs run /tmp/sunspider/tests/sunspider-1.0/3d-cube.js
```

如果跑得通，可以写一个脚本批量跑所有测试并计时。

## 3. 批量跑所有测试（参考脚本）

```sh
# 遍历所有 js 文件，用 agentjs run 跑并计时
for f in /tmp/sunspider/tests/sunspider-1.0/*.js; do
    name=$(basename "$f")
    echo -n "$name: "
    start=$(date +%s%N)
    ./target/release/agentjs run "$f" 2>/dev/null && \
    end=$(date +%s%N) && \
    echo "scale=2; ($end - $start) / 1000000" | bc
done
```

如果 SunSpider 也跑不通，我们就在 reply.md 里记录哪些文件跑通了、哪些报了什么错。

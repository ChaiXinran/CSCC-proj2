// AgentJS 示例: 斐波那契递归计算
// 运行: cargo run -- run examples/fibonacci.js

function fibonacci(n) {
    if (n <= 1) {
        return n;
    }
    return fibonacci(n - 1) + fibonacci(n - 2);
}

// 测试用例
var n = 10;
var result = fibonacci(n);
console.log("fibonacci(" + n + ") = " + result);

// 验证结果
var expected = 55;
console.log("期望结果: " + expected);
console.log("结果正确: " + (result === expected));

// 打印前 10 项
console.log("--- 斐波那契数列前 10 项 ---");
var i = 0;
while (i < 10) {
    console.log("fibonacci(" + i + ") = " + fibonacci(i));
    i = i + 1;
}

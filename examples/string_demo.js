// AgentJS 示例: 字符串处理
// 运行: cargo run --release -- run examples/string_demo.js

// 自定义函数：反转字符串
function reverse(str) {
    var result = "";
    var i = str.length - 1;
    while (i >= 0) {
        result = result + str[i];
        i = i - 1;
    }
    return result;
}

// 自定义函数：统计元音字母
function countVowels(str) {
    var count = 0;
    var vowels = "aeiouAEIOU";
    var i = 0;
    while (i < str.length) {
        var j = 0;
        while (j < vowels.length) {
            if (str[i] === vowels[j]) {
                count = count + 1;
            }
            j = j + 1;
        }
        i = i + 1;
    }
    return count;
}

// ── 主程序 ──
var text = "Hello, AgentJS!";
console.log("原始字符串: " + text);
console.log("长度: " + text.length);
console.log("大写: " + text.toUpperCase());
console.log("小写: " + text.toLowerCase());
console.log("反转: " + reverse(text));
console.log("元音数: " + countVowels(text));

// JSON 处理
var jsonStr = '{"name":"AgentJS","type":"JS Engine","lang":"Rust"}';
var obj = JSON.parse(jsonStr);
console.log("--- JSON 解析 ---");
console.log("名称: " + obj.name);
console.log("类型: " + obj.type);
console.log("语言: " + obj.lang);
console.log("JSON 序列化: " + JSON.stringify(obj));

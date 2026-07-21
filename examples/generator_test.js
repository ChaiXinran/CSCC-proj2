// 验证 Generator.prototype 是否生效
function* gen() {
    yield 1;
    yield 2;
    yield 3;
}

var g = gen();
console.log(g.next().value);  // 期望 1
console.log(g.next().value);  // 期望 2
console.log(g.next().value);  // 期望 3
console.log(g.next().done);   // 期望 true

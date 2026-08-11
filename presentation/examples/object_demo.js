// AgentJS 对象示例
// 运行: cargo run --release -- run examples/object_demo.js

// 创建对象字面量
var person = {
    name: "Alice",
    age: 30,
    greet: function() {
        return "Hi, I'm " + this.name;
    }
};

// 读取属性
console.log(person.name);
console.log(person.age);

// 调用方法
console.log(person.greet());

// 修改属性
person.age = 31;
console.log(person.age);

// 添加新属性
person.city = "Tokyo";
console.log(person.city);

// 计算属性访问
var key = "name";
console.log(person[key]);

// 嵌套对象
var company = {
    name: "AgentJS Corp",
    address: {
        city: "Shanghai",
        zip: 200000
    }
};
console.log(company.address.city);

// Object.keys 内建方法
var keys = Object.keys(person);
console.log(keys);

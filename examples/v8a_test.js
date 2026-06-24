// V8-A feature tests

// 1. Template literals
var name = "world";
print(`hello ${name}!`);
print(`1+1=${1+1}`);
print(`a${1}b${2}c`);

// 2. Rest parameters
function sum(...args) {
  var result = 0;
  for (var i = 0; i < args.length; i++) result += args[i];
  return result;
}
print(sum(1, 2, 3, 4));  // 10

// 3. Spread in calls
var nums = [10, 20, 30];
print(Math.max(...nums));  // 30

// 4. Array spread
var a = [1, 2];
var b = [0, ...a, 3];
print(b[0], b[1], b[2], b[3]);  // 0 1 2 3

// 5. Array destructuring
var [x, y, z] = [10, 20, 30];
print(x, y, z);  // 10 20 30

// 6. Object destructuring
var { a: p, b: q } = { a: 100, b: 200 };
print(p, q);  // 100 200

// 7. Class declarations
class Animal {
  constructor(name) {
    this.name = name;
  }
  speak() {
    return this.name + " says hello";
  }
}
var dog = new Animal("Dog");
print(dog.speak());  // Dog says hello
print(dog.name);     // Dog

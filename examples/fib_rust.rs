// 直接用 Rust 写斐波那契递归
// 这只是个演示文件，不是项目的一部分
// 你可以用 rustc examples/fib_rust.rs && fib_rust.exe 来运行

fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    let n = 10;
    let result = fibonacci(n);
    println!("fibonacci({}) = {}", n, result);
}

use agentjs::{BackendKind, Engine, ExecutionOptions, RuntimeConfig};
fn main() {
  let e=Engine::with_backend(BackendKind::Native, RuntimeConfig::default());
  for s in [
    "function f() { return (() => arguments)().length; } f(1, 2);",
    "\"use strict\"; function f() { return (() => this)(); } f.call('this_val');",
    "function F() { return new.target === F; } new F();",
    "function F() { return (() => new.target)() === F; } new F();",
    "function F() { var nt = new.target; return (() => nt)() === F; } new F();",
  ] { println!("{} => {:?}", s, e.execute(s, ExecutionOptions::default()).map(|r| r.value)); }
}

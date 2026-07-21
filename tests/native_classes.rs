//! V13-B native class execution regression coverage.

use agentjs::{Engine, ExecutionOptions, RuntimeConfig};

fn run(source: &str) -> String {
    Engine::new(RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect("native evaluation succeeds")
        .value
}

#[test]
fn fields_and_static_blocks_follow_source_order() {
    assert_eq!(
        run(
            "let log = ''; class C { a = (log += 'a', 1); b = (log += 'b', this.a + 1); static x = (log += 's', 3); static { log += 't'; this.y = 4; } } let c = new C(); log + ':' + c.b + ':' + C.x + ':' + C.y"
        ),
        "stab:2:3:4"
    );
}

#[test]
fn computed_field_key_is_evaluated_once_at_class_definition() {
    assert_eq!(
        run(
            "let n = 0; let key = () => (n++, 'x'); class C { [key()] = 1; } let c = new C(); c.x + ':' + n"
        ),
        "1:1"
    );
}

#[test]
fn derived_constructor_calls_super_with_the_current_instance() {
    assert_eq!(
        run(
            "class Base { constructor(value) { this.base = value; } } class Derived extends Base { constructor(value) { super(value); this.derived = value + 1; } } let value = new Derived(2); value.base + value.derived"
        ),
        "5"
    );
}

#[test]
fn derived_constructor_forwards_spread_arguments_and_new_target() {
    assert_eq!(
        run(
            "class Base { constructor(...values) { this.count = values.length; this.target = new.target; } } class Derived extends Base { constructor() { super(...[1, 2]); } } let value = new Derived(); value.count + ':' + (value.target === Derived)"
        ),
        "2:true"
    );
}

#[test]
fn derived_constructor_rejects_this_before_super() {
    let error = Engine::new(RuntimeConfig::default())
        .execute(
            "class Base {} class Derived extends Base { constructor() { this.value = 1; super(); } } new Derived()",
            ExecutionOptions::default(),
        )
        .expect_err("derived this is uninitialized before super()");
    assert_eq!(error.kind.name(), "ReferenceError");
}

#[test]
fn derived_constructor_must_initialize_this_before_implicit_return() {
    let error = Engine::new(RuntimeConfig::default())
        .execute(
            "class Base {} class Derived extends Base { constructor() {} } new Derived()",
            ExecutionOptions::default(),
        )
        .expect_err("a derived constructor cannot implicitly return without super()");
    assert_eq!(error.kind.name(), "ReferenceError");
}

#[test]
fn default_derived_constructor_forwards_arguments_to_super() {
    assert_eq!(
        run(
            "class Base { constructor(value) { this.value = value; } } class Derived extends Base {} new Derived(9).value"
        ),
        "9"
    );
}

#[test]
fn private_field_rejects_an_unbranded_receiver() {
    let error = Engine::new(RuntimeConfig::default())
        .execute(
            "class C { #value = 1; read() { return this.#value; } } C.prototype.read.call({})",
            ExecutionOptions::default(),
        )
        .expect_err("private access must check the receiver");
    assert_eq!(error.kind.name(), "TypeError");
}

#[test]
fn derived_constructor_rejects_a_primitive_return_override() {
    let error = Engine::new(RuntimeConfig::default())
        .execute(
            "class Base {} class Derived extends Base { constructor() { return 1; } } new Derived()",
            ExecutionOptions::default(),
        )
        .expect_err("a derived constructor may only override this with an object");
    assert_eq!(error.kind.name(), "TypeError");
}

#[test]
fn class_extending_null_uses_a_null_instance_prototype_parent() {
    assert_eq!(
        run(
            "class C extends null { constructor() { return {}; } } Object.getPrototypeOf(C.prototype) === null"
        ),
        "true"
    );
}

#[test]
fn class_rejects_a_non_constructable_heritage_value() {
    let error = Engine::new(RuntimeConfig::default())
        .execute("class C extends (() => {}) {}", ExecutionOptions::default())
        .expect_err("an arrow function is not a constructor");
    assert_eq!(error.kind.name(), "TypeError");
}

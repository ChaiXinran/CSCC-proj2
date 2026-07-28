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
fn static_fields_and_blocks_resolve_super_with_the_class_as_receiver() {
    assert_eq!(
        run(
            "class Parent { static value = 7; static read() { return this.value + 1; } } class Child extends Parent { static field = super.value; static { this.block = super.read(); } } Child.field + ':' + Child.block"
        ),
        "7:8"
    );
}

#[test]
fn static_accessors_resolve_super_data_accessors_and_methods() {
    assert_eq!(
        run(
            "class Parent { static value = 7; static get doubled() { return this.value * 2; } static read() { return this.value + 1; } } class Child extends Parent { static get result() { return super.doubled + ':' + super['read'](); } static set result(value) { this.seen = value + super.value; } } let first = Child.result; Child.result = 3; first + ':' + Child.seen"
        ),
        "14:8:10"
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
fn computed_static_and_instance_field_keys_follow_source_order() {
    assert_eq!(
        run(
            "let i = 0; class C { [i++] = i++; static [i++] = i++; [i++] = i++; } let c = new C(); c[0] + ':' + C[1] + ':' + c[2] + ':' + i"
        ),
        "4:3:5:6"
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
fn equal_private_spellings_from_distinct_classes_have_distinct_brands() {
    let error = Engine::new(RuntimeConfig::default())
        .execute(
            "class A { #x = 'a'; read(value) { return value.#x; } } class B { #x = 'b'; } new A().read(new B())",
            ExecutionOptions::default(),
        )
        .expect_err("the spelling #x must not identify a different class's field");
    assert_eq!(error.kind.name(), "TypeError");
}

#[test]
fn repeated_evaluation_of_one_class_expression_allocates_fresh_brands() {
    let error = Engine::new(RuntimeConfig::default())
        .execute(
            "function make() { return class { #x = 1; read(value) { return value.#x; } }; } let A = make(); let B = make(); new A().read(new B())",
            ExecutionOptions::default(),
        )
        .expect_err("each class evaluation must allocate a new private brand");
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
            "class C extends null { constructor() { return {}; } } (Object.getPrototypeOf(C.prototype) === null) + ':' + (Object.getPrototypeOf(C) === Function.prototype)"
        ),
        "true:true"
    );
}

#[test]
fn class_rejects_a_non_constructable_heritage_value() {
    let error = Engine::new(RuntimeConfig::default())
        .execute("class C extends (() => {}) {}", ExecutionOptions::default())
        .expect_err("an arrow function is not a constructor");
    assert_eq!(error.kind.name(), "TypeError");
}

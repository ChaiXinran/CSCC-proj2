use agentjs::{Engine, ExecutionOptions, RuntimeConfig};

fn run(source: &str) -> String {
    Engine::new(RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect("native evaluation succeeds")
        .value
}

#[test]
fn array_of_uses_a_constructable_receiver_and_defines_length() {
    assert_eq!(
        run(
            "function Bag(length) { this.constructedWith = length; } let value = Array.of.call(Bag, 1, 2, 3); [value instanceof Bag, value.constructedWith, value.length, value[0], value[2]].join(',')"
        ),
        "true,3,3,1,3"
    );
}

#[test]
fn array_of_falls_back_to_an_array_for_non_constructors() {
    assert_eq!(
        run("let value = Array.of.call({}, 1, 2); [Array.isArray(value), value.length].join(',')"),
        "true,2"
    );
}

#[test]
fn array_iteration_preserves_element_getter_exceptions() {
    assert_eq!(
        run(
            "let object = { length: 3 }; Object.defineProperty(object, '1', { get() { throw new RangeError('element'); } }); let count = 0; let result; try { Array.prototype.every.call(object, () => { count++; return true; }); } catch (error) { result = [error instanceof RangeError, count].join(','); } result"
        ),
        "true,0"
    );
}

#[test]
fn copy_within_preserves_holes_and_uses_internal_delete() {
    assert_eq!(
        run(
            "let array = [1, , 3]; array.copyWithin(0, 1, 3); [Object.hasOwn(array, '0'), array[1], array.length].join(',')"
        ),
        "false,3,3"
    );
}

#[test]
fn array_identity_recurses_through_proxies() {
    assert_eq!(
        run(
            "let array = []; let proxy = new Proxy(new Proxy(array, {}), {}); [Array.isArray(proxy), Array.prototype.concat.call([], proxy).length].join(',')"
        ),
        "true,0"
    );
}

#[test]
fn dense_push_and_pop_preserve_values_and_length() {
    assert_eq!(
        run(
            "let array = []; for (let i = 0; i < 1000; i++) array.push(i); let last = array.pop(); [last, array.length, array[998]].join(',')"
        ),
        "999,999,998"
    );
}

#[test]
fn push_fast_path_defers_to_an_inherited_index_setter() {
    assert_eq!(
        run(
            "let seen = -1; Object.defineProperty(Array.prototype, '0', { set(value) { seen = value; }, configurable: true }); let array = []; array.push(7); [seen, Object.hasOwn(array, '0'), array.length].join(',')"
        ),
        "7,false,1"
    );
}

#[test]
fn pop_fast_path_defers_to_prototype_lookup_for_a_hole() {
    assert_eq!(
        run(
            "Array.prototype[0] = 9; let array = new Array(1); let value = array.pop(); [value, array.length].join(',')"
        ),
        "9,0"
    );
}

#[test]
fn array_prototype_method_cache_is_invalidated_by_redefinition() {
    assert_eq!(
        run(
            "let array = []; array.push(1); Array.prototype.push = function(value) { return value + 40; }; [array.push(2), array.length].join(',')"
        ),
        "42,1"
    );
}

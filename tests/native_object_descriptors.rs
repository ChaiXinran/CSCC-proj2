use agentjs::{Engine, ExecutionOptions, RuntimeConfig};

fn run(source: &str) -> String {
    Engine::new(RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .expect("native evaluation succeeds")
        .value
}

#[test]
fn numeric_property_keys_use_ecmascript_number_spelling() {
    assert_eq!(
        run(
            "let o = {}; for (let key of [-0, Infinity, -Infinity, 1e21, 1e22, 1e-7, 1e-8]) Object.defineProperty(o, key, { value: String(key) }); [Object.getOwnPropertyDescriptor(o, -0).value, o.Infinity, o['-Infinity'], o['1e+21'], o['1e+22'], o['1e-7'], o['1e-8']].join('|')"
        ),
        "0|Infinity|-Infinity|1e+21|1e+22|1e-7|1e-8"
    );
}

#[test]
fn property_descriptor_reads_inherited_fields_in_spec_order() {
    assert_eq!(
        run(
            "let log = []; let proto = {}; Object.defineProperty(proto, 'enumerable', { get() { log.push('enumerable'); } }); Object.defineProperty(proto, 'configurable', { get() { log.push('configurable'); } }); Object.defineProperty(proto, 'value', { get() { log.push('value'); return 42; } }); Object.defineProperty(proto, 'writable', { get() { log.push('writable'); } }); Object.defineProperty(proto, 'get', { get() { log.push('get'); } }); Object.defineProperty(proto, 'set', { get() { log.push('set'); } }); try { Object.defineProperty({}, 'x', Object.create(proto)); } catch (e) {} log.join(',')"
        ),
        "enumerable,configurable,value,writable,get,set"
    );
}

#[test]
fn property_descriptor_rejects_mixed_data_and_accessor_fields() {
    assert_eq!(
        run(
            "let count = 0; try { Object.defineProperty({}, 'x', { value: 1, get() {} }); } catch (e) { count += e instanceof TypeError; } try { Object.defineProperties({}, { x: { writable: true, set(v) {} } }); } catch (e) { count += e instanceof TypeError; } count"
        ),
        "2"
    );
}

#[test]
fn define_properties_coerces_properties_and_converts_before_defining() {
    assert_eq!(
        run(
            "let target = {}; let count = 0; for (let value of [false, 1, '']) count += Object.defineProperties(target, value) === target; try { Object.defineProperties(target, null); } catch (e) { count += e instanceof TypeError; } let descriptors = { a: { value: 1 }, b: { get: 1 } }; try { Object.defineProperties(target, descriptors); } catch (e) { count += !Object.hasOwn(target, 'a'); } count"
        ),
        "5"
    );
}

#[test]
fn group_by_creates_null_prototype_groups_and_passes_index() {
    run(r#"
        const seen = [];
        const grouped = Object.groupBy([1, 2, 3], (value, index) => {
            seen.push(value, index);
            return value % 2 ? "odd" : "even";
        });
        if (Object.getPrototypeOf(grouped) !== null) throw new Error("prototype");
        if (seen.join(",") !== "1,0,2,1,3,2") throw new Error("callback args");
        if (grouped.odd.join(",") !== "1,3") throw new Error("odd group");
        if (grouped.even.join(",") !== "2") throw new Error("even group");
        "#);
}

#[test]
fn get_own_property_descriptors_includes_symbols() {
    run(r#"
        const symbol = Symbol("descriptor");
        const object = {};
        Object.defineProperty(object, symbol, {
            value: 7,
            enumerable: false,
            configurable: true
        });
        const descriptors = Object.getOwnPropertyDescriptors(object);
        if (descriptors[symbol].value !== 7) throw new Error("symbol value");
        if (descriptors[symbol].enumerable !== false) throw new Error("symbol flags");
        "#);
}

#[test]
fn from_entries_reads_value_before_coercing_key_and_closes_on_error() {
    assert_eq!(
        run(
            "let log = []; let iterable = { [Symbol.iterator]() { return { next() { return { done: false, value: { get 0() { log.push('key'); return { toString() { log.push('coerce'); return 'x'; } }; }, get 1() { log.push('value'); throw new RangeError('stop'); } } }; }, return() { log.push('close'); return {}; } }; } }; try { Object.fromEntries(iterable); } catch (error) { log.push(error instanceof RangeError); } log.join(',')"
        ),
        "key,value,close,true"
    );
}

#[test]
fn array_length_descriptors_use_observable_number_coercion() {
    assert_eq!(
        run(
            "let log = []; let array = []; Object.defineProperty(array, 'length', { value: { valueOf() { log.push('valueOf'); return 2; } } }); Object.defineProperties(array, { length: { value: { toString() { log.push('toString'); return '3'; } } } }); [array.length, log.join(',')].join('|')"
        ),
        "3|valueOf,toString"
    );
}

#[test]
fn to_string_tag_can_override_ordinary_and_primitive_prototypes() {
    assert_eq!(
        run(
            "let object = {}; object[Symbol.toStringTag] = 'Custom'; Boolean.prototype[Symbol.toStringTag] = 'Flag'; [Object.prototype.toString.call(object), Object.prototype.toString.call(true)].join('|')"
        ),
        "[object Custom]|[object Flag]"
    );
}

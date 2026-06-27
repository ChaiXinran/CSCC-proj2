use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn collection_and_iterator_globals_are_installed() {
    assert_eq!(
        native_eval(
            "typeof Map + ':' + typeof Set + ':' + typeof WeakMap + ':' + \
             typeof WeakSet + ':' + typeof Iterator + ':' + \
             Object.getOwnPropertyDescriptor(Map, 'prototype').writable + ':' + \
             (Object.getOwnPropertyDescriptor(Map, Symbol.species).get.call(Set) === Set) + ':' + \
             (Set.prototype.keys === Set.prototype.values) + ':' + \
             Object.prototype.toString.call(new Map()) + ':' + \
             Object.prototype.toString.call(new Set());"
        ),
        "function:function:function:function:function:false:true:true:[object Map]:[object Set]"
    );
}

#[test]
fn map_supports_basic_storage_and_same_value_zero_keys() {
    assert_eq!(
        native_eval(
            "var key = {}; \
             var m = new Map([['a', 1], [key, 2], [NaN, 3]]); \
             m.set('a', 4).set(-0, 5); \
             m.size + ':' + m.get('a') + ':' + m.get(key) + ':' + m.get(NaN) + ':' + \
             m.has(0) + ':' + m.delete('a') + ':' + m.has('a') + ':' + m.size;"
        ),
        "4:4:2:3:true:true:false:3"
    );
}

#[test]
fn map_and_set_accept_null_or_undefined_initializers() {
    assert_eq!(
        native_eval("new Map(null).size + ':' + new Set(undefined).size;"),
        "0:0"
    );
}

#[test]
fn map_iterators_preserve_insertion_order() {
    assert_eq!(
        native_eval(
            "var m = new Map(); \
             m.set('a', 1); m.set('b', 2); \
             var i = m.entries(); \
             var a = i.next().value; \
             var b = i.next().value; \
             a[0] + a[1] + ':' + b[0] + b[1] + ':' + i.next().done;"
        ),
        "a1:b2:true"
    );
}

#[test]
fn set_supports_basic_storage_and_iteration() {
    assert_eq!(
        native_eval(
            "var s = new Set([1, 2, 1]); \
             s.add(3); \
             var removed = s.delete(2); \
             var i = s.values(); \
             s.size + ':' + s.has(1) + ':' + removed + ':' + s.has(2) + ':' + \
             i.next().value + ':' + i.next().value + ':' + i.next().done;"
        ),
        "2:true:true:false:1:3:true"
    );
}

#[test]
fn map_and_set_for_each_use_collection_callback_order() {
    assert_eq!(
        native_eval(
            "var out = ''; \
             new Map([['x', 1], ['y', 2]]).forEach(function(value, key) { out = out + key + value; }); \
             new Set(['a', 'b']).forEach(function(value, key) { out = out + key + value; }); \
             out;"
        ),
        "x1y2aabb"
    );
}

#[test]
fn weak_collections_support_object_keys_and_reject_strong_keys() {
    assert_eq!(
        native_eval(
            "var k = {}; var k2 = {}; \
             var wm = new WeakMap([[k, 7]]); wm.set(k2, 8); \
             var ws = new WeakSet([k]); ws.add(k2); \
             var caughtMap = false; var caughtSet = false; \
             try { wm.set('x', 1); } catch (e) { caughtMap = e.name === 'TypeError'; } \
             try { ws.add(1); } catch (e) { caughtSet = e.name === 'TypeError'; } \
             wm.get(k) + ':' + wm.has(k2) + ':' + wm.delete(k) + ':' + wm.has(k) + ':' + \
             ws.has(k) + ':' + ws.delete(k2) + ':' + ws.has(k2) + ':' + caughtMap + ':' + caughtSet;"
        ),
        "7:true:true:false:true:true:false:true:true"
    );
}

#[test]
fn iterator_helpers_cover_eager_collection_iterators() {
    assert_eq!(
        native_eval(
            "var m = new Map([['a', 1], ['b', 2]]); \
             var keys = m.keys().toArray(); \
             var found = m.values().find(function(value) { return value === 2; }); \
             var some = new Set([1, 3]).values().some(function(value) { return value > 2; }); \
             var every = new Set([1, 3]).values().every(function(value) { return value > 0; }); \
             var it = m.keys(); \
             keys.join(',') + ':' + found + ':' + some + ':' + every + ':' + (Iterator.from(it) === it);"
        ),
        "a,b:2:true:true:true"
    );
}

#[test]
fn iterator_static_concat_zip_and_zip_keyed_cover_basic_sequences() {
    assert_eq!(
        native_eval(
            "var c = Iterator.concat([1, 2], ['a', 'b']).toArray(); \
             var z = Iterator.zip([[1, 2], ['a', 'b']]).toArray(); \
             var k = Iterator.zipKeyed({ x: [1], y: ['a'] }).next().value; \
             c.join(',') + ':' + z[0][0] + z[0][1] + z[1][0] + z[1][1] + ':' + \
             k.x + k.y + ':' + (Object.getPrototypeOf(k) === null);"
        ),
        "1,2,a,b:1a2b:1a:true"
    );
}

#[test]
fn iterator_from_wraps_arrays_and_iterables() {
    assert_eq!(
        native_eval(
            "var fromArray = Array.from(Iterator.from([1, 2, 3])).join(','); \
             var custom = {}; \
             custom[Symbol.iterator] = function() { \
               var i = 0; \
               return { next: function() { i = i + 1; return { value: i, done: i > 2 }; } }; \
             }; \
             fromArray + ':' + Array.from(Iterator.from(custom)).join(',');"
        ),
        "1,2,3:1,2"
    );
}

#[test]
fn iterator_from_wraps_plain_iterators_and_nullish_iterator_methods() {
    assert_eq!(
        native_eval(
            "function make(method) { \
               var i = 0; \
               var iter = { next: function() { return { value: i++, done: i > 4 }; } }; \
               if (method !== 'absent') iter[Symbol.iterator] = method; \
               return Array.from(Iterator.from(iter)).join(','); \
             } \
             var bad = false; \
             try { Iterator.from({ next: function() {}, [Symbol.iterator]: 0 }); } \
             catch (e) { bad = e.name === 'TypeError'; } \
             make('absent') + ':' + make(null) + ':' + make(undefined) + ':' + bad;"
        ),
        "0,1,2,3:0,1,2,3:0,1,2,3:true"
    );
}

#[test]
fn array_iterator_exposes_iterator_prototype_symbol_iterator() {
    assert_eq!(
        native_eval(
            "var iter = [1, 2][Symbol.iterator](); \
             var iteratorPrototype = Object.getPrototypeOf(Object.getPrototypeOf(iter)); \
             var fn = iteratorPrototype[Symbol.iterator]; \
             fn.name + ':' + fn.call(5) + ':' + Array.from(iter).join(',');"
        ),
        "[Symbol.iterator]:5:1,2"
    );
}

#[test]
fn iterator_prototype_symbol_dispose_calls_return() {
    assert_eq!(
        native_eval(
            "var iteratorPrototype = Object.getPrototypeOf(Object.getPrototypeOf([].values())); \
             var iter = Object.create(iteratorPrototype); \
             var called = false; \
             iter.return = function() { called = true; return { done: true }; }; \
             var result = iter[Symbol.dispose](); \
             called + ':' + (result === undefined) + ':' + iter[Symbol.dispose].name;"
        ),
        "true:true:[Symbol.dispose]"
    );
}

#[test]
fn iterator_pipeline_helpers_cover_basic_eager_results() {
    assert_eq!(
        native_eval(
            "var mapped = new Set([1, 2, 3]).values().map(function(value) { return value * 2; }).toArray(); \
             var filtered = new Set([1, 2, 3, 4]).values().filter(function(value) { return value % 2 === 0; }).toArray(); \
             var taken = new Set([1, 2, 3]).values().take(2).toArray(); \
             var dropped = new Set([1, 2, 3]).values().drop(1).toArray(); \
             var reduced = new Set([1, 2, 3]).values().reduce(function(acc, value) { return acc + value; }, 0); \
             mapped.join(',') + ':' + filtered.join(',') + ':' + taken.join(',') + ':' + dropped.join(',') + ':' + reduced;"
        ),
        "2,4,6:2,4:1,2:2,3:6"
    );
}

#[test]
fn generator_objects_are_iterable() {
    assert_eq!(
        native_eval(
            "function* prefixes(s) { \
               for (var i = 0; i <= s.length; ++i) { yield s.slice(0, i); } \
             } \
             var out = ''; \
             for (var prefix of prefixes('ab')) { out = out + '[' + prefix + ']'; } \
             out;"
        ),
        "[][a][ab]"
    );
}

#[test]
fn iterator_concat_is_lazy_caches_methods_and_forwards_return() {
    assert_eq!(
        native_eval(
            "var gets = 0; var opens = 0; var closes = 0; var nexts = 0; \
             var iterable = {}; \
             Object.defineProperty(iterable, Symbol.iterator, { get: function() { \
               gets++; return function() { opens++; return { \
                 next: function() { nexts++; return { value: nexts, done: false }; }, \
                 return: function() { closes++; return {}; } \
               }; }; \
             } }); \
             var iterator = Iterator.concat(iterable); \
             var before = gets + ':' + opens + ':' + nexts; \
             var first = iterator.next().value; iterator.return(); iterator.return(); \
             before + ':' + first + ':' + gets + ':' + opens + ':' + nexts + ':' + closes;"
        ),
        "1:0:0:1:1:1:1:1"
    );
}

#[test]
fn iterator_pipeline_helpers_are_lazy_and_forward_close() {
    assert_eq!(
        native_eval(
            "var advances = 0; \
             function* values() { \
               advances = advances + 1; yield 1; \
               advances = advances + 1; yield 2; \
             } \
             var source = values(); \
             var mapped = source.map(function(value) { return value * 10; }); \
             var before = advances; \
             source.next(); \
             var mappedValue = mapped.next().value; \
             var filteredValue = values().filter(function(value) { return value > 1; }).next().value; \
             var flat = values().flatMap(function(value) { return [value, value + 10]; }); \
             var flatValues = '' + flat.next().value + ',' + flat.next().value + ',' + flat.next().value; \
             var closed = 0; var index = 0; \
             var closable = Object.create(Iterator.prototype); \
             closable.next = function() { return { value: index++, done: false }; }; \
             closable.return = function() { closed = closed + 1; return {}; }; \
             var taken = closable.take(1); taken.next(); taken.next(); \
             before + ':' + mappedValue + ':' + filteredValue + ':' + flatValues + ':' + closed;"
        ),
        "0:20:2:1,11,2:1"
    );
}

#[test]
fn generator_objects_inherit_iterator_helpers() {
    assert_eq!(
        native_eval(
            "function* values() { yield 1; yield 2; yield 3; } \
             var iterator = values(); \
             (Object.getPrototypeOf(Object.getPrototypeOf(iterator)) === Iterator.prototype) + ':' + \
             iterator.every(function(value) { return value > 0; });"
        ),
        "true:true"
    );
}

#[test]
fn terminal_iterator_helpers_close_on_early_exit_and_throw() {
    assert_eq!(
        native_eval(
            "var closed = 0; \
             function make() { \
               var iterator = Object.create(Iterator.prototype); \
               iterator.next = function() { return { value: 1, done: false }; }; \
               iterator.return = function() { closed = closed + 1; return {}; }; \
               return iterator; \
             } \
             var early = make().every(function() { return false; }); \
             var thrown = ''; \
             try { make().some(function() { throw 'boom'; }); } catch (error) { thrown = error; } \
             var invalid = false; \
             try { make().find(); } catch (error) { invalid = error.name === 'TypeError'; } \
             early + ':' + thrown + ':' + invalid + ':' + closed;"
        ),
        "false:boom:true:3"
    );
}

#[test]
fn terminal_iterator_helpers_read_next_through_getter() {
    assert_eq!(
        native_eval(
            "var reads = 0; var index = 0; var iterator = Object.create(Iterator.prototype); Object.defineProperty(iterator, 'next', { get: function() { reads = reads + 1; return function() { return index < 2 ? { value: ++index, done: false } : { done: true }; }; }}); var values = iterator.toArray(); reads + ':' + values.join(',');"
        ),
        "1:1,2"
    );
}

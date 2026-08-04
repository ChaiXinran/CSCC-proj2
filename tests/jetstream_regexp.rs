use agentjs::{
    backend::NativeRuntime,
    engine::{ExecutionOptions, RuntimeConfig},
};

fn eval(source: &str) -> String {
    let mut runtime = NativeRuntime::new(RuntimeConfig::default());
    runtime
        .eval_source(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
}

#[test]
fn validatorjs_legacy_character_class_hyphen_matches_literals() {
    assert_eq!(
        eval(
            r#"
            const regexp = /[@_\- ]/g;
            const inputs = ["@", "_", "-", " "];
            let result = "";
            for (const input of inputs) {
                regexp.lastIndex = 0;
                result += regexp.test(input) ? "1" : "0";
            }
            result;
            "#
        ),
        "1111"
    );
}

#[test]
fn regexp_constructor_preserves_escaped_hyphen_in_legacy_classes() {
    assert_eq!(
        eval(
            r#"
            const patterns = ["[\\-]", "[a\\-z]", "[-az]", "[az-]"];
            patterns.map(pattern => new RegExp(pattern).test("-")).join("");
            "#
        ),
        "truetruetruetrue"
    );
}

#[test]
fn global_word_boundary_replace_keeps_noninitial_word_characters() {
    assert_eq!(eval(r#""nsgre".replace(/\b[a-z]/g, "");"#), "sgre");
}

#[test]
fn octane_regexp_checksum_matches_jetstream_reference() {
    let source = include_str!("../benchmarks/JetStream2/Octane/regexp.js");
    let mut instrumented = source.replace(
        "if (sum != expectedSum) throw new Error(\"Wrong checksum. Found \" + sum + \" but expected \" + expectedSum);",
        "globalThis.__agentjsRegexpChecksum = sum;",
    );
    assert_ne!(
        instrumented, source,
        "checksum hook must match benchmark source"
    );
    for index in 0..12 {
        instrumented = instrumented.replace(
            &format!("sum += runBlock{index}();"),
            &format!(
                "globalThis.__agentjsBlockSums ??= []; globalThis.__agentjsBlockSums[{index}] = runBlock{index}(); sum += globalThis.__agentjsBlockSums[{index}];"
            ),
        );
    }

    let deterministic_random = r#"
        let __agentjsSeed = 49734321;
        Math.random = function() {
            __agentjsSeed = ((__agentjsSeed + 0x7ed55d16) + (__agentjsSeed << 12)) & 0xffffffff;
            __agentjsSeed = ((__agentjsSeed ^ 0xc761c23c) ^ (__agentjsSeed >>> 19)) & 0xffffffff;
            __agentjsSeed = ((__agentjsSeed + 0x165667b1) + (__agentjsSeed << 5)) & 0xffffffff;
            __agentjsSeed = ((__agentjsSeed + 0xd3a2646c) ^ (__agentjsSeed << 9)) & 0xffffffff;
            __agentjsSeed = ((__agentjsSeed + 0xfd7046c5) + (__agentjsSeed << 3)) & 0xffffffff;
            __agentjsSeed = ((__agentjsSeed ^ 0xb55a4f09) ^ (__agentjsSeed >>> 16)) & 0xffffffff;
            return (__agentjsSeed & 0xfffffff) / 0x10000000;
        };
    "#;
    assert_eq!(
        eval(&format!(
            "{deterministic_random}\n{instrumented}\nJSON.stringify(__agentjsBlockSums);"
        )),
        "[286139,1212496,5429,25168,11931,4500,77823,6108,3227,13299,6849,13140]"
    );
}

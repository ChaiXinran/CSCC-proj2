use agentjs::{ExecutionOptions, Runtime, RuntimeConfig};

fn native_eval(source: &str) -> String {
    Runtime::new(RuntimeConfig::default())
        .expect("native runtime should initialize")
        .eval(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn bigint_strings_and_arithmetic_are_arbitrary_precision() {
    assert_eq!(
        native_eval(
            "var x = BigInt('340282366920938463463374607431768211456'); \
             var y = BigInt('18446744073709551617'); \
             [x + y, y * y, x / y, x % y].join(':');"
        ),
        "340282366920938463481821351505477763073:340282366920938463500268095579187314689:18446744073709551615:1"
    );
}

#[test]
fn bigint_literals_preserve_values_beyond_i128() {
    assert_eq!(
        native_eval(
            "[340282366920938463463374607431768211456n, \
              0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff_ffffn, \
              0b1_00000000000000000000000000000000000000000000000000000000000000000n, \
              0o4000000000000000000000n].join(':');"
        ),
        "340282366920938463463374607431768211456:22300745198530623141535718272648361505980415:36893488147419103232:36893488147419103232"
    );
}

#[test]
fn bigint_bitwise_shift_and_power_use_shared_operations() {
    assert_eq!(
        native_eval(
            "var a = BigInt('-8'), b = BigInt('3'); \
             [a | b, a & b, a ^ b, ~b, a >> BigInt('1'), b << BigInt('130'), BigInt('3') ** BigInt('80')].join(':');"
        ),
        "-5:0:-5:-4:-4:4083388403051261561560495289181218537472:147808829414345923316083210206383297601"
    );
}

#[test]
fn bigint_number_comparisons_remain_exact_beyond_safe_integer() {
    assert_eq!(
        native_eval(
            "var x = BigInt('9007199254740993'); \
             [x == 9007199254740992, x > 9007199254740992, x < 9007199254740994].join(':');"
        ),
        "false:true:true"
    );
}

#[test]
fn bigint_constructor_and_width_operations_follow_string_to_bigint() {
    assert_eq!(
        native_eval(
            "[BigInt('  '), BigInt(true), BigInt(9007199254740992), \
              BigInt.asUintN(8, BigInt('-1')), BigInt.asIntN(8, BigInt('255'))].join(':');"
        ),
        "0:1:9007199254740992:255:-1"
    );
    assert_eq!(
        native_eval(
            "var signedPrefix = false, fractional = false, constructed = false; \
             try { BigInt('-0x1'); } catch (e) { signedPrefix = e.constructor === SyntaxError; } \
             try { BigInt(1.5); } catch (e) { fractional = e.constructor === RangeError; } \
             try { new BigInt(1); } catch (e) { constructed = e.constructor === TypeError; } \
             [signedPrefix, fractional, constructed].join(':');"
        ),
        "true:true:true"
    );
}

#[test]
fn bigint_invalid_mixed_and_range_operations_are_catchable() {
    assert_eq!(
        native_eval(
            "var mixed = false, divide = false, exponent = false, unsigned = false; \
             try { BigInt('1') + 1; } catch (e) { mixed = e.constructor === TypeError; } \
             try { BigInt('1') / BigInt('0'); } catch (e) { divide = e.constructor === RangeError; } \
             try { BigInt('2') ** BigInt('-1'); } catch (e) { exponent = e.constructor === RangeError; } \
             try { BigInt('1') >>> BigInt('0'); } catch (e) { unsigned = e.constructor === TypeError; } \
             [mixed, divide, exponent, unsigned].join(':');"
        ),
        "true:true:true:true"
    );
}

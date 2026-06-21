#[path = "../src/builtins/string.rs"]
mod string;

use string::{
    MAX_STRING_CODE_UNITS, PROTOTYPE_METHODS, STATIC_METHODS, StringBuiltinError, at, char_at,
    char_code_at, concat, decode_utf16, ends_with, from_char_codes, from_code_points, includes,
    index_of, last_index_of, pad_end, pad_start, repeat, slice, starts_with, substr, substring,
    to_lower_case, to_upper_case, trim, trim_end, trim_start, utf16_code_unit_at, utf16_length,
    utf16_slice, utf16_units,
};

#[test]
fn method_metadata_matches_the_v6_contract() {
    assert_eq!(
        STATIC_METHODS
            .iter()
            .map(|method| (method.name, method.length))
            .collect::<Vec<_>>(),
        [("fromCharCode", 1), ("fromCodePoint", 1)]
    );
    assert!(PROTOTYPE_METHODS.contains(&string::StringMethodSpec {
        name: "charAt",
        length: 1,
    }));
    assert!(PROTOTYPE_METHODS.contains(&string::StringMethodSpec {
        name: "trim",
        length: 0,
    }));
    assert!(PROTOTYPE_METHODS.contains(&string::StringMethodSpec {
        name: "padEnd",
        length: 1,
    }));
}

#[test]
fn utf16_helpers_count_code_units_instead_of_utf8_bytes_or_scalars() {
    let value = "A😀B";
    assert_eq!(utf16_units(value), [0x41, 0xD83D, 0xDE00, 0x42]);
    assert_eq!(utf16_length(value), 4);
    assert_eq!(utf16_code_unit_at(value, 1), Some(0xD83D));
    assert_eq!(utf16_code_unit_at(value, 2), Some(0xDE00));
    assert_eq!(utf16_slice(value, 1, 3), "😀");
}

#[test]
fn character_access_uses_ecmascript_relative_and_code_unit_indexes() {
    assert_eq!(char_at("abc", 1), "b");
    assert_eq!(char_at("abc", -1), "");
    assert_eq!(char_code_at("A😀", 1), Some(0xD83D));
    assert_eq!(char_code_at("abc", 5), None);
    assert_eq!(at("abc", -1), Some("c".into()));
    assert_eq!(at("abc", 3), None);
}

#[test]
fn search_helpers_report_utf16_offsets() {
    let value = "😀a😀";
    assert_eq!(index_of(value, "a", 0), Some(2));
    assert_eq!(index_of(value, "😀", 1), Some(3));
    assert_eq!(last_index_of(value, "😀", None), Some(3));
    assert_eq!(last_index_of(value, "😀", Some(2)), Some(0));
    assert!(includes(value, "a", 1));
    assert!(starts_with(value, "a", 2));
    assert!(ends_with(value, "a", Some(3)));
}

#[test]
fn slice_substring_and_substr_follow_their_distinct_index_rules() {
    assert_eq!(slice("abcdef", -3, None), "def");
    assert_eq!(slice("abcdef", 4, Some(2)), "");
    assert_eq!(substring("abcdef", 4, Some(2)), "cd");
    assert_eq!(substring("abcdef", -2, Some(2)), "ab");
    assert_eq!(substr("abcdef", -3, Some(2)), "de");
    assert_eq!(substr("abcdef", 2, Some(-1)), "");
}

#[test]
fn concat_repeat_and_padding_obey_length_limits() {
    assert_eq!(concat("a", &["b", "c"]), "abc");
    assert_eq!(repeat("ab", 3).unwrap(), "ababab");
    assert_eq!(repeat("x", -1), Err(StringBuiltinError::InvalidRepeatCount));
    assert_eq!(
        repeat("xx", i64::try_from(MAX_STRING_CODE_UNITS).unwrap()),
        Err(StringBuiltinError::AllocationLimit)
    );
    assert_eq!(pad_start("x", 5, "ab").unwrap(), "ababx");
    assert_eq!(pad_end("x", 5, "ab").unwrap(), "xabab");
    assert_eq!(pad_end("abc", 2, "x").unwrap(), "abc");
    assert_eq!(pad_start("x", 3, "").unwrap(), "x");
}

#[test]
fn trimming_includes_bom_and_case_conversion_handles_unicode() {
    assert_eq!(trim("\u{FEFF}  agent \n"), "agent");
    assert_eq!(trim_start("\t agent "), "agent ");
    assert_eq!(trim_end(" agent \r\n"), " agent");
    assert_eq!(to_lower_case("Agent Σ"), "agent σ");
    assert_eq!(to_upper_case("Agent ß"), "AGENT SS");
}

#[test]
fn static_constructors_preserve_exact_utf16_sequences() {
    assert_eq!(
        from_char_codes(&[0x41, 0xD83D, 0xDE00]),
        [0x41, 0xD83D, 0xDE00]
    );
    assert_eq!(
        from_code_points(&[0x41, 0x1F600]).unwrap(),
        [0x41, 0xD83D, 0xDE00]
    );
    assert_eq!(
        from_code_points(&[0x11_0000]),
        Err(StringBuiltinError::InvalidCodePoint(0x11_0000))
    );
    assert_eq!(decode_utf16(&[0x41, 0xD83D, 0xDE00]), "A😀");
}

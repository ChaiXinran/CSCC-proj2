# Supported Correctness Gate

- Label: `agentjs-supported-delivery`
- Command: `C:\Users\36123\Desktop\OS 功能赛\CSCC-proj2\target\release\agentjs.exe eval`
- Result: **24/24 passed**

| Case | Category | Status | Expected | Actual | Time ms |
|:---|:---|:---|:---|:---|---:|
| arithmetic_precedence | expression | pass | `7` | `7` | 13.5 |
| bitwise_unsigned_shift | expression | pass | `true` | `true` | 51.9 |
| parenthesized_unary_exponent | expression | pass | `-8` | `-8` | 11.2 |
| object_is_same_value | object | pass | `true:false` | `true:false` | 11.3 |
| descriptor_delete_and_in | object | pass | `false:true` | `false:true` | 11.4 |
| delete_primitive_and_nullish | object | pass | `true` | `true` | 10.6 |
| prototype_lookup | object | pass | `7` | `7` | 55.0 |
| define_property_descriptor | object | pass | `1:false:false:false` | `1:false:false:false` | 11.3 |
| array_holes_and_length | array | pass | `3:false:3` | `3:false:3` | 10.8 |
| sparse_array_high_index | array | pass | `70001:5` | `70001:5` | 11.0 |
| for_in_member_target | iteration | pass | `x,y` | `x,y` | 54.8 |
| for_let_closure_capture | scope | pass | `0:1:2` | `0:1:2` | 11.1 |
| eval_closure_for_let_capture | scope | pass | `0:1:2` | `0:1:2` | 11.5 |
| with_eval_lookup | scope | pass | `o1:o1` | `o1:o1` | 53.9 |
| arrow_lexical_arguments | function | pass | `2` | `2` | 9.9 |
| constructor_returns_function_object | function | pass | `true` | `true` | 52.5 |
| function_prototype_call_saved | function | pass | `9` | `9` | 10.2 |
| class_instance_static_and_getter_name | class | pass | `-1:1:10:get y` | `-1:1:10:get y` | 54.2 |
| class_extends_static_super_computed | class | pass | `true:-1:-1:1` | `true:-1:-1:1` | 10.9 |
| delete_super_reference_error | class | pass | `true` | `true` | 51.8 |
| try_catch_finally | control | pass | `tcf` | `tcf` | 52.6 |
| json_roundtrip | stdlib | pass | `{"x":1,"y":[2,3]}` | `{"x":1,"y":[2,3]}` | 48.3 |
| map_same_value_zero | stdlib | pass | `1:2` | `1:2` | 54.0 |
| string_raw_tagged_template | stdlib | pass | `abcXd` | `abcXd` | 44.2 |

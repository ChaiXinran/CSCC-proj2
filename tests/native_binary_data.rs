use agentjs::runtime::{JsValue, NativeContext, TypedArrayElementKind};

#[test]
fn array_buffer_records_byte_length_and_detach_state() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(8).expect("create buffer");

    assert_eq!(context.array_buffer_byte_length(buffer).unwrap(), 8);
    assert!(!context.is_array_buffer_detached(buffer).unwrap());

    context.detach_array_buffer(buffer).expect("detach buffer");

    assert_eq!(context.array_buffer_byte_length(buffer).unwrap(), 0);
    assert!(context.is_array_buffer_detached(buffer).unwrap());
}

#[test]
fn typed_array_view_stores_and_loads_elements() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(8).expect("create buffer");
    let view = context
        .create_typed_array_view(buffer, TypedArrayElementKind::Int16, 0, 4)
        .expect("create Int16 view");

    context
        .typed_array_store_element(view, 0, JsValue::Number(-2.0))
        .expect("store first element");
    context
        .typed_array_store_element(view, 3, JsValue::Number(513.0))
        .expect("store last element");

    assert_eq!(
        context.typed_array_load_element(view, 0).unwrap(),
        JsValue::Number(-2.0)
    );
    assert_eq!(
        context.typed_array_load_element(view, 3).unwrap(),
        JsValue::Number(513.0)
    );
    assert_eq!(context.typed_array_byte_length(view).unwrap(), 8);
}

#[test]
fn typed_array_rejects_unaligned_or_out_of_range_views() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(4).expect("create buffer");

    assert!(
        context
            .create_typed_array_view(buffer, TypedArrayElementKind::Int16, 1, 1)
            .is_err()
    );
    assert!(
        context
            .create_typed_array_view(buffer, TypedArrayElementKind::Uint32, 0, 2)
            .is_err()
    );
}

#[test]
fn data_view_uses_shared_storage_and_endianness() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(4).expect("create buffer");
    let typed = context
        .create_typed_array_view(buffer, TypedArrayElementKind::Uint16, 0, 2)
        .expect("create Uint16 view");
    let data = context
        .create_data_view(buffer, 0, 4)
        .expect("create DataView");

    context
        .data_view_set(
            data,
            0,
            TypedArrayElementKind::Uint16,
            JsValue::Number(0x1234 as f64),
            false,
        )
        .expect("big-endian set");

    assert_eq!(
        context
            .data_view_get(data, 0, TypedArrayElementKind::Uint16, false)
            .unwrap(),
        JsValue::Number(0x1234 as f64)
    );
    assert_eq!(
        context.typed_array_load_element(typed, 0).unwrap(),
        JsValue::Number(0x3412 as f64)
    );
}

#[test]
fn detached_buffer_rejects_view_access() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(8).expect("create buffer");
    let view = context
        .create_typed_array_view(buffer, TypedArrayElementKind::Float64, 0, 1)
        .expect("create Float64 view");

    context.detach_array_buffer(buffer).expect("detach buffer");

    assert!(context.typed_array_load_element(view, 0).is_err());
    assert!(
        context
            .typed_array_store_element(view, 0, JsValue::Number(1.0))
            .is_err()
    );
    assert!(
        context
            .create_data_view(buffer, 0, 0)
            .expect_err("detached buffer should reject DataView")
            .message
            .contains("detached")
    );
}

#[test]
fn uint8_clamped_rounds_half_to_even() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(4).expect("create buffer");
    let view = context
        .create_typed_array_view(buffer, TypedArrayElementKind::Uint8Clamped, 0, 4)
        .expect("create Uint8Clamped view");

    for (index, value) in [1.5, 2.5, -1.0, 300.0].into_iter().enumerate() {
        context
            .typed_array_store_element(view, index, JsValue::Number(value))
            .expect("store clamped value");
    }

    assert_eq!(
        context.typed_array_load_element(view, 0).unwrap(),
        JsValue::Number(2.0)
    );
    assert_eq!(
        context.typed_array_load_element(view, 1).unwrap(),
        JsValue::Number(2.0)
    );
    assert_eq!(
        context.typed_array_load_element(view, 2).unwrap(),
        JsValue::Number(0.0)
    );
    assert_eq!(
        context.typed_array_load_element(view, 3).unwrap(),
        JsValue::Number(255.0)
    );
}

#[test]
fn bigint_typed_array_elements_use_bigint_values() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(16).expect("create buffer");
    let signed = context
        .create_typed_array_view(buffer, TypedArrayElementKind::BigInt64, 0, 2)
        .expect("create BigInt64 view");
    let unsigned = context
        .create_typed_array_view(buffer, TypedArrayElementKind::BigUint64, 0, 2)
        .expect("create BigUint64 view");

    context
        .typed_array_store_element(signed, 0, JsValue::BigInt(-1))
        .expect("store signed value");
    context
        .typed_array_store_element(unsigned, 1, JsValue::BigInt(0x0102_0304_0506_0708))
        .expect("store unsigned value");

    assert_eq!(
        context.typed_array_load_element(signed, 0).unwrap(),
        JsValue::BigInt(-1)
    );
    assert_eq!(
        context.typed_array_load_element(unsigned, 0).unwrap(),
        JsValue::BigInt(u64::MAX as i128)
    );
    assert_eq!(
        context.typed_array_load_element(signed, 1).unwrap(),
        JsValue::BigInt(0x0102_0304_0506_0708)
    );
}

#[test]
fn data_view_bigint_and_float16_share_buffer_storage() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(10).expect("create buffer");
    let data = context
        .create_data_view(buffer, 0, 10)
        .expect("create view");

    context
        .data_view_set(
            data,
            0,
            TypedArrayElementKind::BigInt64,
            JsValue::BigInt(-2),
            false,
        )
        .expect("store BigInt64");
    context
        .data_view_set(
            data,
            8,
            TypedArrayElementKind::Float16,
            JsValue::Number(1.5),
            false,
        )
        .expect("store Float16");

    assert_eq!(
        context
            .data_view_get(data, 0, TypedArrayElementKind::BigInt64, false)
            .unwrap(),
        JsValue::BigInt(-2)
    );
    assert_eq!(
        context
            .data_view_get(data, 8, TypedArrayElementKind::Float16, false)
            .unwrap(),
        JsValue::Number(1.5)
    );
}

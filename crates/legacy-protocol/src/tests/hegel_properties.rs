#![allow(clippy::expect_used, clippy::unwrap_used)]

use ::hegel::TestCase;
use ::hegel::generators as gs;

use super::*;

fn assert_protocol_roundtrip<T>(
    value: &T,
    encode: impl Fn(&T) -> Vec<u8>,
    decode: impl Fn(&[u8]) -> Result<T, LegacyProtocolError>,
) where
    T: std::fmt::Debug + PartialEq,
{
    let encoded = encode(value);
    let decoded = decode(&encoded).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(encode(&decoded), encoded);

    let mut with_trailing_byte = encoded;
    with_trailing_byte.push(0);
    assert!(matches!(
        decode(&with_trailing_byte),
        Err(LegacyProtocolError::ShortBuffer)
    ));
}

#[::hegel::composite]
fn protocol_attribute_check(tc: TestCase) -> ProtocolAttributeCheck {
    ProtocolAttributeCheck {
        attr: tc.draw(gs::integers::<u16>()),
        value: tc.draw(gs::binary().max_size(16)),
        datatype: tc.draw(gs::integers::<u16>()),
        predicate: tc.draw(gs::integers::<u16>()),
    }
}

#[::hegel::composite]
fn protocol_funcall(tc: TestCase) -> ProtocolFuncall {
    ProtocolFuncall {
        attr: tc.draw(gs::integers::<u16>()),
        name: tc.draw(gs::integers::<u8>()),
        arg1: tc.draw(gs::binary().max_size(16)),
        arg1_datatype: tc.draw(gs::integers::<u16>()),
        arg2: tc.draw(gs::binary().max_size(16)),
        arg2_datatype: tc.draw(gs::integers::<u16>()),
    }
}

#[::hegel::composite]
fn protocol_key_change(tc: TestCase) -> ProtocolKeyChange {
    ProtocolKeyChange {
        key: tc.draw(gs::binary().max_size(16)),
        erase: tc.draw(gs::booleans()),
        fail_if_not_found: tc.draw(gs::booleans()),
        fail_if_found: tc.draw(gs::booleans()),
        checks: tc.draw(gs::vecs(protocol_attribute_check()).max_size(5)),
        funcalls: tc.draw(gs::vecs(protocol_funcall()).max_size(5)),
    }
}

#[::hegel::composite]
fn protocol_search_start_request(tc: TestCase) -> ProtocolSearchStart {
    ProtocolSearchStart {
        search_id: tc.draw(gs::integers::<u64>()),
        checks: tc.draw(gs::vecs(protocol_attribute_check()).max_size(5)),
    }
}

#[::hegel::composite]
fn protocol_value_list(tc: TestCase) -> Vec<Vec<u8>> {
    tc.draw(gs::vecs(gs::binary().max_size(16)).max_size(5))
}

#[::hegel::composite]
fn protocol_get_response(tc: TestCase) -> ProtocolGetResponse {
    let status = tc.draw(gs::integers::<u16>());
    ProtocolGetResponse {
        status,
        values: if status == LegacyReturnCode::Success as u16 {
            tc.draw(protocol_value_list())
        } else {
            Vec::new()
        },
    }
}

#[::hegel::composite]
fn protocol_search_item(tc: TestCase) -> ProtocolSearchItem {
    ProtocolSearchItem {
        key: tc.draw(gs::binary().max_size(16)),
        values: tc.draw(protocol_value_list()),
    }
}

#[::hegel::test(test_cases = 80)]
fn hegel_protocol_wire_codecs_are_exact_and_canonical(tc: TestCase) {
    let case: u8 = tc.draw(gs::integers::<u8>().max_value(8));

    match case {
        0 => {
            let key: Vec<u8> = tc.draw(gs::binary().max_size(16));
            assert_protocol_roundtrip(
                &key,
                |value| encode_protocol_get_request(value),
                decode_protocol_get_request,
            );
        }
        1 => {
            let checks: Vec<ProtocolAttributeCheck> =
                tc.draw(gs::vecs(protocol_attribute_check()).max_size(5));
            assert_protocol_roundtrip(
                &checks,
                |value| encode_protocol_count_request(value),
                decode_protocol_count_request,
            );
        }
        2 => {
            let change = tc.draw(protocol_key_change());
            assert_protocol_roundtrip(
                &change,
                encode_protocol_atomic_request,
                decode_protocol_atomic_request,
            );
        }
        3 => {
            let request = tc.draw(protocol_search_start_request());
            assert_protocol_roundtrip(
                &request,
                encode_protocol_search_start,
                decode_protocol_search_start,
            );
        }
        4 => {
            let search_id: u64 = tc.draw(gs::integers::<u64>());
            assert_protocol_roundtrip(
                &search_id,
                |value| encode_protocol_search_continue(*value).to_vec(),
                decode_protocol_search_continue,
            );
        }
        5 => {
            let status: u16 = tc.draw(gs::integers::<u16>());
            assert_protocol_roundtrip(
                &status,
                |value| encode_protocol_atomic_response(*value).to_vec(),
                decode_protocol_atomic_response,
            );
        }
        6 => {
            let count: u64 = tc.draw(gs::integers::<u64>());
            assert_protocol_roundtrip(
                &count,
                |value| encode_protocol_count_response(*value).to_vec(),
                decode_protocol_count_response,
            );
        }
        7 => {
            let response = tc.draw(protocol_get_response());
            assert_protocol_roundtrip(
                &response,
                encode_protocol_get_response,
                decode_protocol_get_response,
            );
        }
        8 => {
            let item = tc.draw(protocol_search_item());
            assert_protocol_roundtrip(
                &item,
                encode_protocol_search_item,
                decode_protocol_search_item,
            );
        }
        _ => unreachable!("case selector is bounded to 0..=8"),
    }
}

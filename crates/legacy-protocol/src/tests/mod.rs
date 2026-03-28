#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;

#[test]
fn request_header_round_trips() {
    let header = RequestHeader {
        message_type: LegacyMessageType::ReqAtomic,
        flags: 0x3,
        version: 7,
        target_virtual_server: 11,
        nonce: 19,
    };

    let encoded = header.encode();

    assert_eq!(encoded.len(), LEGACY_REQUEST_HEADER_SIZE);
    assert_eq!(RequestHeader::decode(&encoded).unwrap(), header);
}

#[test]
fn response_header_round_trips() {
    let header = ResponseHeader {
        message_type: LegacyMessageType::RespGet,
        target_virtual_server: 23,
        nonce: 29,
    };

    let encoded = header.encode();

    assert_eq!(encoded.len(), LEGACY_RESPONSE_HEADER_SIZE);
    assert_eq!(ResponseHeader::decode(&encoded).unwrap(), header);
}

#[test]
fn legacy_message_type_numbers_match_hyperdex() {
    assert_eq!(LegacyMessageType::ReqGet as u8, 8);
    assert_eq!(LegacyMessageType::ReqAtomic as u8, 16);
    assert_eq!(LegacyMessageType::ReqSearchDescribe as u8, 52);
    assert_eq!(LegacyMessageType::RespGroupAtomic as u8, 55);
}

#[test]
fn legacy_return_codes_match_hyperdex() {
    assert_eq!(LegacyReturnCode::Success as u16, 8320);
    assert_eq!(LegacyReturnCode::NotFound as u16, 8321);
    assert_eq!(LegacyReturnCode::CompareFailed as u16, 8325);
    assert_eq!(LegacyReturnCode::Unauthorized as u16, 8329);
}

#[test]
fn config_mismatch_response_preserves_routing_fields() {
    let request = RequestHeader {
        message_type: LegacyMessageType::ReqGet,
        flags: 0,
        version: 1,
        target_virtual_server: 23,
        nonce: 29,
    };

    assert_eq!(
        config_mismatch_response(request),
        ResponseHeader {
            message_type: LegacyMessageType::ConfigMismatch,
            target_virtual_server: 23,
            nonce: 29,
        }
    );
}

#[test]
fn identify_frame_uses_busybee_identify_flag() {
    let frame = encode_identify_frame(7, 11);
    assert_eq!(frame.len(), 20);
    assert_eq!(
        u32::from_be_bytes(frame[..4].try_into().unwrap()),
        BUSYBEE_HEADER_IDENTIFY | 20
    );
    assert_eq!(u64::from_be_bytes(frame[4..12].try_into().unwrap()), 7);
    assert_eq!(u64::from_be_bytes(frame[12..20].try_into().unwrap()), 11);
}

#[test]
fn count_request_round_trips() {
    let request = CountRequest {
        space: "profiles".to_owned(),
    };

    assert_eq!(
        CountRequest::decode_body(&request.encode_body()).unwrap(),
        request
    );
}

#[test]
fn count_response_round_trips() {
    let response = CountResponse { count: 42 };

    assert_eq!(
        CountResponse::decode_body(&response.encode_body()).unwrap(),
        response
    );
}

#[test]
fn get_request_round_trips() {
    let request = GetRequest {
        key: b"ada".to_vec(),
    };

    assert_eq!(
        GetRequest::decode_body(&request.encode_body()).unwrap(),
        request
    );
}

#[test]
fn get_response_round_trips() {
    let response = GetResponse {
        status: LegacyReturnCode::Success,
        attributes: vec![
            GetAttribute {
                name: "first".to_owned(),
                value: GetValue::String("Ada".to_owned()),
            },
            GetAttribute {
                name: "views".to_owned(),
                value: GetValue::Int(5),
            },
        ],
    };

    assert_eq!(
        GetResponse::decode_body(&response.encode_body()).unwrap(),
        response
    );
}

#[test]
fn search_start_request_round_trips() {
    let request = SearchStartRequest {
        space: "profiles".to_owned(),
        search_id: 41,
        checks: vec![LegacyCheck {
            attribute: "profile_views".to_owned(),
            predicate: LegacyPredicate::GreaterThanOrEqual,
            value: LegacyValue::Int(2),
        }],
    };

    assert_eq!(
        SearchStartRequest::decode_body(&request.encode_body()).unwrap(),
        request
    );
}

#[test]
fn search_continue_request_round_trips() {
    let request = SearchContinueRequest { search_id: 41 };

    assert_eq!(
        SearchContinueRequest::decode_body(&request.encode_body()).unwrap(),
        request
    );
}

#[test]
fn search_item_response_round_trips() {
    let response = SearchItemResponse {
        search_id: 41,
        key: b"ada".to_vec(),
        attributes: vec![GetAttribute {
            name: "first".to_owned(),
            value: GetValue::String("Ada".to_owned()),
        }],
    };

    assert_eq!(
        SearchItemResponse::decode_body(&response.encode_body()).unwrap(),
        response
    );
}

#[test]
fn search_done_response_round_trips() {
    let response = SearchDoneResponse { search_id: 41 };

    assert_eq!(
        SearchDoneResponse::decode_body(&response.encode_body()).unwrap(),
        response
    );
}

#[test]
fn atomic_request_round_trips() {
    let request = AtomicRequest {
        flags: LEGACY_ATOMIC_FLAG_WRITE | LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND,
        key: b"ada".to_vec(),
        checks: vec![LegacyCheck {
            attribute: "profile_views".to_owned(),
            predicate: LegacyPredicate::GreaterThanOrEqual,
            value: LegacyValue::Int(2),
        }],
        funcalls: vec![
            LegacyFuncall {
                attribute: "first".to_owned(),
                name: LegacyFuncallName::Set,
                arg1: LegacyValue::String("Ada".to_owned()),
                arg2: None,
            },
            LegacyFuncall {
                attribute: "profile_views".to_owned(),
                name: LegacyFuncallName::NumAdd,
                arg1: LegacyValue::Int(3),
                arg2: None,
            },
            LegacyFuncall {
                attribute: "nickname".to_owned(),
                name: LegacyFuncallName::MapAdd,
                arg1: LegacyValue::String("short".to_owned()),
                arg2: Some(LegacyValue::String("Ada".to_owned())),
            },
            LegacyFuncall {
                attribute: "prefix".to_owned(),
                name: LegacyFuncallName::StringPrepend,
                arg1: LegacyValue::String("Dr. ".to_owned()),
                arg2: None,
            },
        ],
    };

    assert_eq!(
        AtomicRequest::decode_body(&request.encode_body()).unwrap(),
        request
    );
}

#[test]
fn atomic_request_decode_rejects_oom_fuzz_input() {
    let bytes = [206, 182, 182, 206, 106, 207];
    assert!(matches!(
        decode_protocol_atomic_request(&bytes),
        Err(LegacyProtocolError::ShortBuffer)
    ));
}

#[test]
fn atomic_response_round_trips() {
    let response = AtomicResponse {
        status: LegacyReturnCode::Success,
    };

    assert_eq!(
        AtomicResponse::decode_body(&response.encode_body()).unwrap(),
        response
    );
}

#[test]
fn request_frame_prefix_matches_payload_length() {
    let frame = encode_request_frame(
        RequestHeader {
            message_type: LegacyMessageType::ReqCount,
            flags: 0,
            version: 1,
            target_virtual_server: 2,
            nonce: 3,
        },
        &CountRequest {
            space: "profiles".to_owned(),
        }
        .encode_body(),
    );

    assert_eq!(
        u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize,
        frame.len()
    );
}

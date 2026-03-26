use thiserror::Error;

pub const BUSYBEE_HEADER_SIZE: usize = 4;
pub const LEGACY_REQUEST_HEADER_SIZE: usize = BUSYBEE_HEADER_SIZE + 1 + 1 + 8 + 8 + 8;
pub const LEGACY_RESPONSE_HEADER_SIZE: usize = BUSYBEE_HEADER_SIZE + 1 + 8 + 8;
pub const GET_REQUEST_PREFIX_SIZE: usize = 2;
pub const COUNT_REQUEST_PREFIX_SIZE: usize = 2;
pub const COUNT_RESPONSE_BODY_SIZE: usize = 8;
pub const ATOMIC_REQUEST_PREFIX_SIZE: usize = 2 + 1 + 2;
pub const ATOMIC_RESPONSE_BODY_SIZE: usize = 2;
pub const LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND: u8 = 0x01;
pub const LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND: u8 = 0x02;
pub const LEGACY_ATOMIC_FLAG_HAS_ATTRIBUTES: u8 = 0x80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LegacyMessageType {
    ReqGet = 8,
    RespGet = 9,
    ReqGetPartial = 10,
    RespGetPartial = 11,
    ReqAtomic = 16,
    RespAtomic = 17,
    ReqSearchStart = 32,
    ReqSearchNext = 33,
    ReqSearchStop = 34,
    RespSearchItem = 35,
    RespSearchDone = 36,
    ReqSortedSearch = 40,
    RespSortedSearch = 41,
    ReqCount = 50,
    RespCount = 51,
    ReqSearchDescribe = 52,
    RespSearchDescribe = 53,
    ReqGroupAtomic = 54,
    RespGroupAtomic = 55,
    ConfigMismatch = 254,
    PacketNop = 255,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum LegacyReturnCode {
    Success = 8320,
    NotFound = 8321,
    BadDimensionSpec = 8322,
    NotUs = 8323,
    ServerError = 8324,
    CompareFailed = 8325,
    ReadOnly = 8327,
    Overflow = 8328,
    Unauthorized = 8329,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestHeader {
    pub message_type: LegacyMessageType,
    pub flags: u8,
    pub version: u64,
    pub target_virtual_server: u64,
    pub nonce: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseHeader {
    pub message_type: LegacyMessageType,
    pub target_virtual_server: u64,
    pub nonce: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountRequest {
    pub space: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountResponse {
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetRequest {
    pub key: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetAttribute {
    pub name: String,
    pub value: GetValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GetValue {
    Null,
    Bool(bool),
    Int(i64),
    Bytes(Vec<u8>),
    String(String),
}

pub type LegacyAttribute = GetAttribute;
pub type LegacyValue = GetValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetResponse {
    pub status: LegacyReturnCode,
    pub attributes: Vec<GetAttribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicRequest {
    pub flags: u8,
    pub key: Vec<u8>,
    pub attributes: Vec<GetAttribute>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtomicResponse {
    pub status: LegacyReturnCode,
}

#[derive(Debug, Error)]
pub enum LegacyProtocolError {
    #[error("unknown message type {0}")]
    UnknownMessageType(u8),
    #[error("unknown return code {0}")]
    UnknownReturnCode(u16),
    #[error("unknown value kind {0}")]
    UnknownValueKind(u8),
    #[error("buffer too short for header")]
    ShortBuffer,
    #[error("invalid utf-8 in request body")]
    InvalidUtf8,
}

impl RequestHeader {
    pub fn encode(self) -> [u8; LEGACY_REQUEST_HEADER_SIZE] {
        let mut bytes = [0u8; LEGACY_REQUEST_HEADER_SIZE];
        bytes[BUSYBEE_HEADER_SIZE] = self.message_type as u8;
        bytes[BUSYBEE_HEADER_SIZE + 1] = self.flags;
        bytes[BUSYBEE_HEADER_SIZE + 2..BUSYBEE_HEADER_SIZE + 10]
            .copy_from_slice(&self.version.to_be_bytes());
        bytes[BUSYBEE_HEADER_SIZE + 10..BUSYBEE_HEADER_SIZE + 18]
            .copy_from_slice(&self.target_virtual_server.to_be_bytes());
        bytes[BUSYBEE_HEADER_SIZE + 18..BUSYBEE_HEADER_SIZE + 26]
            .copy_from_slice(&self.nonce.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < LEGACY_REQUEST_HEADER_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            message_type: decode_message_type(bytes[BUSYBEE_HEADER_SIZE])?,
            flags: bytes[BUSYBEE_HEADER_SIZE + 1],
            version: u64::from_be_bytes(
                bytes[BUSYBEE_HEADER_SIZE + 2..BUSYBEE_HEADER_SIZE + 10]
                    .try_into()
                    .expect("fixed-width slice"),
            ),
            target_virtual_server: u64::from_be_bytes(
                bytes[BUSYBEE_HEADER_SIZE + 10..BUSYBEE_HEADER_SIZE + 18]
                    .try_into()
                    .expect("fixed-width slice"),
            ),
            nonce: u64::from_be_bytes(
                bytes[BUSYBEE_HEADER_SIZE + 18..BUSYBEE_HEADER_SIZE + 26]
                    .try_into()
                    .expect("fixed-width slice"),
            ),
        })
    }
}

impl ResponseHeader {
    pub fn encode(self) -> [u8; LEGACY_RESPONSE_HEADER_SIZE] {
        let mut bytes = [0u8; LEGACY_RESPONSE_HEADER_SIZE];
        bytes[BUSYBEE_HEADER_SIZE] = self.message_type as u8;
        bytes[BUSYBEE_HEADER_SIZE + 1..BUSYBEE_HEADER_SIZE + 9]
            .copy_from_slice(&self.target_virtual_server.to_be_bytes());
        bytes[BUSYBEE_HEADER_SIZE + 9..BUSYBEE_HEADER_SIZE + 17]
            .copy_from_slice(&self.nonce.to_be_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < LEGACY_RESPONSE_HEADER_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            message_type: decode_message_type(bytes[BUSYBEE_HEADER_SIZE])?,
            target_virtual_server: u64::from_be_bytes(
                bytes[BUSYBEE_HEADER_SIZE + 1..BUSYBEE_HEADER_SIZE + 9]
                    .try_into()
                    .expect("fixed-width slice"),
            ),
            nonce: u64::from_be_bytes(
                bytes[BUSYBEE_HEADER_SIZE + 9..BUSYBEE_HEADER_SIZE + 17]
                    .try_into()
                    .expect("fixed-width slice"),
            ),
        })
    }
}

impl CountRequest {
    pub fn encode_body(&self) -> Vec<u8> {
        let space = self.space.as_bytes();
        let mut body = Vec::with_capacity(COUNT_REQUEST_PREFIX_SIZE + space.len());
        body.extend_from_slice(&(space.len() as u16).to_be_bytes());
        body.extend_from_slice(space);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < COUNT_REQUEST_PREFIX_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let len = u16::from_be_bytes(bytes[..2].try_into().expect("fixed-width slice")) as usize;
        if bytes.len() < COUNT_REQUEST_PREFIX_SIZE + len {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let space = std::str::from_utf8(&bytes[2..2 + len])
            .map_err(|_| LegacyProtocolError::InvalidUtf8)?
            .to_owned();
        Ok(Self { space })
    }
}

impl CountResponse {
    pub fn encode_body(self) -> [u8; COUNT_RESPONSE_BODY_SIZE] {
        self.count.to_be_bytes()
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < COUNT_RESPONSE_BODY_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            count: u64::from_be_bytes(bytes[..8].try_into().expect("fixed-width slice")),
        })
    }
}

impl GetRequest {
    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(GET_REQUEST_PREFIX_SIZE + self.key.len());
        body.extend_from_slice(&(self.key.len() as u16).to_be_bytes());
        body.extend_from_slice(&self.key);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < GET_REQUEST_PREFIX_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let len = u16::from_be_bytes(bytes[..2].try_into().expect("fixed-width slice")) as usize;
        if bytes.len() < GET_REQUEST_PREFIX_SIZE + len {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            key: bytes[2..2 + len].to_vec(),
        })
    }
}

impl GetResponse {
    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(self.status as u16).to_be_bytes());
        body.extend_from_slice(&(self.attributes.len() as u16).to_be_bytes());
        encode_attributes_into(&mut body, &self.attributes);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < 4 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let status = decode_return_code(u16::from_be_bytes(
            bytes[..2].try_into().expect("fixed-width slice"),
        ))?;
        let attr_count =
            u16::from_be_bytes(bytes[2..4].try_into().expect("fixed-width slice")) as usize;
        let (attributes, offset) = decode_attributes(bytes, 4, attr_count)?;

        if offset != bytes.len() {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self { status, attributes })
    }
}

impl AtomicRequest {
    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(self.key.len() as u16).to_be_bytes());
        body.extend_from_slice(&self.key);
        body.push(self.flags);
        body.extend_from_slice(&(self.attributes.len() as u16).to_be_bytes());
        encode_attributes_into(&mut body, &self.attributes);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < ATOMIC_REQUEST_PREFIX_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let key_len = u16::from_be_bytes(bytes[..2].try_into().expect("fixed-width slice")) as usize;
        if bytes.len() < 2 + key_len + 3 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let key = bytes[2..2 + key_len].to_vec();
        let flags = bytes[2 + key_len];
        let attr_count = u16::from_be_bytes(
            bytes[3 + key_len..5 + key_len]
                .try_into()
                .expect("fixed-width slice"),
        ) as usize;
        let (attributes, offset) = decode_attributes(bytes, 5 + key_len, attr_count)?;

        if offset != bytes.len() {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            flags,
            key,
            attributes,
        })
    }
}

impl AtomicResponse {
    pub fn encode_body(self) -> [u8; ATOMIC_RESPONSE_BODY_SIZE] {
        (self.status as u16).to_be_bytes()
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < ATOMIC_RESPONSE_BODY_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            status: decode_return_code(u16::from_be_bytes(
                bytes[..2].try_into().expect("fixed-width slice"),
            ))?,
        })
    }
}

fn encode_attributes_into(body: &mut Vec<u8>, attributes: &[GetAttribute]) {
    for attr in attributes {
        let name = attr.name.as_bytes();
        body.extend_from_slice(&(name.len() as u16).to_be_bytes());
        body.extend_from_slice(name);
        encode_value_into(body, &attr.value);
    }
}

fn encode_value_into(body: &mut Vec<u8>, value: &GetValue) {
    match value {
        GetValue::Null => {
            body.push(0);
            body.extend_from_slice(&0u32.to_be_bytes());
        }
        GetValue::Bool(v) => {
            body.push(1);
            body.extend_from_slice(&1u32.to_be_bytes());
            body.push(u8::from(*v));
        }
        GetValue::Int(v) => {
            body.push(2);
            body.extend_from_slice(&8u32.to_be_bytes());
            body.extend_from_slice(&v.to_be_bytes());
        }
        GetValue::Bytes(v) => {
            body.push(3);
            body.extend_from_slice(&(v.len() as u32).to_be_bytes());
            body.extend_from_slice(v);
        }
        GetValue::String(v) => {
            let bytes = v.as_bytes();
            body.push(4);
            body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            body.extend_from_slice(bytes);
        }
    }
}

fn decode_attributes(
    bytes: &[u8],
    mut offset: usize,
    attr_count: usize,
) -> Result<(Vec<GetAttribute>, usize), LegacyProtocolError> {
    let mut attributes = Vec::with_capacity(attr_count);

    for _ in 0..attr_count {
        if bytes.len() < offset + 2 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let name_len = u16::from_be_bytes(
            bytes[offset..offset + 2]
                .try_into()
                .expect("fixed-width slice"),
        ) as usize;
        offset += 2;

        if bytes.len() < offset + name_len + 5 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let name = std::str::from_utf8(&bytes[offset..offset + name_len])
            .map_err(|_| LegacyProtocolError::InvalidUtf8)?
            .to_owned();
        offset += name_len;

        let (value, next_offset) = decode_value(bytes, offset)?;
        offset = next_offset;
        attributes.push(GetAttribute { name, value });
    }

    Ok((attributes, offset))
}

fn decode_value(bytes: &[u8], offset: usize) -> Result<(GetValue, usize), LegacyProtocolError> {
    let kind = bytes[offset];
    let value_len = u32::from_be_bytes(
        bytes[offset + 1..offset + 5]
            .try_into()
            .expect("fixed-width slice"),
    ) as usize;
    let value_start = offset + 5;

    if bytes.len() < value_start + value_len {
        return Err(LegacyProtocolError::ShortBuffer);
    }

    let value_bytes = &bytes[value_start..value_start + value_len];
    let value = match kind {
        0 => GetValue::Null,
        1 => GetValue::Bool(value_bytes.first().copied().unwrap_or(0) != 0),
        2 => {
            if value_bytes.len() != 8 {
                return Err(LegacyProtocolError::ShortBuffer);
            }
            GetValue::Int(i64::from_be_bytes(
                value_bytes.try_into().expect("fixed-width slice"),
            ))
        }
        3 => GetValue::Bytes(value_bytes.to_vec()),
        4 => GetValue::String(
            std::str::from_utf8(value_bytes)
                .map_err(|_| LegacyProtocolError::InvalidUtf8)?
                .to_owned(),
        ),
        other => return Err(LegacyProtocolError::UnknownValueKind(other)),
    };

    Ok((value, value_start + value_len))
}

pub fn config_mismatch_response(request: RequestHeader) -> ResponseHeader {
    ResponseHeader {
        message_type: LegacyMessageType::ConfigMismatch,
        target_virtual_server: request.target_virtual_server,
        nonce: request.nonce,
    }
}

pub fn encode_request_frame(header: RequestHeader, body: &[u8]) -> Vec<u8> {
    encode_frame(header.encode().to_vec(), body)
}

pub fn encode_response_frame(header: ResponseHeader, body: &[u8]) -> Vec<u8> {
    encode_frame(header.encode().to_vec(), body)
}

fn encode_frame(mut head: Vec<u8>, body: &[u8]) -> Vec<u8> {
    let payload_len = (head.len() - BUSYBEE_HEADER_SIZE + body.len()) as u32;
    head[..BUSYBEE_HEADER_SIZE].copy_from_slice(&payload_len.to_be_bytes());
    head.extend_from_slice(body);
    head
}

fn decode_message_type(value: u8) -> Result<LegacyMessageType, LegacyProtocolError> {
    let message_type = match value {
        8 => LegacyMessageType::ReqGet,
        9 => LegacyMessageType::RespGet,
        10 => LegacyMessageType::ReqGetPartial,
        11 => LegacyMessageType::RespGetPartial,
        16 => LegacyMessageType::ReqAtomic,
        17 => LegacyMessageType::RespAtomic,
        32 => LegacyMessageType::ReqSearchStart,
        33 => LegacyMessageType::ReqSearchNext,
        34 => LegacyMessageType::ReqSearchStop,
        35 => LegacyMessageType::RespSearchItem,
        36 => LegacyMessageType::RespSearchDone,
        40 => LegacyMessageType::ReqSortedSearch,
        41 => LegacyMessageType::RespSortedSearch,
        50 => LegacyMessageType::ReqCount,
        51 => LegacyMessageType::RespCount,
        52 => LegacyMessageType::ReqSearchDescribe,
        53 => LegacyMessageType::RespSearchDescribe,
        54 => LegacyMessageType::ReqGroupAtomic,
        55 => LegacyMessageType::RespGroupAtomic,
        254 => LegacyMessageType::ConfigMismatch,
        255 => LegacyMessageType::PacketNop,
        _ => return Err(LegacyProtocolError::UnknownMessageType(value)),
    };

    Ok(message_type)
}

fn decode_return_code(value: u16) -> Result<LegacyReturnCode, LegacyProtocolError> {
    let code = match value {
        8320 => LegacyReturnCode::Success,
        8321 => LegacyReturnCode::NotFound,
        8322 => LegacyReturnCode::BadDimensionSpec,
        8323 => LegacyReturnCode::NotUs,
        8324 => LegacyReturnCode::ServerError,
        8325 => LegacyReturnCode::CompareFailed,
        8327 => LegacyReturnCode::ReadOnly,
        8328 => LegacyReturnCode::Overflow,
        8329 => LegacyReturnCode::Unauthorized,
        _ => return Err(LegacyProtocolError::UnknownReturnCode(value)),
    };

    Ok(code)
}

#[cfg(test)]
mod tests {
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
    fn count_request_round_trips() {
        let request = CountRequest {
            space: "profiles".to_owned(),
        };

        assert_eq!(CountRequest::decode_body(&request.encode_body()).unwrap(), request);
    }

    #[test]
    fn count_response_round_trips() {
        let response = CountResponse { count: 42 };

        assert_eq!(CountResponse::decode_body(&response.encode_body()).unwrap(), response);
    }

    #[test]
    fn get_request_round_trips() {
        let request = GetRequest {
            key: b"ada".to_vec(),
        };

        assert_eq!(GetRequest::decode_body(&request.encode_body()).unwrap(), request);
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

        assert_eq!(GetResponse::decode_body(&response.encode_body()).unwrap(), response);
    }

    #[test]
    fn atomic_request_round_trips() {
        let request = AtomicRequest {
            flags: LEGACY_ATOMIC_FLAG_HAS_ATTRIBUTES,
            key: b"ada".to_vec(),
            attributes: vec![
                GetAttribute {
                    name: "first".to_owned(),
                    value: GetValue::String("Ada".to_owned()),
                },
                GetAttribute {
                    name: "profile_views".to_owned(),
                    value: GetValue::Int(5),
                },
            ],
        };

        assert_eq!(AtomicRequest::decode_body(&request.encode_body()).unwrap(), request);
    }

    #[test]
    fn atomic_response_round_trips() {
        let response = AtomicResponse {
            status: LegacyReturnCode::Success,
        };

        assert_eq!(AtomicResponse::decode_body(&response.encode_body()).unwrap(), response);
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
            frame.len() - BUSYBEE_HEADER_SIZE
        );
    }
}

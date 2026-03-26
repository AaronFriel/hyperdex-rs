use thiserror::Error;

pub const BUSYBEE_HEADER_SIZE: usize = 4;
pub const LEGACY_REQUEST_HEADER_SIZE: usize = BUSYBEE_HEADER_SIZE + 1 + 1 + 8 + 8 + 8;
pub const LEGACY_RESPONSE_HEADER_SIZE: usize = BUSYBEE_HEADER_SIZE + 1 + 8 + 8;

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

#[derive(Debug, Error)]
pub enum LegacyProtocolError {
    #[error("unknown message type {0}")]
    UnknownMessageType(u8),
    #[error("buffer too short for header")]
    ShortBuffer,
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

pub fn config_mismatch_response(request: RequestHeader) -> ResponseHeader {
    ResponseHeader {
        message_type: LegacyMessageType::ConfigMismatch,
        target_virtual_server: request.target_virtual_server,
        nonce: request.nonce,
    }
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
}

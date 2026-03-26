use thiserror::Error;

pub const BUSYBEE_HEADER_SIZE: usize = 4;
pub const LEGACY_REQUEST_HEADER_SIZE: usize = BUSYBEE_HEADER_SIZE + 1 + 1 + 8 + 8 + 8;
pub const LEGACY_RESPONSE_HEADER_SIZE: usize = BUSYBEE_HEADER_SIZE + 1 + 8 + 8;
pub const GET_REQUEST_PREFIX_SIZE: usize = 2;
pub const COUNT_REQUEST_PREFIX_SIZE: usize = 2;
pub const COUNT_RESPONSE_BODY_SIZE: usize = 8;
pub const SEARCH_START_REQUEST_PREFIX_SIZE: usize = 2 + 8 + 2;
pub const SEARCH_CONTINUE_REQUEST_SIZE: usize = 8;
pub const ATOMIC_REQUEST_PREFIX_SIZE: usize = 2 + 1 + 2 + 2;
pub const ATOMIC_RESPONSE_BODY_SIZE: usize = 2;
pub const LEGACY_ATOMIC_FLAG_WRITE: u8 = 0x80;
pub const LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND: u8 = 0x01;
pub const LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND: u8 = 0x02;
pub const LEGACY_ATOMIC_FLAG_HAS_ATTRIBUTES: u8 = LEGACY_ATOMIC_FLAG_WRITE;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum LegacyPredicate {
    Equal = 9729,
    LessThan = 9738,
    LessThanOrEqual = 9730,
    GreaterThanOrEqual = 9731,
    GreaterThan = 9739,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyCheck {
    pub attribute: String,
    pub predicate: LegacyPredicate,
    pub value: LegacyValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LegacyFuncallName {
    Set = 1,
    StringAppend = 2,
    StringPrepend = 3,
    NumAdd = 4,
    NumSub = 5,
    NumMul = 6,
    NumDiv = 7,
    NumMod = 8,
    NumAnd = 9,
    NumOr = 10,
    NumXor = 11,
    ListLPush = 14,
    ListRPush = 15,
    SetAdd = 16,
    SetRemove = 17,
    SetIntersect = 18,
    SetUnion = 19,
    MapAdd = 20,
    MapRemove = 21,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyFuncall {
    pub attribute: String,
    pub name: LegacyFuncallName,
    pub arg1: LegacyValue,
    pub arg2: Option<LegacyValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetResponse {
    pub status: LegacyReturnCode,
    pub attributes: Vec<GetAttribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchStartRequest {
    pub space: String,
    pub search_id: u64,
    pub checks: Vec<LegacyCheck>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchContinueRequest {
    pub search_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchItemResponse {
    pub search_id: u64,
    pub key: Vec<u8>,
    pub attributes: Vec<GetAttribute>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchDoneResponse {
    pub search_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicRequest {
    pub flags: u8,
    pub key: Vec<u8>,
    pub checks: Vec<LegacyCheck>,
    pub funcalls: Vec<LegacyFuncall>,
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
    #[error("unknown predicate {0}")]
    UnknownPredicate(u16),
    #[error("unknown funcall name {0}")]
    UnknownFuncallName(u8),
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

impl SearchStartRequest {
    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        let space = self.space.as_bytes();
        body.extend_from_slice(&(space.len() as u16).to_be_bytes());
        body.extend_from_slice(space);
        body.extend_from_slice(&self.search_id.to_be_bytes());
        body.extend_from_slice(&(self.checks.len() as u16).to_be_bytes());
        encode_checks_into(&mut body, &self.checks);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < SEARCH_START_REQUEST_PREFIX_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let space_len =
            u16::from_be_bytes(bytes[..2].try_into().expect("fixed-width slice")) as usize;
        if bytes.len() < 2 + space_len + 10 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let space = std::str::from_utf8(&bytes[2..2 + space_len])
            .map_err(|_| LegacyProtocolError::InvalidUtf8)?
            .to_owned();
        let search_id = u64::from_be_bytes(
            bytes[2 + space_len..10 + space_len]
                .try_into()
                .expect("fixed-width slice"),
        );
        let check_count = u16::from_be_bytes(
            bytes[10 + space_len..12 + space_len]
                .try_into()
                .expect("fixed-width slice"),
        ) as usize;
        let (checks, offset) = decode_checks(bytes, 12 + space_len, check_count)?;

        if offset != bytes.len() {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            space,
            search_id,
            checks,
        })
    }
}

impl SearchContinueRequest {
    pub fn encode_body(self) -> [u8; SEARCH_CONTINUE_REQUEST_SIZE] {
        self.search_id.to_be_bytes()
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < SEARCH_CONTINUE_REQUEST_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            search_id: u64::from_be_bytes(bytes[..8].try_into().expect("fixed-width slice")),
        })
    }
}

impl SearchItemResponse {
    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&self.search_id.to_be_bytes());
        body.extend_from_slice(&(self.key.len() as u16).to_be_bytes());
        body.extend_from_slice(&self.key);
        body.extend_from_slice(&(self.attributes.len() as u16).to_be_bytes());
        encode_attributes_into(&mut body, &self.attributes);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < 12 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let search_id = u64::from_be_bytes(bytes[..8].try_into().expect("fixed-width slice"));
        let key_len = u16::from_be_bytes(bytes[8..10].try_into().expect("fixed-width slice"))
            as usize;
        if bytes.len() < 10 + key_len + 2 {
            return Err(LegacyProtocolError::ShortBuffer);
        }
        let key = bytes[10..10 + key_len].to_vec();
        let attr_count = u16::from_be_bytes(
            bytes[10 + key_len..12 + key_len]
                .try_into()
                .expect("fixed-width slice"),
        ) as usize;
        let (attributes, offset) = decode_attributes(bytes, 12 + key_len, attr_count)?;

        if offset != bytes.len() {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            search_id,
            key,
            attributes,
        })
    }
}

impl SearchDoneResponse {
    pub fn encode_body(self) -> [u8; SEARCH_CONTINUE_REQUEST_SIZE] {
        self.search_id.to_be_bytes()
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        SearchContinueRequest::decode_body(bytes).map(|request| Self {
            search_id: request.search_id,
        })
    }
}

impl AtomicRequest {
    pub fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(self.key.len() as u16).to_be_bytes());
        body.extend_from_slice(&self.key);
        body.push(self.flags);
        body.extend_from_slice(&(self.checks.len() as u16).to_be_bytes());
        encode_checks_into(&mut body, &self.checks);
        body.extend_from_slice(&(self.funcalls.len() as u16).to_be_bytes());
        encode_funcalls_into(&mut body, &self.funcalls);
        body
    }

    pub fn decode_body(bytes: &[u8]) -> Result<Self, LegacyProtocolError> {
        if bytes.len() < ATOMIC_REQUEST_PREFIX_SIZE {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let key_len = u16::from_be_bytes(bytes[..2].try_into().expect("fixed-width slice")) as usize;
        if bytes.len() < 2 + key_len + 5 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let key = bytes[2..2 + key_len].to_vec();
        let flags = bytes[2 + key_len];
        let check_count = u16::from_be_bytes(
            bytes[3 + key_len..5 + key_len]
                .try_into()
                .expect("fixed-width slice"),
        ) as usize;
        let (checks, offset) = decode_checks(bytes, 5 + key_len, check_count)?;

        if bytes.len() < offset + 2 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let funcall_count = u16::from_be_bytes(
            bytes[offset..offset + 2]
                .try_into()
                .expect("fixed-width slice"),
        ) as usize;
        let (funcalls, offset) = decode_funcalls(bytes, offset + 2, funcall_count)?;

        if offset != bytes.len() {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        Ok(Self {
            flags,
            key,
            checks,
            funcalls,
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
        encode_named_value_into(body, &attr.name, &attr.value);
    }
}

fn encode_checks_into(body: &mut Vec<u8>, checks: &[LegacyCheck]) {
    for check in checks {
        encode_named_value_into(body, &check.attribute, &check.value);
        body.extend_from_slice(&(check.predicate as u16).to_be_bytes());
    }
}

fn encode_funcalls_into(body: &mut Vec<u8>, funcalls: &[LegacyFuncall]) {
    for funcall in funcalls {
        body.push(funcall.name as u8);
        encode_named_value_into(body, &funcall.attribute, &funcall.arg1);
        body.push(u8::from(funcall.arg2.is_some()));

        if let Some(arg2) = &funcall.arg2 {
            encode_value_into(body, arg2);
        }
    }
}

fn encode_named_value_into(body: &mut Vec<u8>, name: &str, value: &GetValue) {
    let name_bytes = name.as_bytes();
    body.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
    body.extend_from_slice(name_bytes);
    encode_value_into(body, value);
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
        let ((name, value), next_offset) = decode_named_value(bytes, offset)?;
        offset = next_offset;
        attributes.push(GetAttribute { name, value });
    }

    Ok((attributes, offset))
}

fn decode_checks(
    bytes: &[u8],
    mut offset: usize,
    check_count: usize,
) -> Result<(Vec<LegacyCheck>, usize), LegacyProtocolError> {
    let mut checks = Vec::with_capacity(check_count);

    for _ in 0..check_count {
        let ((attribute, value), next_offset) = decode_named_value(bytes, offset)?;
        offset = next_offset;

        if bytes.len() < offset + 2 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let predicate = decode_predicate(u16::from_be_bytes(
            bytes[offset..offset + 2]
                .try_into()
                .expect("fixed-width slice"),
        ))?;
        offset += 2;

        checks.push(LegacyCheck {
            attribute,
            predicate,
            value,
        });
    }

    Ok((checks, offset))
}

fn decode_funcalls(
    bytes: &[u8],
    mut offset: usize,
    funcall_count: usize,
) -> Result<(Vec<LegacyFuncall>, usize), LegacyProtocolError> {
    let mut funcalls = Vec::with_capacity(funcall_count);

    for _ in 0..funcall_count {
        if bytes.len() < offset + 1 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let name = decode_funcall_name(bytes[offset])?;
        offset += 1;

        let ((attribute, arg1), next_offset) = decode_named_value(bytes, offset)?;
        offset = next_offset;

        if bytes.len() < offset + 1 {
            return Err(LegacyProtocolError::ShortBuffer);
        }

        let has_arg2 = bytes[offset] != 0;
        offset += 1;
        let (arg2, next_offset) = if has_arg2 {
            let (value, next_offset) = decode_value(bytes, offset)?;
            (Some(value), next_offset)
        } else {
            (None, offset)
        };
        offset = next_offset;

        funcalls.push(LegacyFuncall {
            attribute,
            name,
            arg1,
            arg2,
        });
    }

    Ok((funcalls, offset))
}

fn decode_named_value(
    bytes: &[u8],
    mut offset: usize,
) -> Result<((String, GetValue), usize), LegacyProtocolError> {
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
    Ok(((name, value), next_offset))
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

fn decode_predicate(value: u16) -> Result<LegacyPredicate, LegacyProtocolError> {
    let predicate = match value {
        9729 => LegacyPredicate::Equal,
        9738 => LegacyPredicate::LessThan,
        9730 => LegacyPredicate::LessThanOrEqual,
        9731 => LegacyPredicate::GreaterThanOrEqual,
        9739 => LegacyPredicate::GreaterThan,
        _ => return Err(LegacyProtocolError::UnknownPredicate(value)),
    };

    Ok(predicate)
}

fn decode_funcall_name(value: u8) -> Result<LegacyFuncallName, LegacyProtocolError> {
    let name = match value {
        1 => LegacyFuncallName::Set,
        2 => LegacyFuncallName::StringAppend,
        3 => LegacyFuncallName::StringPrepend,
        4 => LegacyFuncallName::NumAdd,
        5 => LegacyFuncallName::NumSub,
        6 => LegacyFuncallName::NumMul,
        7 => LegacyFuncallName::NumDiv,
        8 => LegacyFuncallName::NumMod,
        9 => LegacyFuncallName::NumAnd,
        10 => LegacyFuncallName::NumOr,
        11 => LegacyFuncallName::NumXor,
        14 => LegacyFuncallName::ListLPush,
        15 => LegacyFuncallName::ListRPush,
        16 => LegacyFuncallName::SetAdd,
        17 => LegacyFuncallName::SetRemove,
        18 => LegacyFuncallName::SetIntersect,
        19 => LegacyFuncallName::SetUnion,
        20 => LegacyFuncallName::MapAdd,
        21 => LegacyFuncallName::MapRemove,
        _ => return Err(LegacyProtocolError::UnknownFuncallName(value)),
    };

    Ok(name)
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

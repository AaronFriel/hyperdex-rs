use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use cluster_config::{ClusterConfig, ClusterNode};
use data_model::{SchemaFormat, Space, SpaceName, SpaceOptions, Subspace, ValueKind};
use serde::{Deserialize, Serialize};

pub const BUSYBEE_HEADER_SIZE: usize = 4;
pub const BUSYBEE_HEADER_IDENTIFY: u32 = 0x8000_0000;
pub const BUSYBEE_HEADER_EXTENDED: u32 = 0x4000_0000;

const BUSYBEE_SIZE_MASK: u32 = 0x00ff_ffff;
const REPLICANT_OBJECT_HYPERDEX: &[u8] = b"hyperdex";
const REPLICANT_CONDITION_CONFIG: &[u8] = b"config";
const REPLICANT_CONDITION_STABLE: &[u8] = b"stable";
const REPLICANT_FUNCTION_SPACE_ADD: &[u8] = b"space_add";
const REPLICANT_FUNCTION_SPACE_RM: &[u8] = b"space_rm";
const REPLICANT_ROBUST_PARAMS_BYTES: usize = 16;
const HYPERDATATYPE_STRING: u16 = 9217;
const HYPERDATATYPE_INT64: u16 = 9218;
const HYPERDATATYPE_FLOAT: u16 = 9219;
const HYPERDATATYPE_DOCUMENT: u16 = 9223;
const HYPERDATATYPE_LIST_GENERIC: u16 = 9280;
const HYPERDATATYPE_SET_GENERIC: u16 = 9344;
const HYPERDATATYPE_MAP_GENERIC: u16 = 9408;
const HYPERDATATYPE_TIMESTAMP_GENERIC: u16 = 9472;
const HYPERDATATYPE_MACAROON_SECRET: u16 = 9664;
const CAPTURED_INITIAL_CONFIG_FOLLOW_REQUEST: [u8; 25] = [
    0x80, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x1c,
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigView {
    pub version: u64,
    pub stable_through: u64,
    pub cluster: ClusterConfig,
    pub spaces: Vec<Space>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum CoordinatorReturnCode {
    Success = 8832,
    Malformed = 8833,
    Duplicate = 8834,
    NotFound = 8835,
    Uninitialized = 8837,
    NoCanDo = 8839,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum LegacyAdminReturnCode {
    Success = 8704,
    CoordFail = 8774,
    BadSpace = 8775,
    Duplicate = 8776,
    NotFound = 8777,
    Internal = 8829,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyAdminRequest {
    SpaceAddDsl(String),
    SpaceRm(SpaceName),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorAdminRequest {
    DaemonRegister(ClusterNode),
    SpaceAdd(Space),
    SpaceRm(SpaceName),
    WaitUntilStable,
    ConfigGet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminRequest {
    RegisterDaemon(ClusterNode),
    CreateSpace(Space),
    CreateSpaceDsl(String),
    DropSpace(SpaceName),
    ListSpaces,
    DumpConfig,
    WaitUntilStable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminResponse {
    Unit,
    Spaces(Vec<SpaceName>),
    Config(ConfigView),
    Stable { version: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusyBeeFrame {
    pub flags: u32,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReplicantNetworkMsgtype {
    Bootstrap = 28,
    CondWait = 69,
    Call = 70,
    GetRobustParams = 72,
    CallRobust = 73,
    ClientResponse = 224,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ReplicantReturnCode {
    Success = 5120,
    Maybe = 5121,
    SeeErrno = 5122,
    ClusterJump = 5123,
    CommFailed = 5124,
    ObjNotFound = 5184,
    ObjExist = 5185,
    FuncNotFound = 5186,
    CondNotFound = 5187,
    CondDestroyed = 5188,
    ServerError = 5248,
    Timeout = 5312,
    Interrupted = 5313,
    NonePending = 5314,
    Internal = 5373,
    Exception = 5374,
    Garbage = 5375,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplicantAdminRequestMessage {
    CondWait {
        nonce: u64,
        object: Vec<u8>,
        condition: Vec<u8>,
        state: u64,
    },
    Call {
        nonce: u64,
        object: Vec<u8>,
        function: Vec<u8>,
        input: Vec<u8>,
    },
    CallRobust {
        nonce: u64,
        command_nonce: u64,
        min_slot: u64,
        object: Vec<u8>,
        function: Vec<u8>,
        input: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicantCallCompletion {
    pub nonce: u64,
    pub status: ReplicantReturnCode,
    pub output: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicantConditionCompletion {
    pub nonce: u64,
    pub status: ReplicantReturnCode,
    pub state: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicantRobustParams {
    pub nonce: u64,
    pub command_nonce: u64,
    pub min_slot: u64,
}

#[async_trait]
pub trait HyperdexAdminService: Send + Sync {
    async fn handle(&self, request: AdminRequest) -> anyhow::Result<AdminResponse>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PackedSpaceAttribute {
    name: String,
    datatype: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PackedSpaceSubspace {
    attrs: Vec<u16>,
    regions_len: usize,
}

impl CoordinatorAdminRequest {
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::DaemonRegister(_) => "daemon_register",
            Self::SpaceAdd(_) => "space_add",
            Self::SpaceRm(_) => "space_rm",
            Self::WaitUntilStable => "wait_until_stable",
            Self::ConfigGet => "config_get",
        }
    }
}

impl CoordinatorReturnCode {
    pub fn encode(self) -> [u8; 2] {
        (self as u16).to_be_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(anyhow!("buffer too short for coordinator return code"));
        }

        match u16::from_be_bytes([bytes[0], bytes[1]]) {
            8832 => Ok(Self::Success),
            8833 => Ok(Self::Malformed),
            8834 => Ok(Self::Duplicate),
            8835 => Ok(Self::NotFound),
            8837 => Ok(Self::Uninitialized),
            8839 => Ok(Self::NoCanDo),
            other => Err(anyhow!("unknown coordinator return code {other}")),
        }
    }

    pub fn legacy_admin_status(self) -> LegacyAdminReturnCode {
        match self {
            Self::Success => LegacyAdminReturnCode::Success,
            Self::Duplicate => LegacyAdminReturnCode::Duplicate,
            Self::NotFound => LegacyAdminReturnCode::NotFound,
            Self::Uninitialized | Self::NoCanDo => LegacyAdminReturnCode::CoordFail,
            Self::Malformed => LegacyAdminReturnCode::Internal,
        }
    }
}

impl BusyBeeFrame {
    pub fn new(payload: Vec<u8>) -> Self {
        Self { flags: 0, payload }
    }

    pub fn identify(payload: Vec<u8>) -> Self {
        Self {
            flags: BUSYBEE_HEADER_IDENTIFY,
            payload,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.flags & BUSYBEE_SIZE_MASK != 0 {
            bail!("busybee flags overlap the size bits");
        }

        let total_len = BUSYBEE_HEADER_SIZE + self.payload.len();

        if total_len > BUSYBEE_SIZE_MASK as usize {
            bail!("busybee extended frames are not supported by this codec");
        }

        let mut out = Vec::with_capacity(total_len);
        let header = self.flags | total_len as u32;
        out.extend_from_slice(&header.to_be_bytes());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < BUSYBEE_HEADER_SIZE {
            bail!("busybee frame is shorter than the header");
        }

        let header = u32::from_be_bytes(bytes[..BUSYBEE_HEADER_SIZE].try_into().unwrap());
        let total_len = (header & BUSYBEE_SIZE_MASK) as usize;

        if total_len < BUSYBEE_HEADER_SIZE {
            bail!("busybee frame size {total_len} is too small");
        }

        if total_len != bytes.len() {
            bail!(
                "busybee frame size header says {total_len} bytes but buffer has {}",
                bytes.len()
            );
        }

        let flags = header & !BUSYBEE_SIZE_MASK;

        if flags & BUSYBEE_HEADER_EXTENDED != 0 {
            bail!("busybee extended frames are not supported by this codec");
        }

        Ok(Self {
            flags,
            payload: bytes[BUSYBEE_HEADER_SIZE..].to_vec(),
        })
    }

    pub fn decode_stream(mut bytes: &[u8]) -> Result<Vec<Self>> {
        let mut frames = Vec::new();

        while !bytes.is_empty() {
            if bytes.len() < BUSYBEE_HEADER_SIZE {
                bail!("busybee stream ended in the middle of a header");
            }

            let header = u32::from_be_bytes(bytes[..BUSYBEE_HEADER_SIZE].try_into().unwrap());
            let total_len = (header & BUSYBEE_SIZE_MASK) as usize;

            if total_len < BUSYBEE_HEADER_SIZE {
                bail!("busybee frame size {total_len} is too small");
            }

            if bytes.len() < total_len {
                bail!("busybee stream ended in the middle of a {total_len}-byte frame");
            }

            frames.push(Self::decode(&bytes[..total_len])?);
            bytes = &bytes[total_len..];
        }

        Ok(frames)
    }

    pub fn encode_stream(frames: &[Self]) -> Result<Vec<u8>> {
        let mut out = Vec::new();

        for frame in frames {
            out.extend_from_slice(&frame.encode()?);
        }

        Ok(out)
    }
}

impl ReplicantNetworkMsgtype {
    pub fn encode(self) -> u8 {
        self as u8
    }

    pub fn decode(byte: u8) -> Result<Self> {
        match byte {
            28 => Ok(Self::Bootstrap),
            69 => Ok(Self::CondWait),
            70 => Ok(Self::Call),
            72 => Ok(Self::GetRobustParams),
            73 => Ok(Self::CallRobust),
            224 => Ok(Self::ClientResponse),
            other => Err(anyhow!("unknown replicant network msgtype {other}")),
        }
    }
}

impl ReplicantReturnCode {
    pub fn encode(self) -> [u8; 2] {
        (self as u16).to_be_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            bail!("buffer too short for replicant return code");
        }

        match u16::from_be_bytes([bytes[0], bytes[1]]) {
            5120 => Ok(Self::Success),
            5121 => Ok(Self::Maybe),
            5122 => Ok(Self::SeeErrno),
            5123 => Ok(Self::ClusterJump),
            5124 => Ok(Self::CommFailed),
            5184 => Ok(Self::ObjNotFound),
            5185 => Ok(Self::ObjExist),
            5186 => Ok(Self::FuncNotFound),
            5187 => Ok(Self::CondNotFound),
            5188 => Ok(Self::CondDestroyed),
            5248 => Ok(Self::ServerError),
            5312 => Ok(Self::Timeout),
            5313 => Ok(Self::Interrupted),
            5314 => Ok(Self::NonePending),
            5373 => Ok(Self::Internal),
            5374 => Ok(Self::Exception),
            5375 => Ok(Self::Garbage),
            other => Err(anyhow!("unknown replicant return code {other}")),
        }
    }
}

impl ReplicantAdminRequestMessage {
    pub fn bootstrap_request() -> Vec<u8> {
        CAPTURED_INITIAL_CONFIG_FOLLOW_REQUEST.to_vec()
    }

    pub fn wait_until_stable(nonce: u64, state: u64) -> Self {
        Self::CondWait {
            nonce,
            object: REPLICANT_OBJECT_HYPERDEX.to_vec(),
            condition: REPLICANT_CONDITION_STABLE.to_vec(),
            state,
        }
    }

    pub fn space_add(nonce: u64, encoded_space: Vec<u8>) -> Self {
        Self::Call {
            nonce,
            object: REPLICANT_OBJECT_HYPERDEX.to_vec(),
            function: REPLICANT_FUNCTION_SPACE_ADD.to_vec(),
            input: encoded_space,
        }
    }

    pub fn space_rm(nonce: u64, space_name: SpaceName) -> Self {
        let mut input = space_name.into_bytes();
        input.push(0);
        Self::Call {
            nonce,
            object: REPLICANT_OBJECT_HYPERDEX.to_vec(),
            function: REPLICANT_FUNCTION_SPACE_RM.to_vec(),
            input,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();

        match self {
            Self::CondWait {
                nonce,
                object,
                condition,
                state,
            } => {
                out.push(ReplicantNetworkMsgtype::CondWait.encode());
                out.extend_from_slice(&nonce.to_be_bytes());
                out.extend_from_slice(&encode_varint_slice(object));
                out.extend_from_slice(&encode_varint_slice(condition));
                out.extend_from_slice(&state.to_be_bytes());
            }
            Self::Call {
                nonce,
                object,
                function,
                input,
            } => {
                out.push(ReplicantNetworkMsgtype::Call.encode());
                out.extend_from_slice(&nonce.to_be_bytes());
                out.extend_from_slice(&encode_varint_slice(object));
                out.extend_from_slice(&encode_varint_slice(function));
                out.extend_from_slice(&encode_varint_slice(input));
            }
            Self::CallRobust {
                nonce,
                command_nonce,
                min_slot,
                object,
                function,
                input,
            } => {
                out.push(ReplicantNetworkMsgtype::CallRobust.encode());
                out.extend_from_slice(&nonce.to_be_bytes());
                out.extend_from_slice(&command_nonce.to_be_bytes());
                out.extend_from_slice(&min_slot.to_be_bytes());
                out.extend_from_slice(&encode_varint_slice(object));
                out.extend_from_slice(&encode_varint_slice(function));
                out.extend_from_slice(&encode_varint_slice(input));
            }
        }

        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            bail!("replicant request is empty");
        }

        let msgtype = ReplicantNetworkMsgtype::decode(bytes[0])?;
        let mut cursor = 1;

        match msgtype {
            ReplicantNetworkMsgtype::CondWait => {
                let nonce = decode_u64_be(bytes, &mut cursor)?;
                let object = decode_varint_slice_at(bytes, &mut cursor)?;
                let condition = decode_varint_slice_at(bytes, &mut cursor)?;
                let state = decode_u64_be(bytes, &mut cursor)?;
                expect_consumed(bytes, cursor, "replicant cond_wait request")?;

                Ok(Self::CondWait {
                    nonce,
                    object,
                    condition,
                    state,
                })
            }
            ReplicantNetworkMsgtype::Call => {
                let nonce = decode_u64_be(bytes, &mut cursor)?;
                let object = decode_varint_slice_at(bytes, &mut cursor)?;
                let function = decode_varint_slice_at(bytes, &mut cursor)?;
                let input = decode_varint_slice_at(bytes, &mut cursor)?;
                expect_consumed(bytes, cursor, "replicant call request")?;

                Ok(Self::Call {
                    nonce,
                    object,
                    function,
                    input,
                })
            }
            ReplicantNetworkMsgtype::CallRobust => {
                let nonce = decode_u64_be(bytes, &mut cursor)?;
                let command_nonce = decode_u64_be(bytes, &mut cursor)?;
                let min_slot = decode_u64_be(bytes, &mut cursor)?;
                let object = decode_varint_slice_at(bytes, &mut cursor)?;
                let function = decode_varint_slice_at(bytes, &mut cursor)?;
                let input = decode_varint_slice_at(bytes, &mut cursor)?;
                expect_consumed(bytes, cursor, "replicant robust call request")?;

                Ok(Self::CallRobust {
                    nonce,
                    command_nonce,
                    min_slot,
                    object,
                    function,
                    input,
                })
            }
            other => bail!("replicant msgtype {other:?} is not an admin request"),
        }
    }

    pub fn nonce(&self) -> u64 {
        match self {
            Self::CondWait { nonce, .. }
            | Self::Call { nonce, .. }
            | Self::CallRobust { nonce, .. } => *nonce,
        }
    }

    pub fn into_coordinator_request(self) -> Result<CoordinatorAdminRequest> {
        match self {
            Self::CondWait {
                object,
                condition,
                ..
            } if object == REPLICANT_OBJECT_HYPERDEX && condition == REPLICANT_CONDITION_STABLE => {
                Ok(CoordinatorAdminRequest::WaitUntilStable)
            }
            Self::CondWait {
                object,
                condition,
                ..
            } if object == REPLICANT_OBJECT_HYPERDEX && condition == REPLICANT_CONDITION_CONFIG => {
                Ok(CoordinatorAdminRequest::ConfigGet)
            }
            Self::Call {
                object,
                function,
                input,
                ..
            }
            | Self::CallRobust {
                object,
                function,
                input,
                ..
            } if object == REPLICANT_OBJECT_HYPERDEX && function == REPLICANT_FUNCTION_SPACE_ADD => {
                Ok(CoordinatorAdminRequest::SpaceAdd(decode_packed_hyperdex_space(
                    &input,
                )?))
            }
            Self::Call {
                object,
                function,
                input,
                ..
            }
            | Self::CallRobust {
                object,
                function,
                input,
                ..
            } if object == REPLICANT_OBJECT_HYPERDEX && function == REPLICANT_FUNCTION_SPACE_RM => {
                Ok(CoordinatorAdminRequest::SpaceRm(
                    decode_c_string(&input)?.to_owned(),
                ))
            }
            other => Err(anyhow!(
                "unsupported replicant admin request for coordinator mapping: {other:?}"
            )),
        }
    }
}

impl ReplicantCallCompletion {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(ReplicantNetworkMsgtype::ClientResponse.encode());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.status.encode());
        out.extend_from_slice(&encode_varint_slice(&self.output));
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            bail!("replicant client response is empty");
        }

        let msgtype = ReplicantNetworkMsgtype::decode(bytes[0])?;

        if msgtype != ReplicantNetworkMsgtype::ClientResponse {
            bail!("replicant call completion must start with CLIENT_RESPONSE");
        }

        let mut cursor = 1;
        let nonce = decode_u64_be(bytes, &mut cursor)?;
        let status = decode_return_code_at(bytes, &mut cursor)?;
        let output = decode_varint_slice_at(bytes, &mut cursor)?;
        expect_consumed(bytes, cursor, "replicant call completion")?;

        Ok(Self {
            nonce,
            status,
            output,
        })
    }
}

impl ReplicantConditionCompletion {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(ReplicantNetworkMsgtype::ClientResponse.encode());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.status.encode());
        out.extend_from_slice(&self.state.to_be_bytes());
        out.extend_from_slice(&encode_varint_slice(&self.data));
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            bail!("replicant client response is empty");
        }

        let msgtype = ReplicantNetworkMsgtype::decode(bytes[0])?;

        if msgtype != ReplicantNetworkMsgtype::ClientResponse {
            bail!("replicant condition completion must start with CLIENT_RESPONSE");
        }

        let mut cursor = 1;
        let nonce = decode_u64_be(bytes, &mut cursor)?;
        let status = decode_return_code_at(bytes, &mut cursor)?;
        let state = decode_u64_be(bytes, &mut cursor)?;
        let data = decode_varint_slice_at(bytes, &mut cursor)?;
        expect_consumed(bytes, cursor, "replicant condition completion")?;

        Ok(Self {
            nonce,
            status,
            state,
            data,
        })
    }
}

impl ReplicantRobustParams {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(ReplicantNetworkMsgtype::ClientResponse.encode());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.command_nonce.to_be_bytes());
        out.extend_from_slice(&self.min_slot.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            bail!("replicant client response is empty");
        }

        let msgtype = ReplicantNetworkMsgtype::decode(bytes[0])?;

        if msgtype != ReplicantNetworkMsgtype::ClientResponse {
            bail!("replicant robust params must start with CLIENT_RESPONSE");
        }

        let mut cursor = 1;
        let nonce = decode_u64_be(bytes, &mut cursor)?;

        if bytes.len().saturating_sub(cursor) != REPLICANT_ROBUST_PARAMS_BYTES {
            bail!("replicant robust params response must contain command_nonce and min_slot");
        }

        let command_nonce = decode_u64_be(bytes, &mut cursor)?;
        let min_slot = decode_u64_be(bytes, &mut cursor)?;
        expect_consumed(bytes, cursor, "replicant robust params response")?;

        Ok(Self {
            nonce,
            command_nonce,
            min_slot,
        })
    }
}

pub fn encode_varint_u64(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();

    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80;
        }

        out.push(byte);

        if value == 0 {
            return out;
        }
    }
}

pub fn decode_varint_u64(bytes: &[u8]) -> Result<(u64, usize)> {
    let mut value = 0_u64;

    for (shift, byte) in bytes.iter().copied().enumerate() {
        if shift == 10 {
            bail!("varint exceeds the maximum 10-byte u64 encoding");
        }

        value |= ((byte & 0x7f) as u64) << (shift * 7);

        if byte & 0x80 == 0 {
            return Ok((value, shift + 1));
        }
    }

    bail!("truncated varint")
}

pub fn encode_varint_slice(bytes: &[u8]) -> Vec<u8> {
    let mut out = encode_varint_u64(bytes.len() as u64);
    out.extend_from_slice(bytes);
    out
}

pub fn decode_varint_slice(bytes: &[u8]) -> Result<(Vec<u8>, usize)> {
    let (len, header_len) = decode_varint_u64(bytes)?;
    let len = usize::try_from(len).map_err(|_| anyhow!("slice length does not fit usize"))?;
    let end = header_len
        .checked_add(len)
        .ok_or_else(|| anyhow!("slice length overflow"))?;

    if bytes.len() < end {
        bail!(
            "slice payload is truncated: header says {len} bytes but only {} remain",
            bytes.len().saturating_sub(header_len)
        );
    }

    Ok((bytes[header_len..end].to_vec(), end))
}

fn decode_u64_be(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let end = cursor
        .checked_add(8)
        .ok_or_else(|| anyhow!("u64 cursor overflow"))?;

    if bytes.len() < end {
        bail!("buffer too short for a u64");
    }

    let value = u64::from_be_bytes(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

pub fn decode_packed_hyperdex_space(bytes: &[u8]) -> Result<Space> {
    let mut cursor = 0;
    let _space_id = decode_u64_be(bytes, &mut cursor)?;
    let name = decode_len32_string(bytes, &mut cursor, "space name")?;
    let fault_tolerance = decode_u64_be(bytes, &mut cursor)?;
    let attrs_len = decode_u16_be(bytes, &mut cursor)? as usize;
    let subspaces_len = decode_u16_be(bytes, &mut cursor)? as usize;
    let indices_len = decode_u16_be(bytes, &mut cursor)? as usize;

    let mut attrs = Vec::with_capacity(attrs_len);
    for _ in 0..attrs_len {
        let attr_name = decode_len32_string(bytes, &mut cursor, "attribute name")?;
        let datatype = decode_u16_be(bytes, &mut cursor)?;
        attrs.push(PackedSpaceAttribute {
            name: attr_name,
            datatype,
        });
    }

    if attrs.is_empty() {
        bail!("packed hyperdex::space did not include a key attribute");
    }

    let mut subspaces = Vec::with_capacity(subspaces_len);
    for _ in 0..subspaces_len {
        subspaces.push(decode_packed_subspace(bytes, &mut cursor)?);
    }

    for _ in 0..indices_len {
        decode_packed_index(bytes, &mut cursor)?;
    }

    expect_consumed(bytes, cursor, "packed hyperdex::space")?;

    let key_attribute = attrs[0].name.clone();
    let mut attribute_defs = Vec::new();
    for attr in attrs.iter().skip(1) {
        if attr.datatype == HYPERDATATYPE_MACAROON_SECRET {
            continue;
        }
        attribute_defs.push(data_model::AttributeDefinition {
            name: attr.name.clone(),
            kind: decode_hyperdatatype(attr.datatype)?,
        });
    }

    let mut rust_subspaces = Vec::new();
    for subspace in subspaces.iter().skip(1) {
        let mut dimensions = Vec::with_capacity(subspace.attrs.len());
        for attr_index in &subspace.attrs {
            let attr = attrs
                .get(*attr_index as usize)
                .ok_or_else(|| anyhow!("subspace attribute index {attr_index} out of range"))?;
            if attr.name == key_attribute || attr.datatype == HYPERDATATYPE_MACAROON_SECRET {
                continue;
            }
            dimensions.push(attr.name.clone());
        }
        if !dimensions.is_empty() {
            rust_subspaces.push(Subspace { dimensions });
        }
    }

    let partitions = subspaces
        .first()
        .map(|subspace| subspace.regions_len as u32)
        .filter(|count| *count > 0)
        .unwrap_or_else(|| SpaceOptions::default().partitions);

    Ok(Space {
        name,
        key_attribute,
        attributes: attribute_defs,
        subspaces: rust_subspaces,
        options: SpaceOptions {
            fault_tolerance: u32::try_from(fault_tolerance)
                .map_err(|_| anyhow!("fault tolerance {fault_tolerance} does not fit in u32"))?,
            partitions,
            schema_format: SchemaFormat::HyperDexDsl,
        },
    })
}

fn decode_packed_subspace(bytes: &[u8], cursor: &mut usize) -> Result<PackedSpaceSubspace> {
    let _subspace_id = decode_u64_be(bytes, cursor)?;
    let attrs_len = decode_u16_be(bytes, cursor)? as usize;
    let regions_len = decode_u32_be(bytes, cursor)? as usize;
    let mut attrs = Vec::with_capacity(attrs_len);

    for _ in 0..attrs_len {
        attrs.push(decode_u16_be(bytes, cursor)?);
    }

    for _ in 0..regions_len {
        decode_packed_region(bytes, cursor)?;
    }

    Ok(PackedSpaceSubspace { attrs, regions_len })
}

fn decode_packed_region(bytes: &[u8], cursor: &mut usize) -> Result<()> {
    let _region_id = decode_u64_be(bytes, cursor)?;
    let hashes_len = decode_u16_be(bytes, cursor)? as usize;
    let replicas_len = decode_u8(bytes, cursor)? as usize;

    for _ in 0..hashes_len {
        let _lower = decode_u64_be(bytes, cursor)?;
        let _upper = decode_u64_be(bytes, cursor)?;
    }

    for _ in 0..replicas_len {
        let _server_id = decode_u64_be(bytes, cursor)?;
        let _virtual_server_id = decode_u64_be(bytes, cursor)?;
    }

    Ok(())
}

fn decode_packed_index(bytes: &[u8], cursor: &mut usize) -> Result<()> {
    let _index_type = decode_u8(bytes, cursor)?;
    let _index_id = decode_u64_be(bytes, cursor)?;
    let _attr = decode_u16_be(bytes, cursor)?;
    let _extra = decode_len32_slice(bytes, cursor, "index extra")?;
    Ok(())
}

fn decode_hyperdatatype(datatype: u16) -> Result<ValueKind> {
    if datatype == HYPERDATATYPE_STRING {
        return Ok(ValueKind::String);
    }
    if datatype == HYPERDATATYPE_INT64 {
        return Ok(ValueKind::Int);
    }
    if datatype == HYPERDATATYPE_FLOAT {
        return Ok(ValueKind::Float);
    }
    if datatype == HYPERDATATYPE_DOCUMENT {
        return Ok(ValueKind::Document);
    }
    if (HYPERDATATYPE_LIST_GENERIC..HYPERDATATYPE_SET_GENERIC).contains(&datatype) {
        return Ok(ValueKind::List(Box::new(decode_container_elem(datatype)?)));
    }
    if (HYPERDATATYPE_SET_GENERIC..HYPERDATATYPE_MAP_GENERIC).contains(&datatype) {
        return Ok(ValueKind::Set(Box::new(decode_container_elem(datatype)?)));
    }
    if (HYPERDATATYPE_MAP_GENERIC..HYPERDATATYPE_TIMESTAMP_GENERIC).contains(&datatype) {
        let (key, value) = decode_map_types(datatype)?;
        return Ok(ValueKind::Map {
            key: Box::new(key),
            value: Box::new(value),
        });
    }
    if (HYPERDATATYPE_TIMESTAMP_GENERIC..HYPERDATATYPE_MACAROON_SECRET).contains(&datatype) {
        return Ok(ValueKind::Timestamp(decode_time_unit(datatype)?));
    }

    bail!("unsupported hyperdatatype {datatype}");
}

fn decode_container_elem(datatype: u16) -> Result<ValueKind> {
    decode_primitive_hyperdatatype(datatype & 0x2407)
}

fn decode_map_types(datatype: u16) -> Result<(ValueKind, ValueKind)> {
    let key = decode_primitive_hyperdatatype(((datatype & 0x0038) >> 3) | (datatype & 0x2400))?;
    let value = decode_primitive_hyperdatatype(datatype & 0x2407)?;
    Ok((key, value))
}

fn decode_primitive_hyperdatatype(datatype: u16) -> Result<ValueKind> {
    match datatype {
        9216 => Ok(ValueKind::Bytes),
        HYPERDATATYPE_STRING => Ok(ValueKind::String),
        HYPERDATATYPE_INT64 => Ok(ValueKind::Int),
        HYPERDATATYPE_FLOAT => Ok(ValueKind::Float),
        HYPERDATATYPE_DOCUMENT => Ok(ValueKind::Document),
        other => bail!("unsupported primitive hyperdatatype {other}"),
    }
}

fn decode_time_unit(datatype: u16) -> Result<data_model::TimeUnit> {
    match datatype {
        9472 | 9473 => Ok(data_model::TimeUnit::Second),
        9474 => Ok(data_model::TimeUnit::Minute),
        9475 => Ok(data_model::TimeUnit::Hour),
        9476 => Ok(data_model::TimeUnit::Day),
        9477 => Ok(data_model::TimeUnit::Week),
        9478 => Ok(data_model::TimeUnit::Month),
        other => bail!("unsupported timestamp hyperdatatype {other}"),
    }
}

fn decode_c_string(bytes: &[u8]) -> Result<&str> {
    let Some((&0, prefix)) = bytes.split_last() else {
        bail!("expected nul-terminated string");
    };
    Ok(std::str::from_utf8(prefix)?)
}

fn decode_len32_string(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<String> {
    let slice = decode_len32_slice(bytes, cursor, label)?;
    Ok(std::str::from_utf8(&slice)?.to_owned())
}

fn decode_len32_slice(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<Vec<u8>> {
    let len = decode_u32_be(bytes, cursor)? as usize;
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| anyhow!("cursor overflow while decoding {label}"))?;
    if end > bytes.len() {
        bail!("buffer too short for {label}");
    }
    let slice = bytes[*cursor..end].to_vec();
    *cursor = end;
    Ok(slice)
}

fn decode_u32_be(bytes: &[u8], cursor: &mut usize) -> Result<u32> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| anyhow!("cursor overflow while decoding u32"))?;
    if end > bytes.len() {
        bail!("buffer too short for u32");
    }
    let value = u32::from_be_bytes(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn decode_u16_be(bytes: &[u8], cursor: &mut usize) -> Result<u16> {
    let end = cursor
        .checked_add(2)
        .ok_or_else(|| anyhow!("cursor overflow while decoding u16"))?;
    if end > bytes.len() {
        bail!("buffer too short for u16");
    }
    let value = u16::from_be_bytes(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn decode_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8> {
    let end = cursor
        .checked_add(1)
        .ok_or_else(|| anyhow!("cursor overflow while decoding u8"))?;
    if end > bytes.len() {
        bail!("buffer too short for u8");
    }
    let value = bytes[*cursor];
    *cursor = end;
    Ok(value)
}

fn decode_return_code_at(bytes: &[u8], cursor: &mut usize) -> Result<ReplicantReturnCode> {
    let end = cursor
        .checked_add(2)
        .ok_or_else(|| anyhow!("return-code cursor overflow"))?;

    if bytes.len() < end {
        bail!("buffer too short for a replicant return code");
    }

    let status = ReplicantReturnCode::decode(&bytes[*cursor..end])?;
    *cursor = end;
    Ok(status)
}

fn decode_varint_slice_at(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>> {
    let (value, consumed) = decode_varint_slice(&bytes[*cursor..])?;
    *cursor += consumed;
    Ok(value)
}

fn expect_consumed(bytes: &[u8], cursor: usize, context: &str) -> Result<()> {
    if bytes.len() != cursor {
        bail!(
            "{context} has {} trailing bytes",
            bytes.len().saturating_sub(cursor)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_model::{AttributeDefinition, SchemaFormat, SpaceOptions, Subspace, ValueKind};

    #[test]
    fn busybee_frame_round_trip() {
        let frame = BusyBeeFrame::identify(vec![0_u8; 16]);
        let encoded = frame.encode().unwrap();

        assert_eq!(BusyBeeFrame::decode(&encoded).unwrap(), frame);
    }

    #[test]
    fn varint_slice_round_trip() {
        let payload = b"hyperdex-admin";
        let encoded = encode_varint_slice(payload);
        let (decoded, consumed) = decode_varint_slice(&encoded).unwrap();

        assert_eq!(decoded, payload);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn captured_bootstrap_request_matches_original_tool_bytes() {
        let encoded = ReplicantAdminRequestMessage::bootstrap_request();

        assert_eq!(encoded, CAPTURED_INITIAL_CONFIG_FOLLOW_REQUEST);

        let frames = BusyBeeFrame::decode_stream(&encoded).unwrap();
        assert_eq!(
            frames,
            vec![
                BusyBeeFrame::identify(vec![0_u8; 16]),
                BusyBeeFrame::new(vec![0x1c])
            ]
        );
        assert_eq!(BusyBeeFrame::encode_stream(&frames).unwrap(), encoded);
    }

    #[test]
    fn wait_until_stable_message_round_trip() {
        let message = ReplicantAdminRequestMessage::wait_until_stable(7, 11);
        let encoded = message.encode().unwrap();
        let decoded = ReplicantAdminRequestMessage::decode(&encoded).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn space_rm_message_round_trip() {
        let message = ReplicantAdminRequestMessage::space_rm(9, "profiles".to_owned());
        let encoded = message.encode().unwrap();
        let decoded = ReplicantAdminRequestMessage::decode(&encoded).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn packed_space_decoder_translates_original_hyperdex_layout() {
        let encoded_space = encode_test_space_payload();

        let decoded = decode_packed_hyperdex_space(&encoded_space).unwrap();

        assert_eq!(
            decoded,
            Space {
                name: "profiles".to_owned(),
                key_attribute: "username".to_owned(),
                attributes: vec![
                    AttributeDefinition {
                        name: "first".to_owned(),
                        kind: ValueKind::String,
                    },
                    AttributeDefinition {
                        name: "profile_views".to_owned(),
                        kind: ValueKind::Int,
                    },
                ],
                subspaces: vec![Subspace {
                    dimensions: vec!["profile_views".to_owned()],
                }],
                options: SpaceOptions {
                    fault_tolerance: 2,
                    partitions: 4,
                    schema_format: SchemaFormat::HyperDexDsl,
                },
            }
        );
    }

    #[test]
    fn space_add_request_maps_through_packed_space_decoder() {
        let request =
            ReplicantAdminRequestMessage::space_add(41, encode_test_space_payload());

        let mapped = request.into_coordinator_request().unwrap();

        assert_eq!(
            mapped,
            CoordinatorAdminRequest::SpaceAdd(Space {
                name: "profiles".to_owned(),
                key_attribute: "username".to_owned(),
                attributes: vec![
                    AttributeDefinition {
                        name: "first".to_owned(),
                        kind: ValueKind::String,
                    },
                    AttributeDefinition {
                        name: "profile_views".to_owned(),
                        kind: ValueKind::Int,
                    },
                ],
                subspaces: vec![Subspace {
                    dimensions: vec!["profile_views".to_owned()],
                }],
                options: SpaceOptions {
                    fault_tolerance: 2,
                    partitions: 4,
                    schema_format: SchemaFormat::HyperDexDsl,
                },
            })
        );
    }

    #[test]
    fn stable_wait_and_space_rm_map_to_coordinator_requests() {
        assert_eq!(
            ReplicantAdminRequestMessage::wait_until_stable(7, 11)
                .into_coordinator_request()
                .unwrap(),
            CoordinatorAdminRequest::WaitUntilStable
        );
        assert_eq!(
            ReplicantAdminRequestMessage::space_rm(9, "profiles".to_owned())
                .into_coordinator_request()
                .unwrap(),
            CoordinatorAdminRequest::SpaceRm("profiles".to_owned())
        );
    }

    #[test]
    fn call_completion_response_decodes() {
        let response = ReplicantCallCompletion {
            nonce: 14,
            status: ReplicantReturnCode::Success,
            output: CoordinatorReturnCode::Success.encode().to_vec(),
        };
        let encoded = response.encode();

        assert_eq!(ReplicantCallCompletion::decode(&encoded).unwrap(), response);
    }

    #[test]
    fn cond_wait_completion_response_decodes() {
        let response = ReplicantConditionCompletion {
            nonce: 18,
            status: ReplicantReturnCode::Success,
            state: 4,
            data: vec![0xde, 0xad, 0xbe, 0xef],
        };
        let encoded = response.encode();

        assert_eq!(
            ReplicantConditionCompletion::decode(&encoded).unwrap(),
            response
        );
    }

    #[test]
    fn coordinator_return_codes_round_trip_through_wire_bytes() {
        let codes = [
            CoordinatorReturnCode::Success,
            CoordinatorReturnCode::Malformed,
            CoordinatorReturnCode::Duplicate,
            CoordinatorReturnCode::NotFound,
            CoordinatorReturnCode::Uninitialized,
            CoordinatorReturnCode::NoCanDo,
        ];

        for code in codes {
            assert_eq!(CoordinatorReturnCode::decode(&code.encode()).unwrap(), code);
        }
    }

    #[test]
    fn coordinator_return_codes_map_to_hyperdex_admin_statuses() {
        assert_eq!(
            CoordinatorReturnCode::Success.legacy_admin_status(),
            LegacyAdminReturnCode::Success
        );
        assert_eq!(
            CoordinatorReturnCode::Duplicate.legacy_admin_status(),
            LegacyAdminReturnCode::Duplicate
        );
        assert_eq!(
            CoordinatorReturnCode::NotFound.legacy_admin_status(),
            LegacyAdminReturnCode::NotFound
        );
        assert_eq!(
            CoordinatorReturnCode::Uninitialized.legacy_admin_status(),
            LegacyAdminReturnCode::CoordFail
        );
        assert_eq!(
            CoordinatorReturnCode::NoCanDo.legacy_admin_status(),
            LegacyAdminReturnCode::CoordFail
        );
        assert_eq!(
            CoordinatorReturnCode::Malformed.legacy_admin_status(),
            LegacyAdminReturnCode::Internal
        );
    }

    #[test]
    fn coordinator_admin_requests_expose_hyperdex_method_names() {
        let space = Space {
            name: "profiles".to_owned(),
            key_attribute: "username".to_owned(),
            attributes: vec![AttributeDefinition {
                name: "first".to_owned(),
                kind: ValueKind::String,
            }],
            subspaces: vec![Subspace {
                dimensions: vec!["username".to_owned()],
            }],
            options: SpaceOptions {
                fault_tolerance: 0,
                partitions: 64,
                schema_format: SchemaFormat::HyperDexDsl,
            },
        };

        assert_eq!(
            CoordinatorAdminRequest::DaemonRegister(ClusterNode {
                id: 9,
                host: "127.0.0.1".to_owned(),
                control_port: 1982,
                data_port: 2012,
            })
            .method_name(),
            "daemon_register"
        );
        assert_eq!(
            CoordinatorAdminRequest::SpaceAdd(space).method_name(),
            "space_add"
        );
        assert_eq!(
            CoordinatorAdminRequest::SpaceRm("profiles".to_owned()).method_name(),
            "space_rm"
        );
        assert_eq!(
            CoordinatorAdminRequest::WaitUntilStable.method_name(),
            "wait_until_stable"
        );
        assert_eq!(
            CoordinatorAdminRequest::ConfigGet.method_name(),
            "config_get"
        );
    }

    fn encode_test_space_payload() -> Vec<u8> {
        let mut out = Vec::new();
        encode_u64(&mut out, 0);
        encode_slice32(&mut out, b"profiles");
        encode_u64(&mut out, 2);
        encode_u16(&mut out, 3);
        encode_u16(&mut out, 2);
        encode_u16(&mut out, 1);

        encode_slice32(&mut out, b"username");
        encode_u16(&mut out, HYPERDATATYPE_STRING);
        encode_slice32(&mut out, b"first");
        encode_u16(&mut out, HYPERDATATYPE_STRING);
        encode_slice32(&mut out, b"profile_views");
        encode_u16(&mut out, HYPERDATATYPE_INT64);

        encode_subspace(&mut out, 0, &[0], 4);
        encode_subspace(&mut out, 1, &[2], 4);

        encode_u8(&mut out, 0);
        encode_u64(&mut out, 0);
        encode_u16(&mut out, 2);
        encode_slice32(&mut out, b"");

        out
    }

    fn encode_subspace(out: &mut Vec<u8>, id: u64, attrs: &[u16], partitions: u32) {
        encode_u64(out, id);
        encode_u16(out, attrs.len() as u16);
        encode_u32(out, partitions);
        for attr in attrs {
            encode_u16(out, *attr);
        }
        for partition in 0..partitions {
            encode_u64(out, partition as u64);
            encode_u16(out, 1);
            encode_u8(out, 0);
            encode_u64(out, partition as u64);
            encode_u64(out, partition as u64);
        }
    }

    fn encode_slice32(out: &mut Vec<u8>, bytes: &[u8]) {
        encode_u32(out, bytes.len() as u32);
        out.extend_from_slice(bytes);
    }

    fn encode_u64(out: &mut Vec<u8>, value: u64) {
        out.extend_from_slice(&value.to_be_bytes());
    }

    fn encode_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_be_bytes());
    }

    fn encode_u16(out: &mut Vec<u8>, value: u16) {
        out.extend_from_slice(&value.to_be_bytes());
    }

    fn encode_u8(out: &mut Vec<u8>, value: u8) {
        out.push(value);
    }
}

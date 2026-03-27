use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use cluster_config::{ClusterConfig, ClusterNode};
use data_model::{SchemaFormat, Space, SpaceName, SpaceOptions, Subspace, ValueKind};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

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
const HYPERDEX_ATTRIBUTE_SECRET: &str = "__secret";
const HYPERDATATYPE_STRING: u16 = 9217;
const HYPERDATATYPE_INT64: u16 = 9218;
const HYPERDATATYPE_FLOAT: u16 = 9219;
const HYPERDATATYPE_DOCUMENT: u16 = 9223;
const HYPERDATATYPE_LIST_GENERIC: u16 = 9280;
const HYPERDATATYPE_SET_GENERIC: u16 = 9344;
const HYPERDATATYPE_MAP_GENERIC: u16 = 9408;
const HYPERDATATYPE_TIMESTAMP_GENERIC: u16 = 9472;
const HYPERDATATYPE_MACAROON_SECRET: u16 = 9664;
const INDEX_TYPE_NORMAL: u8 = 0;
const INDEX_TYPE_DOCUMENT: u8 = 1;
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
    GetRobustParams {
        nonce: u64,
    },
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicantBootstrapServer {
    pub id: u64,
    pub address: SocketAddr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicantBootstrapConfiguration {
    pub cluster_id: u64,
    pub version: u64,
    pub first_slot: u64,
    pub servers: Vec<ReplicantBootstrapServer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicantBootstrapResponse {
    pub server: ReplicantBootstrapServer,
    pub configuration: ReplicantBootstrapConfiguration,
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

struct PackedSpaceDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
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

    pub fn config_follow() -> Vec<u8> {
        Self::bootstrap_request()
    }

    pub fn wait_until_stable(nonce: u64, state: u64) -> Self {
        Self::CondWait {
            nonce,
            object: REPLICANT_OBJECT_HYPERDEX.to_vec(),
            condition: REPLICANT_CONDITION_STABLE.to_vec(),
            state,
        }
    }

    pub fn get_robust_params(nonce: u64) -> Self {
        Self::GetRobustParams { nonce }
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
            Self::GetRobustParams { nonce } => {
                out.push(ReplicantNetworkMsgtype::GetRobustParams.encode());
                out.extend_from_slice(&nonce.to_be_bytes());
            }
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
            ReplicantNetworkMsgtype::GetRobustParams => {
                let nonce = decode_u64_be(bytes, &mut cursor)?;
                expect_consumed(bytes, cursor, "replicant get_robust_params request")?;

                Ok(Self::GetRobustParams { nonce })
            }
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
            Self::GetRobustParams { nonce }
            | Self::CondWait { nonce, .. }
            | Self::Call { nonce, .. }
            | Self::CallRobust { nonce, .. } => *nonce,
        }
    }

    pub fn into_coordinator_request(self) -> Result<CoordinatorAdminRequest> {
        match self {
            Self::GetRobustParams { .. } => Err(anyhow!(
                "get_robust_params is transport machinery, not a coordinator admin request"
            )),
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

impl ReplicantBootstrapServer {
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.id.to_be_bytes());
        encode_socket_addr(out, self.address);
    }

    fn decode(bytes: &[u8], cursor: &mut usize) -> Result<Self> {
        Ok(Self {
            id: decode_u64_be(bytes, cursor)?,
            address: decode_socket_addr(bytes, cursor)?,
        })
    }
}

impl ReplicantBootstrapConfiguration {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(&mut out);
        out
    }

    fn encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.cluster_id.to_be_bytes());
        out.extend_from_slice(&self.version.to_be_bytes());
        out.extend_from_slice(&self.first_slot.to_be_bytes());
        out.extend_from_slice(&encode_varint_u64(self.servers.len() as u64));

        for server in &self.servers {
            server.encode(out);
        }
    }

    fn decode(bytes: &[u8], cursor: &mut usize) -> Result<Self> {
        let cluster_id = decode_u64_be(bytes, cursor)?;
        let version = decode_u64_be(bytes, cursor)?;
        let first_slot = decode_u64_be(bytes, cursor)?;
        let servers_len = decode_varint_u64_at(bytes, cursor)?;
        let servers_len = usize::try_from(servers_len)
            .map_err(|_| anyhow!("bootstrap server list length does not fit usize"))?;
        let mut servers = Vec::with_capacity(servers_len);

        for _ in 0..servers_len {
            servers.push(ReplicantBootstrapServer::decode(bytes, cursor)?);
        }

        Ok(Self {
            cluster_id,
            version,
            first_slot,
            servers,
        })
    }
}

impl ReplicantBootstrapResponse {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(ReplicantNetworkMsgtype::Bootstrap.encode());
        self.server.encode(&mut out);
        self.configuration.encode_into(&mut out);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            bail!("replicant bootstrap response is empty");
        }

        let msgtype = ReplicantNetworkMsgtype::decode(bytes[0])?;

        if msgtype != ReplicantNetworkMsgtype::Bootstrap {
            bail!("replicant bootstrap response must start with BOOTSTRAP");
        }

        let mut cursor = 1;
        let server = ReplicantBootstrapServer::decode(bytes, &mut cursor)?;
        let configuration = ReplicantBootstrapConfiguration::decode(bytes, &mut cursor)?;
        expect_consumed(bytes, cursor, "replicant bootstrap response")?;

        Ok(Self {
            server,
            configuration,
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

fn decode_varint_u64_at(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let (value, consumed) = decode_varint_u64(&bytes[*cursor..])?;
    *cursor = cursor
        .checked_add(consumed)
        .ok_or_else(|| anyhow!("varint cursor overflow"))?;
    Ok(value)
}

fn encode_socket_addr(out: &mut Vec<u8>, address: SocketAddr) {
    match address {
        SocketAddr::V4(address) => {
            out.push(4);
            out.extend_from_slice(&address.ip().octets());
            out.extend_from_slice(&address.port().to_be_bytes());
        }
        SocketAddr::V6(address) => {
            out.push(6);
            out.extend_from_slice(&address.ip().octets());
            out.extend_from_slice(&address.port().to_be_bytes());
        }
    }
}

fn decode_socket_addr(bytes: &[u8], cursor: &mut usize) -> Result<SocketAddr> {
    if bytes.len() <= *cursor {
        bail!("buffer too short for a socket address family");
    }

    let family = bytes[*cursor];
    *cursor += 1;

    match family {
        4 => {
            let end = cursor
                .checked_add(4)
                .ok_or_else(|| anyhow!("ipv4 cursor overflow"))?;
            if bytes.len() < end + 2 {
                bail!("buffer too short for an ipv4 socket address");
            }
            let address = Ipv4Addr::from(<[u8; 4]>::try_from(&bytes[*cursor..end]).unwrap());
            *cursor = end;
            let port = u16::from_be_bytes(bytes[*cursor..*cursor + 2].try_into().unwrap());
            *cursor += 2;
            Ok(SocketAddr::new(IpAddr::V4(address), port))
        }
        6 => {
            let end = cursor
                .checked_add(16)
                .ok_or_else(|| anyhow!("ipv6 cursor overflow"))?;
            if bytes.len() < end + 2 {
                bail!("buffer too short for an ipv6 socket address");
            }
            let address = Ipv6Addr::from(<[u8; 16]>::try_from(&bytes[*cursor..end]).unwrap());
            *cursor = end;
            let port = u16::from_be_bytes(bytes[*cursor..*cursor + 2].try_into().unwrap());
            *cursor += 2;
            Ok(SocketAddr::new(IpAddr::V6(address), port))
        }
        0 => bail!("unspecified bootstrap socket addresses are not supported"),
        other => bail!("unknown bootstrap socket address family {other}"),
    }
}

pub fn decode_packed_hyperdex_space(bytes: &[u8]) -> Result<Space> {
    let mut decoder = PackedSpaceDecoder::new(bytes);
    let _space_id = decoder.read_u64("space id")?;
    let name = decoder.read_string("space name")?;
    let fault_tolerance = decoder.read_u64("fault tolerance")?;
    let attrs_len = decoder.read_u16("attribute count")? as usize;
    let subspaces_len = decoder.read_u16("subspace count")? as usize;
    let indices_len = decoder.read_u16("index count")? as usize;

    let mut attrs = Vec::with_capacity(attrs_len);
    for _ in 0..attrs_len {
        let attr_name = decoder.read_string("attribute name")?;
        let datatype = decoder.read_u16("attribute datatype")?;

        if datatype == HYPERDATATYPE_MACAROON_SECRET && attr_name != HYPERDEX_ATTRIBUTE_SECRET {
            bail!(
                "packed hyperdex::space uses authorization attribute name `{attr_name}`, expected `{HYPERDEX_ATTRIBUTE_SECRET}`"
            );
        }

        attrs.push(PackedSpaceAttribute {
            name: attr_name,
            datatype,
        });
    }

    if attrs.is_empty() {
        bail!("packed hyperdex::space did not include a key attribute");
    }

    if attrs[0].datatype == HYPERDATATYPE_MACAROON_SECRET {
        bail!("packed hyperdex::space key attribute cannot be the authorization secret");
    }

    let mut subspaces = Vec::with_capacity(subspaces_len);
    let mut partitions = None;
    for _ in 0..subspaces_len {
        subspaces.push(decode_packed_subspace(
            &mut decoder,
            attrs.len(),
            &mut partitions,
        )?);
    }

    for _ in 0..indices_len {
        decode_packed_index(&mut decoder, attrs.len())?;
    }

    decoder.finish("packed hyperdex::space")?;

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
            let attr = &attrs[*attr_index as usize];
            if attr.name == key_attribute || attr.datatype == HYPERDATATYPE_MACAROON_SECRET {
                continue;
            }
            dimensions.push(attr.name.clone());
        }
        if !dimensions.is_empty() {
            rust_subspaces.push(Subspace { dimensions });
        }
    }

    let partitions = partitions
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

fn decode_packed_subspace(
    decoder: &mut PackedSpaceDecoder<'_>,
    attribute_count: usize,
    partitions: &mut Option<u32>,
) -> Result<PackedSpaceSubspace> {
    let _subspace_id = decoder.read_u64("subspace id")?;
    let attrs_len = decoder.read_u16("subspace attribute count")? as usize;
    let regions_len = decoder.read_u32("subspace region count")?;
    let mut attrs = Vec::with_capacity(attrs_len);

    if let Some(existing) = *partitions {
        if existing != regions_len {
            bail!(
                "packed hyperdex::space subspaces disagree on partition count: {existing} vs {regions_len}"
            );
        }
    } else {
        *partitions = Some(regions_len);
    }

    for _ in 0..attrs_len {
        let attr = decoder.read_u16("subspace attribute index")?;
        if attr as usize >= attribute_count {
            bail!(
                "packed hyperdex::space subspace references attribute index {attr}, but only {attribute_count} attributes were decoded"
            );
        }
        attrs.push(attr);
    }

    for _ in 0..regions_len as usize {
        decode_packed_region(decoder)?;
    }

    Ok(PackedSpaceSubspace {
        attrs,
        regions_len: regions_len as usize,
    })
}

fn decode_packed_region(decoder: &mut PackedSpaceDecoder<'_>) -> Result<()> {
    let _region_id = decoder.read_u64("region id")?;
    let hashes_len = decoder.read_u16("region hash count")? as usize;
    let replicas_len = decoder.read_u8("region replica count")? as usize;

    let coord_bytes = hashes_len
        .checked_mul(16)
        .ok_or_else(|| anyhow!("region coordinate byte count overflow in packed hyperdex::space"))?;
    decoder.read_exact(coord_bytes, "region coordinates")?;

    let replica_bytes = replicas_len
        .checked_mul(16)
        .ok_or_else(|| anyhow!("region replica byte count overflow in packed hyperdex::space"))?;
    decoder.read_exact(replica_bytes, "region replicas")?;

    Ok(())
}

fn decode_packed_index(decoder: &mut PackedSpaceDecoder<'_>, attribute_count: usize) -> Result<()> {
    let index_type = decoder.read_u8("index type")?;
    match index_type {
        INDEX_TYPE_NORMAL | INDEX_TYPE_DOCUMENT => {}
        other => bail!("packed hyperdex::space uses unknown index type {other}"),
    }

    let _index_id = decoder.read_u64("index id")?;
    let attr = decoder.read_u16("index attribute")?;
    if attr as usize >= attribute_count {
        bail!(
            "packed hyperdex::space index references attribute index {attr}, but only {attribute_count} attributes were decoded"
        );
    }
    let _extra = decoder.read_slice("index extra payload")?;
    Ok(())
}

impl<'a> PackedSpaceDecoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn read_u8(&mut self, label: &str) -> Result<u8> {
        Ok(self.read_exact(1, label)?[0])
    }

    fn read_u16(&mut self, label: &str) -> Result<u16> {
        Ok(u16::from_be_bytes(self.read_exact(2, label)?.try_into().unwrap()))
    }

    fn read_u32(&mut self, label: &str) -> Result<u32> {
        Ok(u32::from_be_bytes(self.read_exact(4, label)?.try_into().unwrap()))
    }

    fn read_u64(&mut self, label: &str) -> Result<u64> {
        Ok(u64::from_be_bytes(self.read_exact(8, label)?.try_into().unwrap()))
    }

    fn read_slice(&mut self, label: &str) -> Result<&'a [u8]> {
        let len = self.read_varint(label)?;
        self.read_exact(len, label)
    }

    fn read_string(&mut self, label: &str) -> Result<String> {
        let slice = self.read_slice(label)?;
        let text = std::str::from_utf8(slice)
            .map_err(|_| anyhow!("{label} is not valid UTF-8 in packed hyperdex::space"))?;
        Ok(text.to_owned())
    }

    fn read_varint(&mut self, label: &str) -> Result<usize> {
        let len = decode_varint_u64_at(self.bytes, &mut self.cursor)
            .map_err(|err| anyhow!("{label} length varint is invalid in packed hyperdex::space: {err}"))?;
        usize::try_from(len).map_err(|_| anyhow!("{label} length does not fit usize in packed hyperdex::space"))
    }

    fn read_exact(&mut self, len: usize, label: &str) -> Result<&'a [u8]> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or_else(|| anyhow!("{label} length overflow in packed hyperdex::space"))?;

        if end > self.bytes.len() {
            bail!(
                "{label} is truncated in packed hyperdex::space: need {len} bytes but only {} remain",
                self.bytes.len().saturating_sub(self.cursor)
            );
        }

        let slice = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(slice)
    }

    fn finish(&self, context: &str) -> Result<()> {
        expect_consumed(self.bytes, self.cursor, context)
    }
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
    use data_model::{AttributeDefinition, SchemaFormat, SpaceOptions, Subspace, TimeUnit, ValueKind};

    #[derive(Clone, Debug)]
    struct PackedAttribute<'a> {
        name: &'a str,
        datatype: u16,
    }

    #[derive(Clone, Debug)]
    struct PackedReplica {
        server_id: u64,
        virtual_server_id: u64,
    }

    #[derive(Clone, Debug)]
    struct PackedRegion {
        id: u64,
        bounds: Vec<(u64, u64)>,
        replicas: Vec<PackedReplica>,
    }

    #[derive(Clone, Debug)]
    struct PackedSubspace {
        id: u64,
        attrs: Vec<u16>,
        regions: Vec<PackedRegion>,
    }

    #[derive(Clone, Debug)]
    struct PackedIndex<'a> {
        index_type: u8,
        id: u64,
        attr: u16,
        extra: &'a [u8],
    }

    #[derive(Clone, Debug)]
    struct PackedSpace<'a> {
        id: u64,
        name: &'a str,
        fault_tolerance: u64,
        attributes: Vec<PackedAttribute<'a>>,
        subspaces: Vec<PackedSubspace>,
        indices: Vec<PackedIndex<'a>>,
    }

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
    fn get_robust_params_message_round_trip() {
        let message = ReplicantAdminRequestMessage::get_robust_params(13);
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
                    AttributeDefinition {
                        name: "upvotes".to_owned(),
                        kind: ValueKind::Map {
                            key: Box::new(ValueKind::String),
                            value: Box::new(ValueKind::Int),
                        },
                    },
                    AttributeDefinition {
                        name: "created".to_owned(),
                        kind: ValueKind::Timestamp(TimeUnit::Day),
                    },
                ],
                subspaces: vec![Subspace {
                    dimensions: vec!["profile_views".to_owned(), "upvotes".to_owned()],
                }],
                options: SpaceOptions {
                    fault_tolerance: 2,
                    partitions: 2,
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
                    AttributeDefinition {
                        name: "upvotes".to_owned(),
                        kind: ValueKind::Map {
                            key: Box::new(ValueKind::String),
                            value: Box::new(ValueKind::Int),
                        },
                    },
                    AttributeDefinition {
                        name: "created".to_owned(),
                        kind: ValueKind::Timestamp(TimeUnit::Day),
                    },
                ],
                subspaces: vec![Subspace {
                    dimensions: vec!["profile_views".to_owned(), "upvotes".to_owned()],
                }],
                options: SpaceOptions {
                    fault_tolerance: 2,
                    partitions: 2,
                    schema_format: SchemaFormat::HyperDexDsl,
                },
            })
        );
    }

    #[test]
    fn packed_space_decoder_rejects_secret_attribute_with_wrong_name() {
        let encoded_space = pack_space(PackedSpace {
            id: 1,
            name: "profiles",
            fault_tolerance: 0,
            attributes: vec![
                PackedAttribute {
                    name: "username",
                    datatype: HYPERDATATYPE_STRING,
                },
                PackedAttribute {
                    name: "api_secret",
                    datatype: HYPERDATATYPE_MACAROON_SECRET,
                },
            ],
            subspaces: vec![packed_primary_subspace(&[0], 1)],
            indices: vec![],
        });

        let err = decode_packed_hyperdex_space(&encoded_space).unwrap_err().to_string();

        assert!(err.contains("authorization attribute name `api_secret`, expected `__secret`"));
    }

    #[test]
    fn packed_space_decoder_rejects_secret_key_attribute() {
        let encoded_space = pack_space(PackedSpace {
            id: 1,
            name: "profiles",
            fault_tolerance: 0,
            attributes: vec![PackedAttribute {
                name: HYPERDEX_ATTRIBUTE_SECRET,
                datatype: HYPERDATATYPE_MACAROON_SECRET,
            }],
            subspaces: vec![packed_primary_subspace(&[0], 1)],
            indices: vec![],
        });

        let err = decode_packed_hyperdex_space(&encoded_space).unwrap_err().to_string();

        assert!(err.contains("key attribute cannot be the authorization secret"));
    }

    #[test]
    fn packed_space_decoder_rejects_inconsistent_partition_counts() {
        let encoded_space = pack_space(PackedSpace {
            id: 1,
            name: "profiles",
            fault_tolerance: 0,
            attributes: vec![
                PackedAttribute {
                    name: "username",
                    datatype: HYPERDATATYPE_STRING,
                },
                PackedAttribute {
                    name: "profile_views",
                    datatype: HYPERDATATYPE_INT64,
                },
            ],
            subspaces: vec![
                packed_primary_subspace(&[0], 2),
                packed_secondary_subspace(2, &[1], 3),
            ],
            indices: vec![],
        });

        let err = decode_packed_hyperdex_space(&encoded_space).unwrap_err().to_string();

        assert!(err.contains("subspaces disagree on partition count: 2 vs 3"));
    }

    #[test]
    fn packed_space_decoder_rejects_unknown_index_type() {
        let mut packed = test_packed_space();
        packed.indices[0].index_type = 9;

        let err = decode_packed_hyperdex_space(&pack_space(packed))
            .unwrap_err()
            .to_string();

        assert!(err.contains("uses unknown index type 9"));
    }

    #[test]
    fn packed_space_decoder_rejects_index_attribute_out_of_range() {
        let mut packed = test_packed_space();
        packed.indices[0].attr = 99;

        let err = decode_packed_hyperdex_space(&pack_space(packed))
            .unwrap_err()
            .to_string();

        assert!(err.contains("index references attribute index 99, but only 6 attributes were decoded"));
    }

    #[test]
    fn packed_space_decoder_reports_truncated_region_replicas() {
        let mut packed = test_packed_space();
        packed.indices.clear();
        let mut encoded_space = pack_space(packed);
        encoded_space.pop();

        let err = decode_packed_hyperdex_space(&encoded_space)
            .unwrap_err()
            .to_string();

        assert!(err.contains("region replicas is truncated"));
    }

    #[test]
    fn packed_space_decoder_reports_truncated_index_extra_payload() {
        let mut packed = test_packed_space();
        packed.indices[1].extra = b"comments.author";
        let mut encoded_space = pack_space(packed);
        encoded_space.pop();

        let err = decode_packed_hyperdex_space(&encoded_space)
            .unwrap_err()
            .to_string();

        assert!(err.contains("index extra payload is truncated"));
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
    fn bootstrap_response_round_trips() {
        let response = ReplicantBootstrapResponse {
            server: ReplicantBootstrapServer {
                id: 1,
                address: "127.0.0.1:1982".parse().unwrap(),
            },
            configuration: ReplicantBootstrapConfiguration {
                cluster_id: 1,
                version: 1,
                first_slot: 1,
                servers: vec![ReplicantBootstrapServer {
                    id: 1,
                    address: "127.0.0.1:1982".parse().unwrap(),
                }],
            },
        };
        let encoded = response.encode();

        assert_eq!(ReplicantBootstrapResponse::decode(&encoded).unwrap(), response);
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
        pack_space(test_packed_space())
    }

    fn test_packed_space<'a>() -> PackedSpace<'a> {
        PackedSpace {
            id: 17,
            name: "profiles",
            fault_tolerance: 2,
            attributes: vec![
                PackedAttribute {
                    name: "username",
                    datatype: HYPERDATATYPE_STRING,
                },
                PackedAttribute {
                    name: "first",
                    datatype: HYPERDATATYPE_STRING,
                },
                PackedAttribute {
                    name: "profile_views",
                    datatype: HYPERDATATYPE_INT64,
                },
                PackedAttribute {
                    name: "upvotes",
                    datatype: 9418,
                },
                PackedAttribute {
                    name: "created",
                    datatype: 9476,
                },
                PackedAttribute {
                    name: HYPERDEX_ATTRIBUTE_SECRET,
                    datatype: HYPERDATATYPE_MACAROON_SECRET,
                },
            ],
            subspaces: vec![
                packed_primary_subspace(&[0], 2),
                packed_secondary_subspace(32, &[2, 3], 2),
            ],
            indices: vec![
                PackedIndex {
                    index_type: INDEX_TYPE_NORMAL,
                    id: 41,
                    attr: 2,
                    extra: b"",
                },
                PackedIndex {
                    index_type: INDEX_TYPE_DOCUMENT,
                    id: 42,
                    attr: 3,
                    extra: b"comments.author",
                },
            ],
        }
    }

    fn packed_primary_subspace(attrs: &[u16], partitions: u32) -> PackedSubspace {
        packed_secondary_subspace(31, attrs, partitions)
    }

    fn packed_secondary_subspace(id: u64, attrs: &[u16], partitions: u32) -> PackedSubspace {
        let bounds = vec![(0_u64, 100_u64); attrs.len().max(1)];
        let mut regions = Vec::new();
        for partition in 0..partitions {
            let base = partition as u64 + 1;
            regions.push(PackedRegion {
                id: id * 10 + partition as u64,
                bounds: bounds.clone(),
                replicas: vec![PackedReplica {
                    server_id: base,
                    virtual_server_id: base * 10,
                }],
            });
        }
        PackedSubspace {
            id,
            attrs: attrs.to_vec(),
            regions,
        }
    }

    fn pack_space(space: PackedSpace<'_>) -> Vec<u8> {
        let mut out = Vec::new();
        encode_u64(&mut out, space.id);
        encode_slice(&mut out, space.name.as_bytes());
        encode_u64(&mut out, space.fault_tolerance);
        encode_u16(&mut out, space.attributes.len() as u16);
        encode_u16(&mut out, space.subspaces.len() as u16);
        encode_u16(&mut out, space.indices.len() as u16);

        for attribute in space.attributes {
            encode_slice(&mut out, attribute.name.as_bytes());
            encode_u16(&mut out, attribute.datatype);
        }

        for subspace in space.subspaces {
            encode_u64(&mut out, subspace.id);
            encode_u16(&mut out, subspace.attrs.len() as u16);
            encode_u32(&mut out, subspace.regions.len() as u32);

            for attr in subspace.attrs {
                encode_u16(&mut out, attr);
            }

            for region in subspace.regions {
                encode_u64(&mut out, region.id);
                encode_u16(&mut out, region.bounds.len() as u16);
                encode_u8(&mut out, region.replicas.len() as u8);

                for (lower, upper) in region.bounds {
                    encode_u64(&mut out, lower);
                    encode_u64(&mut out, upper);
                }

                for replica in region.replicas {
                    encode_u64(&mut out, replica.server_id);
                    encode_u64(&mut out, replica.virtual_server_id);
                }
            }
        }

        for index in space.indices {
            encode_u8(&mut out, index.index_type);
            encode_u64(&mut out, index.id);
            encode_u16(&mut out, index.attr);
            encode_slice(&mut out, index.extra);
        }

        out
    }

    fn encode_slice(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&encode_varint_slice(bytes));
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

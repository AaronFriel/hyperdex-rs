use std::fs::{self, File};
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use cluster_config::{ClusterConfig, ClusterNode, TransportBackend};
use data_model::parse_hyperdex_space;
use hyperdex_admin_protocol::{
    decode_packed_hyperdex_space, BusyBeeFrame, ConfigView, CoordinatorAdminRequest,
    CoordinatorReturnCode, ReplicantAdminRequestMessage, ReplicantCallCompletion,
    ReplicantNetworkMsgtype,
};
use legacy_frontend::request_once;
use legacy_protocol::{
    AtomicRequest, AtomicResponse, CountRequest, CountResponse, GetAttribute, GetRequest,
    GetResponse, GetValue, LegacyCheck, LegacyFuncall, LegacyFuncallName, LegacyMessageType,
    LegacyPredicate, LegacyReturnCode, RequestHeader, ResponseHeader, SearchContinueRequest,
    SearchDoneResponse, SearchItemResponse, SearchStartRequest, BUSYBEE_HEADER_SIZE,
    LEGACY_ATOMIC_FLAG_WRITE, LEGACY_REQUEST_HEADER_SIZE, LEGACY_RESPONSE_HEADER_SIZE,
};
use serial_test::serial;
use server::{
    request_coordinator_control_once, request_coordinator_control_with_body_once, ClusterRuntime,
};
use tempfile::TempDir;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::sleep;
use transport_core::{
    DataPlaneRequest, DataPlaneResponse, InternodeRequest, InternodeResponse, DATA_PLANE_METHOD,
};

pub mod grpc_api {
    pub mod v1 {
        tonic::include_proto!("hyperdex.v1");
    }
}

static MULTIPROCESS_HARNESS_LOCK: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

struct ChildProcess {
    name: &'static str,
    child: Child,
    log_path: PathBuf,
}

struct ReservedPort {
    port: u16,
    listener: Option<std::net::TcpListener>,
}

struct SingleDaemonCluster {
    _tempdir: TempDir,
    _coordinator: ChildProcess,
    _daemon: ChildProcess,
    coordinator_address: SocketAddr,
}

#[derive(Clone, Debug, Default)]
struct BusyBeeCapture {
    client_frames: Vec<BusyBeeFrame>,
    server_frames: Vec<BusyBeeFrame>,
}

#[derive(Clone, Debug)]
struct LegacyAdminProbeResult {
    capture: BusyBeeCapture,
    tool_exit: Option<std::process::ExitStatus>,
    stdout: String,
    stderr: String,
}

#[derive(Clone, Debug, Default)]
struct LegacyCapture {
    events: Vec<LegacyFrameEvent>,
}

#[derive(Clone, Debug)]
struct LegacyFrameEvent {
    connection_id: usize,
    direction: LegacyFrameDirection,
    raw_prefix: String,
    summary: String,
}

#[derive(Clone, Copy, Debug)]
enum LegacyFrameDirection {
    ClientToDaemon,
    DaemonToClient,
}

const MINIMAL_PROFILES_SPACE_DESC: &str = "space profiles\n\
    key username\n\
    attributes\n\
        string first,\n\
        int profile_views\n\
    subspace first\n\
    create 8 partitions\n\
    tolerate 0 failures\n";

const FULL_PROFILES_SPACE_DESC: &str = "space profiles                         \n\
key username                             \n\
attributes                               \n\
   string first,                         \n\
   string last,                          \n\
   float score,                          \n\
   int profile_views,                    \n\
   list(string) pending_requests,        \n\
   list(float) rankings,                 \n\
   list(int) todolist,                   \n\
   set(string) hobbies,                  \n\
   set(float) imonafloat,                \n\
   set(int) friendids,                   \n\
   map(string, string) unread_messages,  \n\
   map(string, int) upvotes,             \n\
   map(string, float) friendranks,       \n\
   map(int, string) posts,               \n\
   map(int, int) friendremapping,        \n\
   map(int, float) intfloatmap,          \n\
   map(float, string) still_looking,     \n\
   map(float, int) for_a_reason,         \n\
   map(float, float) for_float_keyed_map \n\
tolerate 0 failures\n";

#[derive(Debug)]
struct ClientTraceResult {
    exit_status: Option<std::process::ExitStatus>,
    stdout: String,
    stderr: String,
    trace: String,
}

impl ReservedPort {
    fn new() -> Result<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            port,
            listener: Some(listener),
        })
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn release(&mut self) {
        drop(self.listener.take());
    }
}

impl ChildProcess {
    fn spawn(name: &'static str, args: &[String], log_dir: &Path) -> Result<Self> {
        let binary = server_binary_path()?;
        let log_path = log_dir.join(format!("{name}.log"));
        let log_file = File::create(&log_path)?;
        let stderr_file = log_file.try_clone()?;
        let child = Command::new(&binary)
            .args(args)
            .env("RUST_LOG", "info")
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn {name} child process from {}",
                    binary.display()
                )
            })?;

        Ok(Self {
            name,
            child,
            log_path,
        })
    }

    async fn wait_for_coordinator(&mut self, address: SocketAddr) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match request_coordinator_control_with_body_once(
                address,
                CoordinatorAdminRequest::WaitUntilStable.method_name(),
                &CoordinatorAdminRequest::WaitUntilStable,
            )
            .await
            {
                Ok(response)
                    if CoordinatorReturnCode::decode(&response.status)
                        .is_ok_and(|status| status == CoordinatorReturnCode::Success) =>
                {
                    return Ok(());
                }
                Ok(_) => self.ensure_running()?,
                Err(err)
                    if err.downcast_ref::<io::Error>().is_some_and(|io_err| {
                        io_err.kind() == io::ErrorKind::ConnectionRefused
                            || io_err.kind() == io::ErrorKind::TimedOut
                            || io_err.kind() == io::ErrorKind::UnexpectedEof
                    }) =>
                {
                    self.ensure_running()?;
                }
                Err(err) => return Err(err),
            }

            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for {} coordinator control response\n{}",
                    self.name,
                    self.read_logs()?
                ));
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn wait_for_daemon(&mut self, address: SocketAddr) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match request_once(
                address,
                RequestHeader {
                    message_type: LegacyMessageType::ReqGetPartial,
                    flags: 0,
                    version: 7,
                    target_virtual_server: 0,
                    nonce: 0,
                },
                &[],
            )
            .await
            {
                Ok((header, _)) if header.message_type == LegacyMessageType::ConfigMismatch => {
                    return Ok(());
                }
                Ok(_) => self.ensure_running()?,
                Err(_) => {
                    self.ensure_running()?;
                }
            }

            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for {} legacy frontend response\n{}",
                    self.name,
                    self.read_logs()?
                ));
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn wait_for_config_view<F>(
        &mut self,
        address: SocketAddr,
        description: &str,
        predicate: F,
    ) -> Result<ConfigView>
    where
        F: Fn(&ConfigView) -> bool,
    {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match request_coordinator_control_with_body_once(
                address,
                CoordinatorAdminRequest::ConfigGet.method_name(),
                &CoordinatorAdminRequest::ConfigGet,
            )
            .await
            {
                Ok(response)
                    if CoordinatorReturnCode::decode(&response.status)
                        .is_ok_and(|status| status == CoordinatorReturnCode::Success) =>
                {
                    let view: ConfigView = serde_json::from_slice(&response.body)?;
                    if predicate(&view) {
                        return Ok(view);
                    }
                    self.ensure_running()?;
                }
                Ok(_) => self.ensure_running()?,
                Err(err)
                    if err.downcast_ref::<io::Error>().is_some_and(|io_err| {
                        io_err.kind() == io::ErrorKind::ConnectionRefused
                            || io_err.kind() == io::ErrorKind::TimedOut
                            || io_err.kind() == io::ErrorKind::UnexpectedEof
                    }) =>
                {
                    self.ensure_running()?;
                }
                Err(err) => return Err(err),
            }

            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for {} coordinator config: {description}\n{}",
                    self.name,
                    self.read_logs()?
                ));
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    async fn wait_for_internode_space(&mut self, address: SocketAddr, space: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match request_internode_data_plane(
                address,
                DataPlaneRequest::Get {
                    space: space.to_owned(),
                    key: b"readiness-probe".to_vec().into(),
                },
            )
            .await
            {
                Ok(DataPlaneResponse::Record(_)) => return Ok(()),
                Ok(_) => self.ensure_running()?,
                Err(_) => self.ensure_running()?,
            }

            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for {} internode data-plane readiness for space `{space}` on {address}\n{}",
                    self.name,
                    self.read_logs()?
                ));
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    fn ensure_running(&mut self) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            return Err(anyhow!(
                "{} exited early with status {status}\n{}",
                self.name,
                self.read_logs()?
            ));
        }

        Ok(())
    }

    fn read_logs(&self) -> Result<String> {
        Ok(fs::read_to_string(&self.log_path).unwrap_or_default())
    }

    fn stop(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
            let _ = self.child.wait()?;
        }
        Ok(())
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
            Err(_) => {}
        }
    }
}

fn server_binary_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_server") {
        return Ok(PathBuf::from(path));
    }

    let test_binary = std::env::current_exe()?;
    let debug_dir = test_binary
        .parent()
        .and_then(Path::parent)
        .context("could not resolve target/debug from current test binary path")?;
    let binary = debug_dir.join(format!("server{}", std::env::consts::EXE_SUFFIX));
    if binary.exists() {
        Ok(binary)
    } else {
        Err(anyhow!(
            "could not find server binary via CARGO_BIN_EXE_server or fallback path {}",
            binary.display()
        ))
    }
}

fn localhost(port: u16) -> Result<SocketAddr> {
    Ok(format!("127.0.0.1:{port}").parse()?)
}

fn legacy_hyperdex_root() -> PathBuf {
    PathBuf::from("/home/friel/c/aaronfriel/HyperDex")
}

fn legacy_tool_path(tool: &str) -> PathBuf {
    legacy_hyperdex_root().join(tool)
}

fn legacy_tool_library_path() -> String {
    let mut path = legacy_hyperdex_root().join(".libs").display().to_string();
    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        path.push(':');
        path.push_str(&existing.to_string_lossy());
    }
    path
}

fn hyhac_root() -> PathBuf {
    PathBuf::from("/home/friel/c/aaronfriel/hyhac")
}

fn hyhac_runtime_library_path() -> String {
    let mut path = hyhac_root().join(".toolchain/lib").display().to_string();
    path.push(':');
    path.push_str(&legacy_hyperdex_root().join(".libs").display().to_string());
    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        path.push(':');
        path.push_str(&existing.to_string_lossy());
    }
    path
}

fn find_hyhac_test_binary() -> Result<Option<PathBuf>> {
    let dist_root = hyhac_root().join("dist-newstyle");
    if !dist_root.is_dir() {
        return Ok(None);
    }

    let mut stack = vec![dist_root];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if path.file_name().is_some_and(|name| name == "tests")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "tests")
                && path
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "build")
            {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

fn hyhac_test_binary_path() -> Result<PathBuf> {
    if let Some(path) = find_hyhac_test_binary()? {
        return Ok(path);
    }

    let status = Command::new(hyhac_root().join("scripts/cabal.sh"))
        .arg("build")
        .arg("-f")
        .arg("tests")
        .arg("lib:hyhac")
        .arg("test:tests")
        .status()
        .context("failed to build hyhac test binary")?;
    if !status.success() {
        anyhow::bail!("hyhac test binary build failed with status {status}");
    }

    find_hyhac_test_binary()?
        .context("hyhac test binary still not present after building test:tests")
}

fn busybee_total_len(header: [u8; 4]) -> usize {
    (u32::from_be_bytes(header) & 0x00ff_ffff) as usize
}

fn legacy_total_len(header: [u8; 4]) -> usize {
    BUSYBEE_HEADER_SIZE + u32::from_be_bytes(header) as usize
}

fn drain_busybee_frames(buffer: &mut Vec<u8>) -> Result<Vec<BusyBeeFrame>> {
    let mut frames = Vec::new();

    loop {
        if buffer.len() < 4 {
            return Ok(frames);
        }

        let total_len = busybee_total_len(buffer[..4].try_into().unwrap());
        if total_len < 4 {
            return Err(anyhow!("busybee frame size {total_len} is too small"));
        }
        if buffer.len() < total_len {
            return Ok(frames);
        }

        let frame = BusyBeeFrame::decode(&buffer[..total_len])?;
        frames.push(frame);
        buffer.drain(..total_len);
    }
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe_legacy_request(header: RequestHeader, body: &[u8]) -> String {
    match header.message_type {
        LegacyMessageType::ReqAtomic => match AtomicRequest::decode_body(body) {
            Ok(request) => format!(
                "ReqAtomic flags=0x{:02x} key_len={} checks={} funcalls={} first_funcall={}",
                request.flags,
                request.key.len(),
                request.checks.len(),
                request.funcalls.len(),
                request
                    .funcalls
                    .first()
                    .map(|funcall| format!("{}::{:?}", funcall.attribute, funcall.name))
                    .unwrap_or_else(|| "<none>".to_owned()),
            ),
            Err(err) => format!("ReqAtomic decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::ReqGet => match GetRequest::decode_body(body) {
            Ok(request) => format!("ReqGet key_len={}", request.key.len()),
            Err(err) => format!("ReqGet decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::ReqCount => match CountRequest::decode_body(body) {
            Ok(request) => format!("ReqCount space={}", request.space),
            Err(err) => format!("ReqCount decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::ReqSearchStart => match SearchStartRequest::decode_body(body) {
            Ok(request) => format!(
                "ReqSearchStart space={} search_id={} checks={}",
                request.space,
                request.search_id,
                request.checks.len()
            ),
            Err(err) => format!("ReqSearchStart decode_error={err} body_len={}", body.len()),
        },
        other => format!("{other:?} body_len={}", body.len()),
    }
}

fn describe_legacy_response(header: ResponseHeader, body: &[u8]) -> String {
    match header.message_type {
        LegacyMessageType::RespAtomic => match AtomicResponse::decode_body(body) {
            Ok(response) => format!("RespAtomic status={:?}", response.status),
            Err(err) => format!("RespAtomic decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::RespGet => match GetResponse::decode_body(body) {
            Ok(response) => format!(
                "RespGet status={:?} attrs={}",
                response.status,
                response.attributes.len()
            ),
            Err(err) => format!("RespGet decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::RespCount => match CountResponse::decode_body(body) {
            Ok(response) => format!("RespCount count={}", response.count),
            Err(err) => format!("RespCount decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::RespSearchItem => match SearchItemResponse::decode_body(body) {
            Ok(response) => format!(
                "RespSearchItem search_id={} key_len={} attrs={}",
                response.search_id,
                response.key.len(),
                response.attributes.len()
            ),
            Err(err) => format!("RespSearchItem decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::RespSearchDone => match SearchDoneResponse::decode_body(body) {
            Ok(response) => format!("RespSearchDone search_id={}", response.search_id),
            Err(err) => format!("RespSearchDone decode_error={err} body_len={}", body.len()),
        },
        LegacyMessageType::ConfigMismatch => "ConfigMismatch".to_owned(),
        other => format!("{other:?} body_len={}", body.len()),
    }
}

fn drain_legacy_events(
    buffer: &mut Vec<u8>,
    connection_id: usize,
    direction: LegacyFrameDirection,
) -> Result<Vec<LegacyFrameEvent>> {
    let mut events = Vec::new();

    loop {
        if buffer.len() < BUSYBEE_HEADER_SIZE {
            return Ok(events);
        }

        let total_len = legacy_total_len(buffer[..BUSYBEE_HEADER_SIZE].try_into().unwrap());
        if total_len < BUSYBEE_HEADER_SIZE {
            return Err(anyhow!("legacy frame size {total_len} is too small"));
        }
        if buffer.len() < total_len {
            return Ok(events);
        }

        let raw = buffer[..total_len].to_vec();
        let raw_prefix = hex_prefix(&raw, 24);
        let summary = match direction {
            LegacyFrameDirection::ClientToDaemon => match RequestHeader::decode(&raw) {
                Ok(header) => {
                    let body = &raw[LEGACY_REQUEST_HEADER_SIZE..];
                    format!(
                        "request {:?} nonce={} target_vs={} {}",
                        header.message_type,
                        header.nonce,
                        header.target_virtual_server,
                        describe_legacy_request(header, body)
                    )
                }
                Err(err) => format!("request header decode_error={err} body_len={}", raw.len()),
            },
            LegacyFrameDirection::DaemonToClient => match ResponseHeader::decode(&raw) {
                Ok(header) => {
                    let body = &raw[LEGACY_RESPONSE_HEADER_SIZE..];
                    format!(
                        "response {:?} nonce={} target_vs={} {}",
                        header.message_type,
                        header.nonce,
                        header.target_virtual_server,
                        describe_legacy_response(header, body)
                    )
                }
                Err(err) => format!("response header decode_error={err} body_len={}", raw.len()),
            },
        };

        events.push(LegacyFrameEvent {
            connection_id,
            direction,
            raw_prefix,
            summary,
        });
        buffer.drain(..total_len);
    }
}

async fn proxy_copy_and_capture<R, W>(
    mut reader: R,
    mut writer: W,
    capture: Arc<Mutex<BusyBeeCapture>>,
    client_direction: bool,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut read_buf = [0_u8; 8192];
    let mut frame_buf = Vec::new();

    loop {
        let n = reader.read(&mut read_buf).await?;
        if n == 0 {
            writer.shutdown().await?;
            return Ok(());
        }

        writer.write_all(&read_buf[..n]).await?;
        frame_buf.extend_from_slice(&read_buf[..n]);
        let frames = drain_busybee_frames(&mut frame_buf)?;
        if !frames.is_empty() {
            let mut capture = capture.lock().unwrap();
            if client_direction {
                capture.client_frames.extend(frames);
            } else {
                capture.server_frames.extend(frames);
            }
        }
    }
}

async fn proxy_copy_and_capture_legacy<R, W>(
    mut reader: R,
    mut writer: W,
    capture: Arc<Mutex<LegacyCapture>>,
    connection_id: usize,
    direction: LegacyFrameDirection,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut read_buf = [0_u8; 8192];
    let mut frame_buf = Vec::new();

    loop {
        let n = reader.read(&mut read_buf).await?;
        if n == 0 {
            if !frame_buf.is_empty() {
                capture.lock().unwrap().events.push(LegacyFrameEvent {
                    connection_id,
                    direction,
                    raw_prefix: hex_prefix(&frame_buf, 24),
                    summary: format!("partial frame trailing_bytes={}", frame_buf.len()),
                });
            }
            writer.shutdown().await?;
            return Ok(());
        }

        writer.write_all(&read_buf[..n]).await?;
        frame_buf.extend_from_slice(&read_buf[..n]);
        let events = drain_legacy_events(&mut frame_buf, connection_id, direction)?;
        if !events.is_empty() {
            capture.lock().unwrap().events.extend(events);
        }
    }
}

async fn run_busybee_proxy_capture(
    listener: tokio::net::TcpListener,
    upstream_addr: SocketAddr,
    capture: Arc<Mutex<BusyBeeCapture>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    loop {
        let accept = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
        let (client_stream, _) = match accept {
            Ok(Ok(pair)) => pair,
            Ok(Err(err)) => return Err(err.into()),
            Err(_) if stop.load(Ordering::Relaxed) => return Ok(()),
            Err(_) => continue,
        };
        let upstream_stream = tokio::net::TcpStream::connect(upstream_addr).await?;

        let (client_reader, client_writer) = client_stream.into_split();
        let (upstream_reader, upstream_writer) = upstream_stream.into_split();

        let client_task = tokio::spawn(proxy_copy_and_capture(
            client_reader,
            upstream_writer,
            Arc::clone(&capture),
            true,
        ));
        let server_task = tokio::spawn(proxy_copy_and_capture(
            upstream_reader,
            client_writer,
            Arc::clone(&capture),
            false,
        ));

        let client_result = client_task.await.context("client proxy task join")?;
        let server_result = server_task.await.context("server proxy task join")?;
        client_result?;
        server_result?;
    }
}

async fn run_legacy_proxy_capture(
    listener: tokio::net::TcpListener,
    upstream_addr: SocketAddr,
    capture: Arc<Mutex<LegacyCapture>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let mut next_connection_id = 0usize;

    loop {
        let accept = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
        let (client_stream, _) = match accept {
            Ok(Ok(pair)) => pair,
            Ok(Err(err)) => return Err(err.into()),
            Err(_) if stop.load(Ordering::Relaxed) => return Ok(()),
            Err(_) => continue,
        };
        let upstream_stream = tokio::net::TcpStream::connect(upstream_addr).await?;
        next_connection_id += 1;
        let connection_id = next_connection_id;

        let (client_reader, client_writer) = client_stream.into_split();
        let (upstream_reader, upstream_writer) = upstream_stream.into_split();

        let client_task = tokio::spawn(proxy_copy_and_capture_legacy(
            client_reader,
            upstream_writer,
            Arc::clone(&capture),
            connection_id,
            LegacyFrameDirection::ClientToDaemon,
        ));
        let server_task = tokio::spawn(proxy_copy_and_capture_legacy(
            upstream_reader,
            client_writer,
            Arc::clone(&capture),
            connection_id,
            LegacyFrameDirection::DaemonToClient,
        ));

        let client_result = client_task.await.context("legacy client proxy task join")?;
        let server_result = server_task.await.context("legacy server proxy task join")?;
        client_result?;
        server_result?;
    }
}

async fn finalize_proxy_task(
    proxy_task: tokio::task::JoinHandle<Result<()>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    stop.store(true, Ordering::Relaxed);
    tokio::time::timeout(Duration::from_secs(2), proxy_task)
        .await
        .context("timed out waiting for proxy task to finish")?
        .context("proxy task join")??;
    Ok(())
}

fn second_client_request_observed(capture: &BusyBeeCapture) -> bool {
    capture
        .client_frames
        .iter()
        .filter_map(|frame| frame.payload.first().copied())
        .filter_map(|byte| ReplicantNetworkMsgtype::decode(byte).ok())
        .filter(|msgtype| *msgtype != ReplicantNetworkMsgtype::Bootstrap)
        .count()
        > 0
}

fn frame_summary(frame: &BusyBeeFrame) -> String {
    let msgtype = frame
        .payload
        .first()
        .copied()
        .and_then(|byte| ReplicantNetworkMsgtype::decode(byte).ok())
        .map(|msgtype| format!("{msgtype:?}"))
        .unwrap_or_else(|| "unknown".to_owned());
    let prefix = frame
        .encode()
        .ok()
        .map(|bytes| {
            bytes
                .iter()
                .take(16)
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| "<encode-failed>".to_owned());
    format!("{msgtype} [{prefix}]")
}

async fn spawn_single_daemon_cluster() -> Result<SingleDaemonCluster> {
    let tempdir = TempDir::new()?;
    let mut coordinator_port = ReservedPort::new()?;
    let mut daemon_port = ReservedPort::new()?;
    let mut daemon_control_port = ReservedPort::new()?;
    let coordinator_address = localhost(coordinator_port.port())?;
    let daemon_address = localhost(daemon_port.port())?;

    coordinator_port.release();
    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", coordinator_port.port()),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_address)
        .await
        .context("waiting for coordinator startup response")?;

    daemon_port.release();
    daemon_control_port.release();
    let mut daemon = ChildProcess::spawn(
        "daemon",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_port.port()),
            format!("--control-port={}", daemon_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon.wait_for_daemon(daemon_address).await?;

    Ok(SingleDaemonCluster {
        _tempdir: tempdir,
        _coordinator: coordinator,
        _daemon: daemon,
        coordinator_address,
    })
}

async fn run_wait_until_stable_probe_via_proxy(
    proxy_addr: SocketAddr,
    capture: Arc<Mutex<BusyBeeCapture>>,
) -> Result<LegacyAdminProbeResult> {
    run_legacy_admin_probe_via_proxy(
        proxy_addr,
        capture,
        "hyperdex-wait-until-stable",
        &["-h", "127.0.0.1", "-p", &proxy_addr.port().to_string()],
        None,
        Duration::from_secs(3),
        true,
    )
    .await
}

async fn run_add_space_probe_via_proxy(
    proxy_addr: SocketAddr,
    capture: Arc<Mutex<BusyBeeCapture>>,
) -> Result<LegacyAdminProbeResult> {
    run_legacy_admin_probe_via_proxy(
        proxy_addr,
        capture,
        "hyperdex-add-space",
        &["-h", "127.0.0.1", "-p", &proxy_addr.port().to_string()],
        Some(MINIMAL_PROFILES_SPACE_DESC),
        Duration::from_secs(5),
        false,
    )
    .await
}

async fn run_add_space_direct_with_schema(
    address: SocketAddr,
    schema: &str,
) -> Result<(Option<std::process::ExitStatus>, String, String)> {
    let stdout_path = std::env::temp_dir().join(format!(
        "legacy-admin-add-space-stdout-{}.log",
        std::process::id()
    ));
    let stderr_path = std::env::temp_dir().join(format!(
        "legacy-admin-add-space-stderr-{}.log",
        std::process::id()
    ));
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;

    let mut child = Command::new(legacy_tool_path("hyperdex-add-space"))
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(address.port().to_string())
        .env("LD_LIBRARY_PATH", legacy_tool_library_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .context("failed to spawn hyperdex-add-space")?;

    {
        use std::io::Write;

        child
            .stdin
            .as_mut()
            .context("hyperdex-add-space stdin missing")?
            .write_all(schema.as_bytes())?;
        child.stdin.take();
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    let exit_status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            child.kill().ok();
            break child.wait().ok();
        }

        sleep(Duration::from_millis(20)).await;
    };

    Ok((
        exit_status,
        fs::read_to_string(stdout_path).unwrap_or_default(),
        fs::read_to_string(stderr_path).unwrap_or_default(),
    ))
}

async fn run_add_space_direct(
    address: SocketAddr,
) -> Result<(Option<std::process::ExitStatus>, String, String)> {
    run_add_space_direct_with_schema(address, MINIMAL_PROFILES_SPACE_DESC).await
}

async fn run_wait_until_stable_direct(
    address: SocketAddr,
) -> Result<(Option<std::process::ExitStatus>, String, String)> {
    let stdout_path = std::env::temp_dir().join(format!(
        "legacy-admin-wait-until-stable-stdout-{}.log",
        std::process::id()
    ));
    let stderr_path = std::env::temp_dir().join(format!(
        "legacy-admin-wait-until-stable-stderr-{}.log",
        std::process::id()
    ));
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;

    let mut child = Command::new(legacy_tool_path("hyperdex-wait-until-stable"))
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(address.port().to_string())
        .env("LD_LIBRARY_PATH", legacy_tool_library_path())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .context("failed to spawn hyperdex-wait-until-stable")?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let exit_status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            child.kill().ok();
            break child.wait().ok();
        }

        sleep(Duration::from_millis(20)).await;
    };

    Ok((
        exit_status,
        fs::read_to_string(stdout_path).unwrap_or_default(),
        fs::read_to_string(stderr_path).unwrap_or_default(),
    ))
}

async fn setup_full_profiles_schema(
    address: SocketAddr,
) -> Result<(
    Option<std::process::ExitStatus>,
    String,
    String,
    Option<std::process::ExitStatus>,
    String,
    String,
)> {
    let (add_exit_status, add_stdout, add_stderr) =
        run_add_space_direct_with_schema(address, FULL_PROFILES_SPACE_DESC).await?;
    let (stable_exit_status, stable_stdout, stable_stderr) =
        run_wait_until_stable_direct(address).await?;

    Ok((
        add_exit_status,
        add_stdout,
        add_stderr,
        stable_exit_status,
        stable_stdout,
        stable_stderr,
    ))
}

async fn run_hyhac_selected_tests_direct(
    address: SocketAddr,
    pattern: &str,
    deadline_span: Duration,
) -> Result<(Option<std::process::ExitStatus>, String, String)> {
    let stdout_path = std::env::temp_dir().join(format!(
        "hyhac-selected-stdout-{}-{}.log",
        std::process::id(),
        address.port()
    ));
    let stderr_path = std::env::temp_dir().join(format!(
        "hyhac-selected-stderr-{}-{}.log",
        std::process::id(),
        address.port()
    ));
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;

    let mut child = Command::new(hyhac_test_binary_path()?)
        .arg("--plain")
        .arg("--test-seed=1")
        .arg(format!("--select-tests={pattern}"))
        .env("LD_LIBRARY_PATH", hyhac_runtime_library_path())
        .env("HYPERDEX_ROOT", legacy_hyperdex_root())
        .env("HYPERDEX_COORD_HOST", "127.0.0.1")
        .env("HYPERDEX_COORD_PORT", address.port().to_string())
        .current_dir(hyhac_root())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("failed to spawn hyhac selected test `{pattern}`"))?;

    let deadline = Instant::now() + deadline_span;
    let exit_status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            child.kill().ok();
            break child.wait().ok();
        }

        sleep(Duration::from_millis(20)).await;
    };

    Ok((
        exit_status,
        fs::read_to_string(stdout_path).unwrap_or_default(),
        fs::read_to_string(stderr_path).unwrap_or_default(),
    ))
}

fn compile_client_trace_preload(work_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let source_path = work_dir.join("client-trace-preload.c");
    let library_path = work_dir.join("libclient-trace-preload.so");
    let log_path = work_dir.join("client-trace.log");
    let source = r#"
#define _GNU_SOURCE
#include <dlfcn.h>
#include <hyperdex/client.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static FILE* open_trace_file(void) {
    const char* path = getenv("HYPERDEX_RS_CLIENT_TRACE");
    if (!path || !path[0]) {
        return NULL;
    }
    return fopen(path, "a");
}

int64_t hyperdex_client_put(struct hyperdex_client* client,
                            const char* space,
                            const char* key, size_t key_sz,
                            const struct hyperdex_client_attribute* attrs, size_t attrs_sz,
                            enum hyperdex_client_returncode* status) {
    static int64_t (*real_fn)(struct hyperdex_client*, const char*, const char*, size_t,
                              const struct hyperdex_client_attribute*, size_t,
                              enum hyperdex_client_returncode*) = NULL;
    if (!real_fn) {
        real_fn = dlsym(RTLD_NEXT, "hyperdex_client_put");
    }
    int64_t handle = real_fn(client, space, key, key_sz, attrs, attrs_sz, status);
    FILE* trace = open_trace_file();
    if (trace) {
        fprintf(trace,
                "put handle=%" PRId64 " status=%d space=%s key=%.*s attrs_sz=%zu\n",
                handle,
                status ? (int)*status : -1,
                space ? space : "(null)",
                (int)key_sz,
                key ? key : "",
                attrs_sz);
        fclose(trace);
    }
    return handle;
}

int64_t hyperdex_client_loop(struct hyperdex_client* client,
                             int timeout,
                             enum hyperdex_client_returncode* status) {
    static int64_t (*real_fn)(struct hyperdex_client*, int, enum hyperdex_client_returncode*) = NULL;
    if (!real_fn) {
        real_fn = dlsym(RTLD_NEXT, "hyperdex_client_loop");
    }
    int64_t handle = real_fn(client, timeout, status);
    FILE* trace = open_trace_file();
    if (trace) {
        fprintf(trace,
                "loop handle=%" PRId64 " status=%d timeout=%d\n",
                handle,
                status ? (int)*status : -1,
                timeout);
        fclose(trace);
    }
    return handle;
}
"#;
    fs::write(&source_path, source)?;
    let output = Command::new("cc")
        .arg("-shared")
        .arg("-fPIC")
        .arg("-O0")
        .arg("-g")
        .arg("-I")
        .arg(legacy_hyperdex_root().join("include"))
        .arg("-o")
        .arg(&library_path)
        .arg(&source_path)
        .arg("-ldl")
        .output()
        .context("failed to compile client-trace preload")?;
    if !output.status.success() {
        return Err(anyhow!(
            "client-trace preload compile failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok((library_path, log_path))
}

async fn run_hyhac_selected_tests_with_client_trace(
    address: SocketAddr,
    pattern: &str,
    deadline_span: Duration,
) -> Result<ClientTraceResult> {
    let tempdir = TempDir::new()?;
    let (preload_path, trace_path) = compile_client_trace_preload(tempdir.path())?;
    let stdout_path = tempdir.path().join("hyhac-client-trace-stdout.log");
    let stderr_path = tempdir.path().join("hyhac-client-trace-stderr.log");
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;

    let mut child = Command::new(hyhac_test_binary_path()?)
        .arg("--plain")
        .arg("--test-seed=1")
        .arg(format!("--select-tests={pattern}"))
        .env("LD_LIBRARY_PATH", hyhac_runtime_library_path())
        .env("LD_PRELOAD", &preload_path)
        .env("HYPERDEX_RS_CLIENT_TRACE", &trace_path)
        .env("HYPERDEX_ROOT", legacy_hyperdex_root())
        .env("HYPERDEX_COORD_HOST", "127.0.0.1")
        .env("HYPERDEX_COORD_PORT", address.port().to_string())
        .current_dir(hyhac_root())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("failed to spawn traced hyhac selected test `{pattern}`"))?;

    let deadline = Instant::now() + deadline_span;
    let exit_status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            child.kill().ok();
            break child.wait().ok();
        }

        sleep(Duration::from_millis(20)).await;
    };

    Ok(ClientTraceResult {
        exit_status,
        stdout: fs::read_to_string(stdout_path).unwrap_or_default(),
        stderr: fs::read_to_string(stderr_path).unwrap_or_default(),
        trace: fs::read_to_string(trace_path).unwrap_or_default(),
    })
}

fn compile_native_large_object_probe(work_dir: &Path) -> Result<PathBuf> {
    let source_path = work_dir.join("native-large-object-probe.c");
    let binary_path = work_dir.join("native-large-object-probe");
    let source = r#"
#include <hyperdex/client.h>
#include <hyperdex/datastructures.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void set_empty_attr(struct hyperdex_client_attribute* attr,
                           const char* name,
                           enum hyperdatatype datatype) {
    static const char empty[] = "";
    attr->attr = name;
    attr->value = empty;
    attr->value_sz = 0;
    attr->datatype = datatype;
}

static int set_int_attr(struct hyperdex_ds_arena* arena,
                        struct hyperdex_client_attribute* attr,
                        const char* name,
                        int64_t value) {
    enum hyperdex_ds_returncode ds_status = HYPERDEX_DS_SUCCESS;
    const char* encoded = NULL;
    size_t encoded_sz = 0;
    if (hyperdex_ds_copy_int(arena, value, &ds_status, &encoded, &encoded_sz) < 0) {
        fprintf(stderr, "hyperdex_ds_copy_int failed for %s with status=%d\n", name, (int)ds_status);
        return -1;
    }
    attr->attr = name;
    attr->value = encoded;
    attr->value_sz = encoded_sz;
    attr->datatype = HYPERDATATYPE_INT64;
    return 0;
}

static int set_float_attr(struct hyperdex_ds_arena* arena,
                          struct hyperdex_client_attribute* attr,
                          const char* name,
                          double value) {
    enum hyperdex_ds_returncode ds_status = HYPERDEX_DS_SUCCESS;
    const char* encoded = NULL;
    size_t encoded_sz = 0;
    if (hyperdex_ds_copy_float(arena, value, &ds_status, &encoded, &encoded_sz) < 0) {
        fprintf(stderr, "hyperdex_ds_copy_float failed for %s with status=%d\n", name, (int)ds_status);
        return -1;
    }
    attr->attr = name;
    attr->value = encoded;
    attr->value_sz = encoded_sz;
    attr->datatype = HYPERDATATYPE_FLOAT;
    return 0;
}

int main(int argc, char** argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: %s <host> <port>\n", argv[0]);
        return 2;
    }

    const char* host = argv[1];
    int port = atoi(argv[2]);
    struct hyperdex_client* client = hyperdex_client_create(host, (uint16_t)port);
    if (!client) {
        fprintf(stderr, "hyperdex_client_create failed\n");
        return 3;
    }

    struct hyperdex_ds_arena* arena = hyperdex_ds_arena_create();
    if (!arena) {
        fprintf(stderr, "hyperdex_ds_arena_create failed\n");
        hyperdex_client_destroy(client);
        return 4;
    }

    struct hyperdex_client_attribute* attrs = hyperdex_ds_allocate_attribute(arena, 19);
    if (!attrs) {
        fprintf(stderr, "hyperdex_ds_allocate_attribute failed\n");
        hyperdex_ds_arena_destroy(arena);
        hyperdex_client_destroy(client);
        return 5;
    }

    set_empty_attr(&attrs[0], "first", HYPERDATATYPE_STRING);
    set_empty_attr(&attrs[1], "last", HYPERDATATYPE_STRING);
    if (set_float_attr(arena, &attrs[2], "score", 0.0) < 0) return 6;
    if (set_int_attr(arena, &attrs[3], "profile_views", 0) < 0) return 7;
    set_empty_attr(&attrs[4], "pending_requests", HYPERDATATYPE_LIST_STRING);
    set_empty_attr(&attrs[5], "rankings", HYPERDATATYPE_LIST_FLOAT);
    set_empty_attr(&attrs[6], "todolist", HYPERDATATYPE_LIST_INT64);
    set_empty_attr(&attrs[7], "hobbies", HYPERDATATYPE_SET_STRING);
    set_empty_attr(&attrs[8], "imonafloat", HYPERDATATYPE_SET_FLOAT);
    set_empty_attr(&attrs[9], "friendids", HYPERDATATYPE_SET_INT64);
    set_empty_attr(&attrs[10], "unread_messages", HYPERDATATYPE_MAP_STRING_STRING);
    set_empty_attr(&attrs[11], "upvotes", HYPERDATATYPE_MAP_STRING_INT64);
    set_empty_attr(&attrs[12], "friendranks", HYPERDATATYPE_MAP_STRING_FLOAT);
    set_empty_attr(&attrs[13], "posts", HYPERDATATYPE_MAP_INT64_STRING);
    set_empty_attr(&attrs[14], "friendremapping", HYPERDATATYPE_MAP_INT64_INT64);
    set_empty_attr(&attrs[15], "intfloatmap", HYPERDATATYPE_MAP_INT64_FLOAT);
    set_empty_attr(&attrs[16], "still_looking", HYPERDATATYPE_MAP_FLOAT_STRING);
    set_empty_attr(&attrs[17], "for_a_reason", HYPERDATATYPE_MAP_FLOAT_INT64);
    set_empty_attr(&attrs[18], "for_float_keyed_map", HYPERDATATYPE_MAP_FLOAT_FLOAT);

    enum hyperdex_client_returncode put_status = HYPERDEX_CLIENT_GARBAGE;
    int64_t handle = hyperdex_client_put(
        client,
        "profiles",
        "large",
        strlen("large"),
        attrs,
        19,
        &put_status);
    printf("put handle=%" PRId64 " status=%d\n", handle, (int)put_status);
    fflush(stdout);

    enum hyperdex_client_returncode loop_status = HYPERDEX_CLIENT_GARBAGE;
    int64_t loop_handle = hyperdex_client_loop(client, -1, &loop_status);
    printf("loop handle=%" PRId64 " status=%d\n", loop_handle, (int)loop_status);
    fflush(stdout);

    hyperdex_ds_arena_destroy(arena);
    hyperdex_client_destroy(client);
    return 0;
}
"#;
    fs::write(&source_path, source)?;
    let library_dir = legacy_hyperdex_root().join(".libs");
    let output = Command::new("cc")
        .arg("-O0")
        .arg("-g")
        .arg("-I")
        .arg(legacy_hyperdex_root().join("include"))
        .arg("-L")
        .arg(&library_dir)
        .arg(format!("-Wl,-rpath,{}", library_dir.display()))
        .arg("-o")
        .arg(&binary_path)
        .arg(&source_path)
        .arg("-lhyperdex-client")
        .output()
        .context("failed to compile native large-object probe")?;
    if !output.status.success() {
        return Err(anyhow!(
            "native large-object probe compile failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(binary_path)
}

async fn run_native_large_object_probe(
    address: SocketAddr,
) -> Result<(Option<std::process::ExitStatus>, String, String)> {
    let tempdir = TempDir::new()?;
    let binary_path = compile_native_large_object_probe(tempdir.path())?;
    let stdout_path = tempdir.path().join("native-large-object-probe-stdout.log");
    let stderr_path = tempdir.path().join("native-large-object-probe-stderr.log");
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;

    let mut child = Command::new(&binary_path)
        .arg("127.0.0.1")
        .arg(address.port().to_string())
        .env("LD_LIBRARY_PATH", legacy_tool_library_path())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .context("failed to spawn native large-object probe")?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let exit_status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            child.kill().ok();
            break child.wait().ok();
        }

        sleep(Duration::from_millis(20)).await;
    };

    Ok((
        exit_status,
        fs::read_to_string(stdout_path).unwrap_or_default(),
        fs::read_to_string(stderr_path).unwrap_or_default(),
    ))
}

async fn spawn_single_daemon_cluster_with_legacy_proxy() -> Result<(
    TempDir,
    ChildProcess,
    ChildProcess,
    SocketAddr,
    tokio::task::JoinHandle<Result<()>>,
    Arc<AtomicBool>,
    Arc<Mutex<LegacyCapture>>,
)> {
    let tempdir = TempDir::new()?;
    let mut coordinator_backend_port = ReservedPort::new()?;
    let mut coordinator_public_port = ReservedPort::new()?;
    let mut daemon_port = ReservedPort::new()?;
    let mut daemon_control_port = ReservedPort::new()?;
    let coordinator_backend_address = localhost(coordinator_backend_port.port())?;
    let coordinator_public_address = localhost(coordinator_public_port.port())?;
    let daemon_address = localhost(daemon_port.port())?;

    coordinator_backend_port.release();
    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", coordinator_backend_port.port()),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_backend_address)
        .await?;

    coordinator_public_port.release();
    let proxy_listener = tokio::net::TcpListener::bind(coordinator_public_address).await?;
    let proxy_stop = Arc::new(AtomicBool::new(false));
    let legacy_capture = Arc::new(Mutex::new(LegacyCapture::default()));
    let proxy_task = tokio::spawn(run_legacy_proxy_capture(
        proxy_listener,
        coordinator_backend_address,
        Arc::clone(&legacy_capture),
        Arc::clone(&proxy_stop),
    ));

    daemon_port.release();
    daemon_control_port.release();
    let mut daemon = ChildProcess::spawn(
        "daemon",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_port.port()),
            format!("--control-port={}", daemon_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_backend_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon.wait_for_daemon(daemon_address).await?;

    Ok((
        tempdir,
        coordinator,
        daemon,
        coordinator_public_address,
        proxy_task,
        proxy_stop,
        legacy_capture,
    ))
}

async fn spawn_single_daemon_cluster_with_busybee_proxy() -> Result<(
    TempDir,
    ChildProcess,
    ChildProcess,
    SocketAddr,
    tokio::task::JoinHandle<Result<()>>,
    Arc<AtomicBool>,
    Arc<Mutex<BusyBeeCapture>>,
)> {
    let tempdir = TempDir::new()?;
    let mut coordinator_backend_port = ReservedPort::new()?;
    let mut coordinator_public_port = ReservedPort::new()?;
    let mut daemon_port = ReservedPort::new()?;
    let mut daemon_control_port = ReservedPort::new()?;
    let coordinator_backend_address = localhost(coordinator_backend_port.port())?;
    let coordinator_public_address = std::env::var("HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR")
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(localhost(coordinator_public_port.port())?);
    let daemon_address = localhost(daemon_port.port())?;

    coordinator_backend_port.release();
    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", coordinator_backend_port.port()),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_backend_address)
        .await?;

    coordinator_public_port.release();
    let proxy_listener = tokio::net::TcpListener::bind(coordinator_public_address).await?;
    let proxy_stop = Arc::new(AtomicBool::new(false));
    let busybee_capture = Arc::new(Mutex::new(BusyBeeCapture::default()));
    let proxy_task = tokio::spawn(run_busybee_proxy_capture(
        proxy_listener,
        coordinator_backend_address,
        Arc::clone(&busybee_capture),
        Arc::clone(&proxy_stop),
    ));

    daemon_port.release();
    daemon_control_port.release();
    let mut daemon = ChildProcess::spawn(
        "daemon",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_port.port()),
            format!("--control-port={}", daemon_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_backend_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon.wait_for_daemon(daemon_address).await?;

    Ok((
        tempdir,
        coordinator,
        daemon,
        coordinator_public_address,
        proxy_task,
        proxy_stop,
        busybee_capture,
    ))
}

async fn run_legacy_admin_probe_via_proxy(
    _proxy_addr: SocketAddr,
    capture: Arc<Mutex<BusyBeeCapture>>,
    tool: &str,
    args: &[&str],
    stdin: Option<&str>,
    deadline_span: Duration,
    stop_on_progress: bool,
) -> Result<LegacyAdminProbeResult> {
    let stdout_path = std::env::temp_dir().join(format!(
        "legacy-admin-probe-stdout-{}.log",
        std::process::id()
    ));
    let stderr_path = std::env::temp_dir().join(format!(
        "legacy-admin-probe-stderr-{}.log",
        std::process::id()
    ));
    let stdout_file = File::create(&stdout_path)?;
    let stderr_file = File::create(&stderr_path)?;

    let mut child = Command::new(legacy_tool_path(tool))
        .args(args)
        .env("LD_LIBRARY_PATH", legacy_tool_library_path())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("failed to spawn {tool}"))?;

    if let Some(stdin) = stdin {
        use std::io::Write;

        child
            .stdin
            .as_mut()
            .context("legacy admin tool stdin missing")?
            .write_all(stdin.as_bytes())?;
        child.stdin.take();
    }

    let deadline = Instant::now() + deadline_span;
    let tool_exit = loop {
        if stop_on_progress && second_client_request_observed(&capture.lock().unwrap()) {
            child.kill().ok();
            break child.wait().ok();
        }

        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            child.kill().ok();
            break child.wait().ok();
        }

        sleep(Duration::from_millis(20)).await;
    };

    Ok(LegacyAdminProbeResult {
        capture: capture.lock().unwrap().clone(),
        tool_exit,
        stdout: fs::read_to_string(stdout_path).unwrap_or_default(),
        stderr: fs::read_to_string(stderr_path).unwrap_or_default(),
    })
}

fn count_request_header(nonce: u64) -> RequestHeader {
    RequestHeader {
        message_type: LegacyMessageType::ReqCount,
        flags: 0,
        version: 7,
        target_virtual_server: 0,
        nonce,
    }
}

fn get_request_header(nonce: u64) -> RequestHeader {
    RequestHeader {
        message_type: LegacyMessageType::ReqGet,
        flags: 0,
        version: 7,
        target_virtual_server: 0,
        nonce,
    }
}

fn atomic_request_header(nonce: u64) -> RequestHeader {
    RequestHeader {
        message_type: LegacyMessageType::ReqAtomic,
        flags: 0,
        version: 7,
        target_virtual_server: 0,
        nonce,
    }
}

async fn request_count(
    address: SocketAddr,
    space: &str,
    nonce: u64,
) -> Result<(ResponseHeader, CountResponse)> {
    let (header, body) = request_once(
        address,
        count_request_header(nonce),
        &CountRequest {
            space: space.to_owned(),
        }
        .encode_body(),
    )
    .await?;
    Ok((header, CountResponse::decode_body(&body)?))
}

async fn request_get(
    address: SocketAddr,
    key: &[u8],
    nonce: u64,
) -> Result<(ResponseHeader, GetResponse)> {
    let (header, body) = request_once(
        address,
        get_request_header(nonce),
        &GetRequest { key: key.to_vec() }.encode_body(),
    )
    .await?;
    Ok((header, GetResponse::decode_body(&body)?))
}

async fn request_atomic(
    address: SocketAddr,
    request: &AtomicRequest,
    nonce: u64,
) -> Result<(ResponseHeader, AtomicResponse)> {
    let (header, body) = request_once(
        address,
        atomic_request_header(nonce),
        &request.encode_body(),
    )
    .await?;
    Ok((header, AtomicResponse::decode_body(&body)?))
}

async fn request_search_all(
    address: SocketAddr,
    space: &str,
    checks: Vec<LegacyCheck>,
    search_id: u64,
    nonce: u64,
) -> Result<Vec<SearchItemResponse>> {
    let (mut header, mut body) = request_once(
        address,
        RequestHeader {
            message_type: LegacyMessageType::ReqSearchStart,
            flags: 0,
            version: 7,
            target_virtual_server: 0,
            nonce,
        },
        &SearchStartRequest {
            space: space.to_owned(),
            search_id,
            checks,
        }
        .encode_body(),
    )
    .await?;

    let mut items = Vec::new();
    loop {
        match header.message_type {
            LegacyMessageType::RespSearchItem => {
                items.push(SearchItemResponse::decode_body(&body)?);
                (header, body) = request_once(
                    address,
                    RequestHeader {
                        message_type: LegacyMessageType::ReqSearchNext,
                        flags: 0,
                        version: 7,
                        target_virtual_server: 0,
                        nonce: nonce + items.len() as u64,
                    },
                    &SearchContinueRequest { search_id }.encode_body(),
                )
                .await?;
            }
            LegacyMessageType::RespSearchDone => {
                let done = SearchDoneResponse::decode_body(&body)?;
                if done.search_id != search_id {
                    return Err(anyhow!(
                        "search completion id mismatch: expected {search_id}, got {}",
                        done.search_id
                    ));
                }
                return Ok(items);
            }
            other => {
                return Err(anyhow!(
                    "unexpected legacy search response message type: {other:?}"
                ));
            }
        }
    }
}

async fn request_internode_data_plane(
    address: SocketAddr,
    request: DataPlaneRequest,
) -> Result<DataPlaneResponse> {
    let mut client = grpc_api::v1::internode_transport_client::InternodeTransportClient::connect(
        format!("http://{address}"),
    )
    .await?;
    let request = InternodeRequest::encode(DATA_PLANE_METHOD, &request)?;
    let response = client
        .send(grpc_api::v1::InternodeRpcRequest {
            method: request.method,
            body: request.body.to_vec(),
        })
        .await?
        .into_inner();
    let response = InternodeResponse {
        status: response.status as u16,
        body: response.body.into(),
    };
    Ok(response.decode()?)
}

fn grpc_route_runtime(
    daemon_one_control_port: u16,
    daemon_one_data_port: u16,
    daemon_two_control_port: u16,
    daemon_two_data_port: u16,
) -> ClusterRuntime {
    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: daemon_one_control_port,
                data_port: daemon_one_data_port,
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: daemon_two_control_port,
                data_port: daemon_two_data_port,
            },
        ],
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    ClusterRuntime::for_node(config, 1).expect("grpc route runtime")
}

#[tokio::test]
#[serial]
async fn coordinator_space_add_reaches_multiple_daemon_processes() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let tempdir = TempDir::new()?;
    let mut coordinator_port = ReservedPort::new()?;
    let mut daemon_one_port = ReservedPort::new()?;
    let mut daemon_two_port = ReservedPort::new()?;
    let mut daemon_one_control_port = ReservedPort::new()?;
    let mut daemon_two_control_port = ReservedPort::new()?;
    let coordinator_address = localhost(coordinator_port.port())?;
    let daemon_one_address = localhost(daemon_one_port.port())?;
    let daemon_two_address = localhost(daemon_two_port.port())?;
    let daemon_one_control_address = localhost(daemon_one_control_port.port())?;
    let daemon_two_control_address = localhost(daemon_two_control_port.port())?;

    coordinator_port.release();
    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", coordinator_port.port()),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_address)
        .await?;

    daemon_one_port.release();
    daemon_one_control_port.release();
    let mut daemon_one = ChildProcess::spawn(
        "daemon-one",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-one").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_one_port.port()),
            format!("--control-port={}", daemon_one_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_one
        .wait_for_daemon(daemon_one_address)
        .await
        .context("waiting for daemon one legacy frontend startup response")?;

    daemon_two_port.release();
    daemon_two_control_port.release();
    let mut daemon_two = ChildProcess::spawn(
        "daemon-two",
        &[
            "daemon".to_owned(),
            "--node-id=2".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-two").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_two_port.port()),
            format!("--control-port={}", daemon_two_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_two
        .wait_for_daemon(daemon_two_address)
        .await
        .context("waiting for daemon two legacy frontend startup response")?;

    coordinator
        .wait_for_config_view(coordinator_address, "both daemon registrations", |view| {
            view.cluster.nodes.iter().any(|node| node.id == 1)
                && view.cluster.nodes.iter().any(|node| node.id == 2)
        })
        .await
        .context("waiting for both daemon registrations in coordinator config")?;

    let ready = request_coordinator_control_with_body_once(
        coordinator_address,
        CoordinatorAdminRequest::WaitUntilStable.method_name(),
        &CoordinatorAdminRequest::WaitUntilStable,
    )
    .await
    .context("requesting wait_until_stable after both daemon registrations")?;
    assert_eq!(
        CoordinatorReturnCode::decode(&ready.status)?,
        CoordinatorReturnCode::Success
    );

    let space = parse_hyperdex_space(
        r#"
        space profiles
        key username
        attributes
        string first
        string last
        "#,
    )?;
    let status = request_coordinator_control_once(
        coordinator_address,
        CoordinatorAdminRequest::SpaceAdd(space.clone()).method_name(),
        &CoordinatorAdminRequest::SpaceAdd(space),
    )
    .await
    .context("requesting coordinator space_add for `profiles`")?;
    assert_eq!(
        CoordinatorReturnCode::decode(&status)?,
        CoordinatorReturnCode::Success
    );

    coordinator
        .wait_for_config_view(
            coordinator_address,
            "space `profiles` in config view",
            |view| view.spaces.iter().any(|space| space.name == "profiles"),
        )
        .await
        .context("waiting for `profiles` space to appear in coordinator config")?;
    daemon_one
        .wait_for_internode_space(daemon_one_control_address, "profiles")
        .await
        .context("waiting for daemon one internode readiness for `profiles`")?;
    daemon_two
        .wait_for_internode_space(daemon_two_control_address, "profiles")
        .await
        .context("waiting for daemon two internode readiness for `profiles`")?;

    let (daemon_one_header, daemon_one_count) =
        request_count(daemon_one_address, "profiles", 1).await?;
    assert_eq!(daemon_one_header.message_type, LegacyMessageType::RespCount);
    assert_eq!(daemon_one_count.count, 0);

    let (daemon_two_header, daemon_two_count) =
        request_count(daemon_two_address, "profiles", 2).await?;
    assert_eq!(daemon_two_header.message_type, LegacyMessageType::RespCount);
    assert_eq!(daemon_two_count.count, 0);

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_atomic_routes_numeric_update_to_remote_primary_process() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let tempdir = TempDir::new()?;
    let mut coordinator_port = ReservedPort::new()?;
    let mut daemon_one_port = ReservedPort::new()?;
    let mut daemon_two_port = ReservedPort::new()?;
    let mut daemon_one_control_port = ReservedPort::new()?;
    let mut daemon_two_control_port = ReservedPort::new()?;
    let coordinator_address = localhost(coordinator_port.port())?;
    let daemon_one_address = localhost(daemon_one_port.port())?;
    let daemon_two_address = localhost(daemon_two_port.port())?;
    let daemon_one_control_address = localhost(daemon_one_control_port.port())?;
    let daemon_two_control_address = localhost(daemon_two_control_port.port())?;

    coordinator_port.release();
    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", coordinator_port.port()),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_address)
        .await?;

    daemon_one_port.release();
    daemon_one_control_port.release();
    let mut daemon_one = ChildProcess::spawn(
        "daemon-one",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-one").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_one_port.port()),
            format!("--control-port={}", daemon_one_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_one.wait_for_daemon(daemon_one_address).await?;

    daemon_two_port.release();
    daemon_two_control_port.release();
    let mut daemon_two = ChildProcess::spawn(
        "daemon-two",
        &[
            "daemon".to_owned(),
            "--node-id=2".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-two").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_two_port.port()),
            format!("--control-port={}", daemon_two_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_two.wait_for_daemon(daemon_two_address).await?;

    coordinator
        .wait_for_config_view(coordinator_address, "both daemon registrations", |view| {
            view.cluster.nodes.iter().any(|node| node.id == 1)
                && view.cluster.nodes.iter().any(|node| node.id == 2)
        })
        .await?;

    let route_runtime = grpc_route_runtime(
        daemon_one_control_port.port(),
        daemon_one_port.port(),
        daemon_two_control_port.port(),
        daemon_two_port.port(),
    );
    let key = (0..4096)
        .map(|i| format!("remote-atomic-{i}"))
        .find(|key| route_runtime.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let space = parse_hyperdex_space(
        r#"
        space profiles
        key username
        attributes
        int profile_views
        "#,
    )?;
    let status = request_coordinator_control_once(
        coordinator_address,
        CoordinatorAdminRequest::SpaceAdd(space.clone()).method_name(),
        &CoordinatorAdminRequest::SpaceAdd(space),
    )
    .await
    .context("requesting coordinator space_add for `profiles` in remote primary test")?;
    assert_eq!(
        CoordinatorReturnCode::decode(&status)?,
        CoordinatorReturnCode::Success
    );

    coordinator
        .wait_for_config_view(
            coordinator_address,
            "space `profiles` in config view",
            |view| view.spaces.iter().any(|space| space.name == "profiles"),
        )
        .await?;
    daemon_one
        .wait_for_internode_space(daemon_one_control_address, "profiles")
        .await
        .context(
            "waiting for daemon one internode readiness for `profiles` in remote primary test",
        )?;
    daemon_two
        .wait_for_internode_space(daemon_two_control_address, "profiles")
        .await
        .context(
            "waiting for daemon two internode readiness for `profiles` in remote primary test",
        )?;

    let (atomic_header, atomic_response) = request_atomic(
        daemon_one_address,
        &AtomicRequest {
            flags: LEGACY_ATOMIC_FLAG_WRITE,
            key: key.as_bytes().to_vec(),
            checks: Vec::new(),
            funcalls: vec![LegacyFuncall {
                attribute: "profile_views".to_owned(),
                name: LegacyFuncallName::NumAdd,
                arg1: GetValue::Int(3),
                arg2: None,
            }],
        },
        41,
    )
    .await?;
    assert_eq!(atomic_header.message_type, LegacyMessageType::RespAtomic);
    assert_eq!(atomic_response.status, LegacyReturnCode::Success);

    let (get_header, get_response) = request_get(daemon_two_address, key.as_bytes(), 42).await?;
    assert_eq!(get_header.message_type, LegacyMessageType::RespGet);
    assert_eq!(get_response.status, LegacyReturnCode::Success);
    assert!(get_response
        .attributes
        .iter()
        .any(|GetAttribute { name, value }| {
            name == "profile_views" && *value == GetValue::Int(3)
        }));

    Ok(())
}

#[tokio::test]
#[serial]
async fn degraded_search_and_count_survive_one_daemon_process_shutdown() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let tempdir = TempDir::new()?;
    let mut coordinator_port = ReservedPort::new()?;
    let mut daemon_one_port = ReservedPort::new()?;
    let mut daemon_two_port = ReservedPort::new()?;
    let mut daemon_one_control_port = ReservedPort::new()?;
    let mut daemon_two_control_port = ReservedPort::new()?;
    let coordinator_address = localhost(coordinator_port.port())?;
    let daemon_one_address = localhost(daemon_one_port.port())?;
    let daemon_two_address = localhost(daemon_two_port.port())?;
    let daemon_one_control_address = localhost(daemon_one_control_port.port())?;
    let daemon_two_control_address = localhost(daemon_two_control_port.port())?;

    coordinator_port.release();
    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", coordinator_port.port()),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_address)
        .await?;

    daemon_one_port.release();
    daemon_one_control_port.release();
    let mut daemon_one = ChildProcess::spawn(
        "daemon-one",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-one").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_one_port.port()),
            format!("--control-port={}", daemon_one_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_one.wait_for_daemon(daemon_one_address).await?;

    daemon_two_port.release();
    daemon_two_control_port.release();
    let mut daemon_two = ChildProcess::spawn(
        "daemon-two",
        &[
            "daemon".to_owned(),
            "--node-id=2".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-two").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={}", daemon_two_port.port()),
            format!("--control-port={}", daemon_two_control_port.port()),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={}", coordinator_port.port()),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_two.wait_for_daemon(daemon_two_address).await?;

    coordinator
        .wait_for_config_view(coordinator_address, "both daemon registrations", |view| {
            view.cluster.nodes.iter().any(|node| node.id == 1)
                && view.cluster.nodes.iter().any(|node| node.id == 2)
        })
        .await?;

    let space = parse_hyperdex_space(
        r#"
        space profiles
        key username
        attributes
        int profile_views
        "#,
    )?;
    let status = request_coordinator_control_once(
        coordinator_address,
        CoordinatorAdminRequest::SpaceAdd(space.clone()).method_name(),
        &CoordinatorAdminRequest::SpaceAdd(space),
    )
    .await
    .context("requesting coordinator space_add for `profiles` in degraded read test")?;
    assert_eq!(
        CoordinatorReturnCode::decode(&status)?,
        CoordinatorReturnCode::Success
    );

    coordinator
        .wait_for_config_view(
            coordinator_address,
            "space `profiles` in config view",
            |view| view.spaces.iter().any(|space| space.name == "profiles"),
        )
        .await?;
    daemon_one
        .wait_for_internode_space(daemon_one_control_address, "profiles")
        .await
        .context(
            "waiting for daemon one internode readiness for `profiles` in degraded read test",
        )?;
    daemon_two
        .wait_for_internode_space(daemon_two_control_address, "profiles")
        .await
        .context(
            "waiting for daemon two internode readiness for `profiles` in degraded read test",
        )?;

    for (nonce, key, views) in [
        (100_u64, "degraded-search-a", 7_i64),
        (101_u64, "degraded-search-b", 9_i64),
        (102_u64, "degraded-search-survivor", 1_i64),
    ] {
        let (atomic_header, atomic_response) = request_atomic(
            daemon_one_address,
            &AtomicRequest {
                flags: LEGACY_ATOMIC_FLAG_WRITE,
                key: key.as_bytes().to_vec(),
                checks: Vec::new(),
                funcalls: vec![LegacyFuncall {
                    attribute: "profile_views".to_owned(),
                    name: LegacyFuncallName::NumAdd,
                    arg1: GetValue::Int(views),
                    arg2: None,
                }],
            },
            nonce,
        )
        .await?;
        assert_eq!(atomic_header.message_type, LegacyMessageType::RespAtomic);
        assert_eq!(atomic_response.status, LegacyReturnCode::Success);
    }

    daemon_two.stop()?;
    sleep(Duration::from_millis(100)).await;

    let mut keys: Vec<Vec<u8>> = request_search_all(
        daemon_one_address,
        "profiles",
        vec![LegacyCheck {
            attribute: "profile_views".to_owned(),
            predicate: LegacyPredicate::GreaterThanOrEqual,
            value: GetValue::Int(5),
        }],
        77,
        200,
    )
    .await?
    .into_iter()
    .map(|item| item.key)
    .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![b"degraded-search-a".to_vec(), b"degraded-search-b".to_vec()]
    );

    let (count_header, count_response) = request_count(daemon_one_address, "profiles", 300).await?;
    assert_eq!(count_header.message_type, LegacyMessageType::RespCount);
    assert_eq!(count_response.count, 3);

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_admin_wait_until_stable_probe_reports_bootstrap_progress() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let mut proxy_port = ReservedPort::new()?;
    let proxy_address = localhost(proxy_port.port())?;
    proxy_port.release();
    let proxy_listener = tokio::net::TcpListener::bind(proxy_address).await?;
    std::env::set_var(
        "HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR",
        proxy_address.to_string(),
    );
    let cluster = spawn_single_daemon_cluster().await?;
    std::env::remove_var("HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR");

    let capture = Arc::new(Mutex::new(BusyBeeCapture::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let proxy_capture = Arc::clone(&capture);
    let proxy_stop = Arc::clone(&stop);
    let proxy_task = tokio::spawn(run_busybee_proxy_capture(
        proxy_listener,
        cluster.coordinator_address,
        proxy_capture,
        proxy_stop,
    ));

    let probe = run_wait_until_stable_probe_via_proxy(proxy_address, Arc::clone(&capture)).await?;
    finalize_proxy_task(proxy_task, stop).await?;

    let bootstrap_frame = probe
        .capture
        .client_frames
        .iter()
        .find(|frame| {
            frame.payload.len() == 1
                && ReplicantNetworkMsgtype::decode(frame.payload[0])
                    .is_ok_and(|msgtype| msgtype == ReplicantNetworkMsgtype::Bootstrap)
        })
        .context("probe never captured the bootstrap frame")?;
    assert!(
        !probe.capture.server_frames.is_empty(),
        "probe captured no server frames; stdout=`{}` stderr=`{}`",
        probe.stdout,
        probe.stderr
    );

    let advanced = second_client_request_observed(&probe.capture);
    eprintln!(
        "legacy admin bootstrap progress: advanced={advanced} tool_exit={:?} client_frames={} server_frames={} first_client={} first_server={}",
        probe.tool_exit,
        probe.capture.client_frames.len(),
        probe.capture.server_frames.len(),
        frame_summary(bootstrap_frame),
        frame_summary(&probe.capture.server_frames[0]),
    );

    if std::env::var_os("HYPERDEX_EXPECT_LEGACY_ADMIN_ADVANCE").is_some() {
        assert!(
            advanced,
            "expected the C admin client to advance beyond bootstrap; client_frames={:?} server_frames={:?} stdout=`{}` stderr=`{}`",
            probe.capture
                .client_frames
                .iter()
                .map(frame_summary)
                .collect::<Vec<_>>(),
            probe.capture
                .server_frames
                .iter()
                .map(frame_summary)
                .collect::<Vec<_>>(),
            probe.stdout,
            probe.stderr
        );
    }

    assert!(
        probe.tool_exit.is_some() || !probe.capture.server_frames.is_empty(),
        "probe produced no observable result"
    );
    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_admin_add_space_probe_completes_after_bootstrap_and_robust_call() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let mut proxy_port = ReservedPort::new()?;
    let proxy_address = localhost(proxy_port.port())?;
    proxy_port.release();
    let proxy_listener = tokio::net::TcpListener::bind(proxy_address).await?;
    std::env::set_var(
        "HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR",
        proxy_address.to_string(),
    );
    let cluster = spawn_single_daemon_cluster().await?;
    std::env::remove_var("HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR");

    let capture = Arc::new(Mutex::new(BusyBeeCapture::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let proxy_capture = Arc::clone(&capture);
    let proxy_stop = Arc::clone(&stop);
    let proxy_task = tokio::spawn(run_busybee_proxy_capture(
        proxy_listener,
        cluster.coordinator_address,
        proxy_capture,
        proxy_stop,
    ));

    let probe = run_add_space_probe_via_proxy(proxy_address, Arc::clone(&capture)).await?;
    finalize_proxy_task(proxy_task, stop).await?;

    let frame_summaries = probe
        .capture
        .client_frames
        .iter()
        .map(frame_summary)
        .collect::<Vec<_>>();
    eprintln!(
        "legacy add-space probe: tool_exit={:?} client_frames={:?} server_frames={:?} stdout=`{}` stderr=`{}`",
        probe.tool_exit,
        frame_summaries,
        probe.capture
            .server_frames
            .iter()
            .map(frame_summary)
            .collect::<Vec<_>>(),
        probe.stdout,
        probe.stderr,
    );

    if let Some(call_frame) = probe.capture.client_frames.iter().find(|frame| {
        frame.payload.first().copied().is_some_and(|byte| {
            ReplicantNetworkMsgtype::decode(byte)
                .is_ok_and(|msgtype| msgtype == ReplicantNetworkMsgtype::CallRobust)
        })
    }) {
        let decoded = ReplicantAdminRequestMessage::decode(&call_frame.payload)?;
        eprintln!("decoded add-space robust request: {decoded:?}");
        if let ReplicantAdminRequestMessage::CallRobust { input, .. } = &decoded {
            match decode_packed_hyperdex_space(input) {
                Ok(space) => eprintln!("decoded live packed space: {space:?}"),
                Err(err) => eprintln!("live packed space decode error: {err}"),
            }
        }
    }
    if let Some(response_frame) = probe.capture.server_frames.iter().find(|frame| {
        ReplicantCallCompletion::decode(&frame.payload)
            .ok()
            .is_some_and(|completion| completion.nonce == 12)
    }) {
        let decoded = ReplicantCallCompletion::decode(&response_frame.payload)?;
        eprintln!("decoded add-space robust response: {decoded:?}");
    }

    assert_eq!(
        probe.tool_exit.map(|status| status.code()),
        Some(Some(0)),
        "expected hyperdex-add-space to exit successfully"
    );
    assert!(
        probe.capture.client_frames.iter().any(|frame| {
            frame.payload.first().copied().is_some_and(|byte| {
                ReplicantNetworkMsgtype::decode(byte)
                    .is_ok_and(|msgtype| msgtype == ReplicantNetworkMsgtype::GetRobustParams)
            })
        }),
        "expected hyperdex-add-space to request robust params"
    );
    assert!(
        probe.capture.client_frames.iter().any(|frame| {
            frame.payload.first().copied().is_some_and(|byte| {
                ReplicantNetworkMsgtype::decode(byte)
                    .is_ok_and(|msgtype| msgtype == ReplicantNetworkMsgtype::CallRobust)
            })
        }),
        "expected hyperdex-add-space to issue a robust call"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_admin_add_space_succeeds_against_live_cluster() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;

    let (exit_status, stdout, stderr) = run_add_space_direct(cluster.coordinator_address).await?;
    eprintln!(
        "legacy add-space direct: exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    assert_eq!(exit_status.map(|status| status.code()), Some(Some(0)));
    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_large_object_probe_hits_clientgarbage_fast() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;

    let started = Instant::now();
    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*Can store a large object*",
        Duration::from_secs(10),
    )
    .await?;
    eprintln!(
        "hyhac large-object probe: elapsed={:?} exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`",
        started.elapsed(),
    );

    assert!(
        stdout.contains("Can store a large object: [Failed]"),
        "expected the focused hyhac probe to fail in the large-object test"
    );
    assert!(
        stdout.contains("Left ClientGarbage"),
        "expected the focused hyhac probe to report ClientGarbage"
    );
    assert!(
        exit_status.is_some(),
        "expected the focused hyhac probe to finish before the deadline"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_large_object_probe_reports_no_daemon_traffic_after_startup() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let capture_file = tempfile::NamedTempFile::new()?;
    let capture_path = capture_file.path().to_path_buf();
    drop(capture_file);

    std::env::set_var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE", &capture_path);
    let mut cluster = spawn_single_daemon_cluster().await?;
    std::env::remove_var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE");
    fs::write(&capture_path, "")?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*Can store a large object*",
        Duration::from_secs(10),
    )
    .await?;
    cluster._coordinator.ensure_running()?;
    cluster._daemon.ensure_running()?;

    let capture = fs::read_to_string(&capture_path).unwrap_or_default();
    eprintln!(
        "hyhac large-object daemon trace: exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}` capture=`{capture}`"
    );

    assert!(
        stdout.contains("Left ClientGarbage"),
        "expected the focused hyhac probe to report ClientGarbage"
    );
    assert!(
        capture.trim().is_empty(),
        "expected no daemon legacy frontend traffic after clearing the harness startup probe; capture=`{capture}`"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_large_object_probe_reports_immediate_unknownspace_before_deferred_loop(
) -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let capture_file = tempfile::NamedTempFile::new()?;
    let capture_path = capture_file.path().to_path_buf();
    drop(capture_file);

    std::env::set_var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE", &capture_path);
    let mut cluster = spawn_single_daemon_cluster().await?;
    std::env::remove_var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE");
    fs::write(&capture_path, "")?;

    let hyhac = run_hyhac_selected_tests_with_client_trace(
        cluster.coordinator_address,
        "*Can store a large object*",
        Duration::from_secs(10),
    )
    .await?;
    let native = run_native_large_object_probe(cluster.coordinator_address).await?;

    cluster._coordinator.ensure_running()?;
    cluster._daemon.ensure_running()?;

    let capture = fs::read_to_string(&capture_path).unwrap_or_default();
    eprintln!(
        "hyhac immediate-handle probe: hyhac_exit={:?} hyhac_stdout=`{}` hyhac_stderr=`{}` hyhac_trace=`{}` native_exit={:?} native_stdout=`{}` native_stderr=`{}` capture=`{}`",
        hyhac.exit_status,
        hyhac.stdout,
        hyhac.stderr,
        hyhac.trace,
        native.0,
        native.1,
        native.2,
        capture
    );

    assert!(
        hyhac.stdout.contains("Left ClientGarbage"),
        "expected the focused hyhac probe to report ClientGarbage"
    );
    assert!(
        hyhac.trace.contains("put handle=-1 status=8512"),
        "expected hyhac to receive immediate UnknownSpace from hyperdex_client_put; trace=`{}`",
        hyhac.trace
    );
    assert!(
        hyhac.trace.contains("loop handle=-1 status=8523"),
        "expected hyhac to hit NonePending in hyperdex_client_loop after demanding the negative handle; trace=`{}`",
        hyhac.trace
    );
    assert!(
        native.1.contains("put handle=-1 status=8512"),
        "expected the native C probe to receive the same immediate UnknownSpace status; stdout=`{}` stderr=`{}`",
        native.1,
        native.2
    );
    assert!(
        native.1.contains("loop handle=-1 status=8523"),
        "expected the native C probe to report NonePending after the failed put; stdout=`{}` stderr=`{}`",
        native.1,
        native.2
    );
    assert!(
        capture.trim().is_empty(),
        "expected no daemon legacy frontend traffic because the request fails before send; capture=`{}`",
        capture
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_large_object_probe_reaches_daemon_after_full_profiles_setup() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let capture_file = tempfile::NamedTempFile::new()?;
    let capture_path = capture_file.path().to_path_buf();
    drop(capture_file);

    std::env::set_var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE", &capture_path);
    let mut cluster = spawn_single_daemon_cluster().await?;
    std::env::remove_var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE");

    let (add_exit_status, add_stdout, add_stderr, stable_exit_status, stable_stdout, stable_stderr) =
        setup_full_profiles_schema(cluster.coordinator_address).await?;

    fs::write(&capture_path, "")?;

    let native = run_native_large_object_probe(cluster.coordinator_address).await?;
    let capture_after_native = fs::read_to_string(&capture_path).unwrap_or_default();

    eprintln!(
        "hyhac full-profiles native-first probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` native_exit={:?} native_stdout=`{}` native_stderr=`{}` capture_after_native=`{}`",
        native.0,
        native.1,
        native.2,
        capture_after_native
    );

    cluster._coordinator.ensure_running()?;
    cluster._daemon.ensure_running()?;

    fs::write(&capture_path, "")?;

    let hyhac = run_hyhac_selected_tests_with_client_trace(
        cluster.coordinator_address,
        "*Can store a large object*",
        Duration::from_secs(20),
    )
    .await?;

    cluster._coordinator.ensure_running()?;
    cluster._daemon.ensure_running()?;

    let capture = fs::read_to_string(&capture_path).unwrap_or_default();
    eprintln!(
        "hyhac full-profiles large-object probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` hyhac_exit={:?} hyhac_stdout=`{}` hyhac_stderr=`{}` hyhac_trace=`{}` native_exit={:?} native_stdout=`{}` native_stderr=`{}` capture=`{}`",
        hyhac.exit_status,
        hyhac.stdout,
        hyhac.stderr,
        hyhac.trace,
        native.0,
        native.1,
        native.2,
        capture
    );

    assert_eq!(add_exit_status.map(|status| status.code()), Some(Some(0)));
    assert_eq!(
        stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert!(
        !hyhac.trace.contains("put handle=-1 status=8512"),
        "expected the corrected probe to move beyond immediate UnknownSpace; trace=`{}`",
        hyhac.trace
    );
    assert!(
        !native.1.contains("put handle=-1 status=8512"),
        "expected the native probe to move beyond immediate UnknownSpace once the full schema exists; stdout=`{}` stderr=`{}`",
        native.1,
        native.2
    );
    assert!(
        !capture.trim().is_empty(),
        "expected daemon legacy frontend traffic after the full profiles schema is created; capture=`{}`",
        capture
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_large_object_probe_reports_first_coordinator_frame_pair() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let (
        _tempdir,
        mut coordinator,
        mut daemon,
        coordinator_address,
        proxy_task,
        proxy_stop,
        legacy_capture,
    ) = spawn_single_daemon_cluster_with_legacy_proxy().await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        coordinator_address,
        "*Can store a large object*",
        Duration::from_secs(10),
    )
    .await?;

    finalize_proxy_task(proxy_task, proxy_stop).await?;
    coordinator.ensure_running()?;
    daemon.ensure_running()?;

    let capture = legacy_capture.lock().unwrap().clone();
    let events = capture
        .events
        .iter()
        .map(|event| {
            format!(
                "conn={} dir={:?} {} raw=[{}]",
                event.connection_id, event.direction, event.summary, event.raw_prefix
            )
        })
        .collect::<Vec<_>>();
    eprintln!(
        "hyhac large-object legacy proxy: coordinator_proxy_address={coordinator_address} exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}` events={events:?}"
    );

    assert!(
        stdout.contains("Left ClientGarbage"),
        "expected focused hyhac probe to report ClientGarbage"
    );
    assert_eq!(
        capture.events.len(),
        4,
        "expected two client/server partial-frame pairs on the coordinator path"
    );
    assert!(
        capture
            .events
            .iter()
            .any(|event| matches!(event.direction, LegacyFrameDirection::ClientToDaemon)),
        "expected proxy to capture at least one client-to-daemon frame"
    );
    assert!(
        capture
            .events
            .iter()
            .any(|event| matches!(event.direction, LegacyFrameDirection::DaemonToClient)),
        "expected proxy to capture at least one daemon-to-client frame"
    );
    assert!(
        capture
            .events
            .iter()
            .all(|event| event.summary.starts_with("partial frame")),
        "expected the coordinator-path proxy to observe only partial non-legacy frames: {events:?}"
    );
    assert!(
        capture
            .events
            .iter()
            .filter(|event| matches!(event.direction, LegacyFrameDirection::ClientToDaemon))
            .all(|event| {
                event.summary.contains("trailing_bytes=45")
                    && event.raw_prefix.starts_with(
                        "80 00 00 14 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05"
                    )
            }),
        "expected client-side coordinator frames to be 45-byte partial BusyBee-style payloads: {events:?}"
    );
    assert!(
        capture
            .events
            .iter()
            .filter(|event| matches!(event.direction, LegacyFrameDirection::DaemonToClient))
            .all(|event| {
                event.summary.contains("trailing_bytes=80")
                    && event.raw_prefix.starts_with(
                        "80 00 00 14 00 00 00 00 00 00 00 02 00 00 00 00 00 00 00 00 00 00 00 3c"
                    )
            }),
        "expected server-side coordinator frames to be 80-byte partial BusyBee-style payloads: {events:?}"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_large_object_probe_reports_coordinator_busybee_sequence() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let mut proxy_port = ReservedPort::new()?;
    let proxy_address = localhost(proxy_port.port())?;
    proxy_port.release();
    std::env::set_var(
        "HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR",
        proxy_address.to_string(),
    );
    let (
        _tempdir,
        mut coordinator,
        mut daemon,
        _coordinator_address,
        proxy_task,
        proxy_stop,
        busybee_capture,
    ) = spawn_single_daemon_cluster_with_busybee_proxy().await?;
    std::env::remove_var("HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR");

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        proxy_address,
        "*Can store a large object*",
        Duration::from_secs(10),
    )
    .await?;

    finalize_proxy_task(proxy_task, proxy_stop).await?;
    coordinator.ensure_running()?;
    daemon.ensure_running()?;

    let capture = busybee_capture.lock().unwrap().clone();
    let client_frames = capture
        .client_frames
        .iter()
        .map(frame_summary)
        .collect::<Vec<_>>();
    let server_frames = capture
        .server_frames
        .iter()
        .map(frame_summary)
        .collect::<Vec<_>>();
    eprintln!(
        "hyhac large-object busybee proxy: coordinator_proxy_address={proxy_address} exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}` client_frames={client_frames:?} server_frames={server_frames:?}"
    );

    assert!(
        !capture.client_frames.is_empty(),
        "expected proxy to capture client-to-coordinator BusyBee frames"
    );
    assert!(
        !capture.server_frames.is_empty(),
        "expected proxy to capture coordinator-to-client BusyBee frames"
    );
    let client_msgtypes = capture
        .client_frames
        .iter()
        .filter_map(|frame| frame.payload.first().copied())
        .filter_map(|byte| ReplicantNetworkMsgtype::decode(byte).ok())
        .collect::<Vec<_>>();
    let server_msgtypes = capture
        .server_frames
        .iter()
        .filter_map(|frame| frame.payload.first().copied())
        .filter_map(|byte| ReplicantNetworkMsgtype::decode(byte).ok())
        .collect::<Vec<_>>();
    assert_eq!(
        client_msgtypes.first(),
        Some(&ReplicantNetworkMsgtype::Bootstrap),
        "expected the coordinator path to begin with bootstrap"
    );
    assert!(
        second_client_request_observed(&capture),
        "expected the client to advance beyond bootstrap on the coordinator connection; client_frames={client_frames:?} server_frames={server_frames:?}"
    );
    assert!(
        server_msgtypes.contains(&ReplicantNetworkMsgtype::ClientResponse),
        "expected the coordinator to answer follow requests with client responses; server_frames={server_frames:?}"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_integer_div_probe_turns_green_after_full_profiles_setup() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;
    let (add_exit_status, add_stdout, add_stderr, stable_exit_status, stable_stdout, stable_stderr) =
        setup_full_profiles_schema(cluster.coordinator_address).await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*pooled*/*atomic*/*integer*/*div",
        Duration::from_secs(15),
    )
    .await?;
    eprintln!(
        "hyhac full-profiles integer-div probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    assert_eq!(add_exit_status.map(|status| status.code()), Some(Some(0)));
    assert_eq!(
        stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert!(
        stdout.contains("pooled:\n      atomic:\n        integer:"),
        "expected the focused probe to stay inside the pooled integer atomic group"
    );
    assert!(
        stdout.contains("div: [OK, passed 100 tests]"),
        "expected the focused probe to show integer div success"
    );
    assert!(
        !stdout.contains("[Failed]"),
        "expected the focused probe to avoid any remaining failures"
    );
    assert!(
        !stdout.contains("ClientReconfigure"),
        "expected the focused probe to avoid the prior client reconfigure path"
    );
    assert!(
        !stdout.contains("Failed in running atomic op:"),
        "expected the focused probe to avoid the prior atomic failure details"
    );
    assert!(
        !stdout.contains("search: ["),
        "expected the focused probe to avoid rerunning earlier pooled groups"
    );
    assert!(
        !stdout.contains("count: ["),
        "expected the focused probe to avoid rerunning earlier pooled groups"
    );
    assert!(
        !stdout.contains("mod: ["),
        "expected the focused probe to isolate div instead of later atomic failures"
    );
    assert!(
        exit_status.is_some(),
        "expected the focused probe to finish before the deadline"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_pooled_probe_turns_green_after_map_atomic_compatibility() -> Result<()>
{
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;
    let (add_exit_status, add_stdout, add_stderr, stable_exit_status, stable_stdout, stable_stderr) =
        setup_full_profiles_schema(cluster.coordinator_address).await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*pooled*",
        Duration::from_secs(30),
    )
    .await?;
    eprintln!(
        "hyhac full-profiles pooled probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    let map_section_idx = stdout
        .find("        map:\n          int-int:\n            union: [OK, passed 100 tests]")
        .context("pooled hyhac probe did not preserve map int-int union success")?;

    assert_eq!(add_exit_status.map(|status| status.code()), Some(Some(0)));
    assert_eq!(
        stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert!(
        stdout.contains("Can store a large object: [OK]"),
        "expected the large-object boundary to stay green"
    );
    assert!(
        stdout.contains("roundtrip: [OK, passed 100 tests]"),
        "expected the pooled roundtrip boundary to stay green"
    );
    assert!(
        stdout.contains("conditional: [OK, passed 100 tests]"),
        "expected the pooled conditional boundary to stay green"
    );
    assert!(
        stdout.contains("search: [OK, passed 100 tests]"),
        "expected the pooled search boundary to stay green"
    );
    assert!(
        stdout.contains("count: [OK, passed 100 tests]"),
        "expected the pooled count boundary to stay green"
    );
    assert!(
        stdout.contains(
            "        integer:\n          add: [OK, passed 100 tests]\n          sub: [OK, passed 100 tests]\n          mul: [OK, passed 100 tests]\n          div: [OK, passed 100 tests]\n          mod: [OK, passed 100 tests]"
        ),
        "expected the pooled integer atomic section to stay green through div and mod"
    );
    assert!(
        stdout.contains(
            "        float:\n          add: [OK, passed 100 tests]\n          sub: [OK, passed 100 tests]\n          mul: [OK, passed 100 tests]\n          div: [OK, passed 100 tests]"
        ),
        "expected the pooled float atomic section to stay green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          int-int:\n            union: [OK, passed 100 tests]\n            add: [OK, passed 100 tests]\n            sub: [OK, passed 100 tests]\n            mul: [OK, passed 100 tests]\n            div: [OK, passed 100 tests]\n            and: [OK, passed 100 tests]\n            or: [OK, passed 100 tests]\n            xor: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the int-int numeric map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          int-float:\n            union: [OK, passed 100 tests]\n            add: [OK, passed 100 tests]\n            sub: [OK, passed 100 tests]\n            mul: [OK, passed 100 tests]\n            div: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the int-float numeric map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          float-int:\n            union: [OK, passed 100 tests]\n            add: [OK, passed 100 tests]\n            sub: [OK, passed 100 tests]\n            mul: [OK, passed 100 tests]\n            div: [OK, passed 100 tests]\n            and: [OK, passed 100 tests]\n            or: [OK, passed 100 tests]\n            xor: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the float-int numeric map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          float-float:\n            union: [OK, passed 100 tests]\n            add: [OK, passed 100 tests]\n            sub: [OK, passed 100 tests]\n            mul: [OK, passed 100 tests]\n            div: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the float-float numeric map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          string-int:\n            union: [OK, passed 100 tests]\n            add: [OK, passed 100 tests]\n            sub: [OK, passed 100 tests]\n            mul: [OK, passed 100 tests]\n            div: [OK, passed 100 tests]\n            and: [OK, passed 100 tests]\n            or: [OK, passed 100 tests]\n            xor: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the string-int numeric map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          string-float:\n            union: [OK, passed 100 tests]\n            add: [OK, passed 100 tests]\n            sub: [OK, passed 100 tests]\n            mul: [OK, passed 100 tests]\n            div: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the string-float numeric map section green"
    );
    assert!(
        stdout[map_section_idx..].contains("          int-string:\n            union: [OK, passed 100 tests]"),
        "expected the pooled probe to reach the string-valued map section"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          int-string:\n            union: [OK, passed 100 tests]\n            prepend: [OK, passed 100 tests]\n            prepend: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the int-string string-map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          float-string:\n            union: [OK, passed 100 tests]\n            prepend: [OK, passed 100 tests]\n            prepend: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the float-string string-map section green"
    );
    assert!(
        stdout[map_section_idx..].contains(
            "          string-string:\n            union: [OK, passed 100 tests]\n            prepend: [OK, passed 100 tests]\n            prepend: [OK, passed 100 tests]"
        ),
        "expected the pooled probe to keep the string-string string-map section green"
    );
    assert!(
        !stdout.contains("[Failed]"),
        "expected the pooled hyhac probe to avoid later failures"
    );
    assert!(
        !stdout.contains("ClientServererror"),
        "expected the pooled hyhac probe to avoid server errors"
    );
    assert_eq!(
        exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the pooled hyhac probe to exit successfully"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_split_acceptance_suite_passes_live_cluster() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;

    let admin_cluster = spawn_single_daemon_cluster().await?;
    let (add_exit_status, add_stdout, add_stderr) = run_hyhac_selected_tests_direct(
        admin_cluster.coordinator_address,
        "*Can add a space*",
        Duration::from_secs(20),
    )
    .await?;
    eprintln!(
        "hyhac live add-space acceptance: exit_status={add_exit_status:?} stdout=`{add_stdout}` stderr=`{add_stderr}`"
    );
    assert_eq!(
        add_exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the live hyhac add-space acceptance phase to exit successfully"
    );
    assert!(
        add_stdout.contains("Can add a space: [OK]"),
        "expected the live hyhac add-space acceptance phase to pass"
    );

    let (remove_exit_status, remove_stdout, remove_stderr) = run_hyhac_selected_tests_direct(
        admin_cluster.coordinator_address,
        "*Can remove a space*",
        Duration::from_secs(20),
    )
    .await?;
    eprintln!(
        "hyhac live remove-space acceptance: exit_status={remove_exit_status:?} stdout=`{remove_stdout}` stderr=`{remove_stderr}`"
    );
    assert_eq!(
        remove_exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the live hyhac remove-space acceptance phase to exit successfully"
    );
    assert!(
        remove_stdout.contains("Can remove a space: [OK]"),
        "expected the live hyhac remove-space acceptance phase to pass"
    );

    let data_cluster = spawn_single_daemon_cluster().await?;
    let (
        setup_add_exit_status,
        setup_add_stdout,
        setup_add_stderr,
        setup_stable_exit_status,
        setup_stable_stdout,
        setup_stable_stderr,
    ) = setup_full_profiles_schema(data_cluster.coordinator_address).await?;
    eprintln!(
        "hyhac live data acceptance setup: add_exit={setup_add_exit_status:?} add_stdout=`{setup_add_stdout}` add_stderr=`{setup_add_stderr}` stable_exit={setup_stable_exit_status:?} stable_stdout=`{setup_stable_stdout}` stable_stderr=`{setup_stable_stderr}`"
    );
    assert_eq!(
        setup_add_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert_eq!(
        setup_stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );

    let (pooled_exit_status, pooled_stdout, pooled_stderr) = run_hyhac_selected_tests_direct(
        data_cluster.coordinator_address,
        "*pooled*",
        Duration::from_secs(30),
    )
    .await?;
    eprintln!(
        "hyhac live pooled acceptance: exit_status={pooled_exit_status:?} stdout=`{pooled_stdout}` stderr=`{pooled_stderr}`"
    );
    assert_eq!(
        pooled_exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the live hyhac pooled acceptance phase to exit successfully"
    );
    assert!(
        pooled_stdout.contains("Properties  Test Cases  Total"),
        "expected pooled acceptance output to include the pooled summary"
    );
    assert!(
        !pooled_stdout.contains("[Failed]"),
        "expected the live hyhac pooled acceptance phase to avoid failures"
    );

    let (shared_exit_status, shared_stdout, shared_stderr) = run_hyhac_selected_tests_direct(
        data_cluster.coordinator_address,
        "*shared*",
        Duration::from_secs(20),
    )
    .await?;
    eprintln!(
        "hyhac live shared acceptance: exit_status={shared_exit_status:?} stdout=`{shared_stdout}` stderr=`{shared_stderr}`"
    );
    assert_eq!(
        shared_exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the live hyhac shared acceptance phase to exit successfully"
    );
    assert!(
        shared_stdout.contains("shared:"),
        "expected shared acceptance output to include the shared section"
    );
    assert!(
        !shared_stdout.contains("[Failed]"),
        "expected the live hyhac shared acceptance phase to avoid failures"
    );

    let (cbstring_exit_status, cbstring_stdout, cbstring_stderr) = run_hyhac_selected_tests_direct(
        data_cluster.coordinator_address,
        "*CBString*",
        Duration::from_secs(20),
    )
    .await?;
    eprintln!(
        "hyhac live cbstring acceptance: exit_status={cbstring_exit_status:?} stdout=`{cbstring_stdout}` stderr=`{cbstring_stderr}`"
    );
    assert_eq!(
        cbstring_exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the live hyhac CBString acceptance phase to exit successfully"
    );
    assert!(
        cbstring_stdout.contains("CBString API Tests of varying size:"),
        "expected CBString acceptance output to include the CBString section"
    );
    assert!(
        !cbstring_stdout.contains("[Failed]"),
        "expected the live hyhac CBString acceptance phase to avoid failures"
    );
    assert!(
        !cbstring_stdout.contains(" but got: Left "),
        "expected the live hyhac acceptance phases to avoid compatibility failures"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_map_int_int_add_probe_turns_green_after_full_profiles_setup() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;
    let (add_exit_status, add_stdout, add_stderr, stable_exit_status, stable_stdout, stable_stderr) =
        setup_full_profiles_schema(cluster.coordinator_address).await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*pooled*/*atomic*/*map*/*int-int*/*add",
        Duration::from_secs(15),
    )
    .await?;
    eprintln!(
        "hyhac full-profiles map-int-int-add probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    assert_eq!(add_exit_status.map(|status| status.code()), Some(Some(0)));
    assert_eq!(
        stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert!(
        stdout.contains("pooled:\n      atomic:\n        map:\n          int-int:"),
        "expected the focused probe to stay inside the pooled map int-int atomic group"
    );
    assert!(
        stdout.contains("add: [OK, passed 100 tests]"),
        "expected the focused probe to show map int-int add success"
    );
    assert!(
        !stdout.contains("[Failed]"),
        "expected the focused probe to avoid later failures"
    );
    assert!(
        !stdout.contains("ClientServererror"),
        "expected the focused probe to avoid the prior server error"
    );
    assert!(
        !stdout.contains("integer:\n"),
        "expected the focused probe to avoid rerunning the earlier integer group"
    );
    assert!(
        exit_status.is_some(),
        "expected the focused probe to finish before the deadline"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_map_string_string_prepend_probe_turns_green_after_full_profiles_setup(
) -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;
    let (add_exit_status, add_stdout, add_stderr, stable_exit_status, stable_stdout, stable_stderr) =
        setup_full_profiles_schema(cluster.coordinator_address).await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*pooled*/*atomic*/*map*/*string-string*/*prepend",
        Duration::from_secs(15),
    )
    .await?;
    eprintln!(
        "hyhac full-profiles map-string-string-prepend probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    assert_eq!(add_exit_status.map(|status| status.code()), Some(Some(0)));
    assert_eq!(
        stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert!(
        stdout.contains("pooled:\n      atomic:\n        map:\n          string-string:"),
        "expected the focused probe to stay inside the pooled map string-string atomic group"
    );
    assert!(
        stdout.contains("prepend: [OK, passed 100 tests]"),
        "expected the focused probe to turn the string-string prepend path green"
    );
    assert!(
        !stdout.contains("ClientServererror"),
        "expected the focused probe to avoid server errors after the string-map fix"
    );
    assert!(
        !stdout.contains("[Failed]"),
        "expected the focused probe to avoid failed string-string prepend checks"
    );
    assert_eq!(
        exit_status.map(|status| status.code()),
        Some(Some(0)),
        "expected the focused probe to exit successfully"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn legacy_hyhac_map_int_string_prepend_probe_turns_green_after_numeric_map_boundary(
) -> Result<()>
{
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;
    let (add_exit_status, add_stdout, add_stderr, stable_exit_status, stable_stdout, stable_stderr) =
        setup_full_profiles_schema(cluster.coordinator_address).await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*pooled*/*atomic*/*map*/*int-string*/*prepend",
        Duration::from_secs(15),
    )
    .await?;
    eprintln!(
        "hyhac full-profiles map-int-string-prepend probe: add_exit={add_exit_status:?} add_stdout=`{add_stdout}` add_stderr=`{add_stderr}` stable_exit={stable_exit_status:?} stable_stdout=`{stable_stdout}` stable_stderr=`{stable_stderr}` exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    assert_eq!(add_exit_status.map(|status| status.code()), Some(Some(0)));
    assert_eq!(
        stable_exit_status.map(|status| status.code()),
        Some(Some(0))
    );
    assert!(
        stdout.contains("pooled:\n      atomic:\n        map:\n          int-string:"),
        "expected the focused probe to stay inside the pooled map int-string atomic group"
    );
    assert!(
        stdout.contains("prepend: [OK, passed 100 tests]"),
        "expected the focused probe to turn the int-string prepend path green"
    );
    assert!(
        !stdout.contains("[Failed]"),
        "expected the focused probe to avoid failed int-string prepend checks"
    );
    assert!(
        !stdout.contains("ClientServererror"),
        "expected the focused probe to avoid server errors after the numeric-map fix"
    );
    assert!(
        !stdout.contains("int-int:"),
        "expected the focused probe to avoid rerunning the earlier numeric map group"
    );
    assert!(
        exit_status.map(|status| status.code()) == Some(Some(0)),
        "expected the focused probe to exit successfully"
    );

    Ok(())
}

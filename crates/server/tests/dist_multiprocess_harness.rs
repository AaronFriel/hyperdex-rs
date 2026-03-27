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
    SearchDoneResponse, SearchItemResponse, SearchStartRequest, LEGACY_ATOMIC_FLAG_WRITE,
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
        .await?;

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
        Some(
            "space profiles\n\
             key username\n\
             attributes\n\
                 string first,\n\
                 int profile_views\n\
             subspace first\n\
             create 8 partitions\n\
             tolerate 0 failures\n",
        ),
        Duration::from_secs(5),
        false,
    )
    .await
}

async fn run_add_space_direct(
    address: SocketAddr,
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
            .write_all(
                b"space profiles\n\
                  key username\n\
                  attributes\n\
                      string first,\n\
                      int profile_views\n\
                  subspace first\n\
                  create 8 partitions\n\
                  tolerate 0 failures\n",
            )?;
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

    let ready = request_coordinator_control_with_body_once(
        coordinator_address,
        CoordinatorAdminRequest::WaitUntilStable.method_name(),
        &CoordinatorAdminRequest::WaitUntilStable,
    )
    .await?;
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
    .await?;
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
        .await?;
    daemon_two
        .wait_for_internode_space(daemon_two_control_address, "profiles")
        .await?;

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
    .await?;
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
        .await?;
    daemon_two
        .wait_for_internode_space(daemon_two_control_address, "profiles")
        .await?;

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
    .await?;
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
        .await?;
    daemon_two
        .wait_for_internode_space(daemon_two_control_address, "profiles")
        .await?;

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
async fn legacy_hyhac_pooled_probe_reports_large_object_failure_first() -> Result<()> {
    let _guard = MULTIPROCESS_HARNESS_LOCK.lock().await;
    let cluster = spawn_single_daemon_cluster().await?;

    let (exit_status, stdout, stderr) = run_hyhac_selected_tests_direct(
        cluster.coordinator_address,
        "*pooled*",
        Duration::from_secs(20),
    )
    .await?;
    eprintln!(
        "hyhac pooled probe: exit_status={exit_status:?} stdout=`{stdout}` stderr=`{stderr}`"
    );

    let large_object_idx = stdout
        .find("Can store a large object: [Failed]")
        .context("pooled hyhac probe did not report the large-object failure")?;
    let roundtrip_idx = stdout
        .find("roundtrip: [Failed]")
        .context("pooled hyhac probe did not reach the later roundtrip failure")?;
    assert!(
        large_object_idx < roundtrip_idx,
        "expected the large-object failure to appear before later pooled failures"
    );
    assert!(
        stdout.contains("Left ClientGarbage"),
        "expected the pooled hyhac probe to report ClientGarbage"
    );
    assert!(
        exit_status.is_some(),
        "expected the pooled hyhac probe to finish before the deadline"
    );

    Ok(())
}

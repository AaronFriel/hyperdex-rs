use std::fs::{self, File};
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use cluster_config::{ClusterConfig, ClusterNode, TransportBackend};
use data_model::parse_hyperdex_space;
use hyperdex_admin_protocol::{CoordinatorAdminRequest, CoordinatorReturnCode};
use legacy_frontend::request_once;
use legacy_protocol::{
    AtomicRequest, AtomicResponse, CountRequest, CountResponse, GetAttribute, GetRequest,
    GetResponse, GetValue, LegacyFuncall, LegacyFuncallName, LegacyMessageType, LegacyReturnCode,
    RequestHeader, ResponseHeader, LEGACY_ATOMIC_FLAG_WRITE,
};
use server::{
    request_coordinator_control_once, request_coordinator_control_with_body_once, ClusterRuntime,
};
use tempfile::TempDir;
use tokio::time::sleep;

struct ChildProcess {
    name: &'static str,
    child: Child,
    log_path: PathBuf,
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
                Err(err)
                    if err.downcast_ref::<io::Error>().is_some_and(|io_err| {
                        io_err.kind() == io::ErrorKind::ConnectionRefused
                            || io_err.kind() == io::ErrorKind::TimedOut
                    }) =>
                {
                    self.ensure_running()?;
                }
                Err(err) => return Err(err.into()),
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

    async fn wait_for_log(&mut self, needle: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let logs = self.read_logs()?;
            if logs.contains(needle) {
                return Ok(());
            }

            self.ensure_running()?;
            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for {} log containing `{needle}`\n{}",
                    self.name,
                    logs
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

fn reserve_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
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
async fn coordinator_space_add_reaches_multiple_daemon_processes() -> Result<()> {
    let tempdir = TempDir::new()?;
    let coordinator_port = reserve_port()?;
    let daemon_one_port = reserve_port()?;
    let daemon_two_port = reserve_port()?;
    let daemon_one_control_port = reserve_port()?;
    let daemon_two_control_port = reserve_port()?;
    let coordinator_address: SocketAddr = format!("127.0.0.1:{coordinator_port}").parse()?;
    let daemon_one_address: SocketAddr = format!("127.0.0.1:{daemon_one_port}").parse()?;
    let daemon_two_address: SocketAddr = format!("127.0.0.1:{daemon_two_port}").parse()?;

    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={coordinator_port}"),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_address)
        .await?;

    let mut daemon_one = ChildProcess::spawn(
        "daemon-one",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-one").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={daemon_one_port}"),
            format!("--control-port={daemon_one_control_port}"),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={coordinator_port}"),
        ],
        tempdir.path(),
    )?;
    daemon_one.wait_for_daemon(daemon_one_address).await?;

    let mut daemon_two = ChildProcess::spawn(
        "daemon-two",
        &[
            "daemon".to_owned(),
            "--node-id=2".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-two").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={daemon_two_port}"),
            format!("--control-port={daemon_two_control_port}"),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={coordinator_port}"),
        ],
        tempdir.path(),
    )?;
    daemon_two.wait_for_daemon(daemon_two_address).await?;

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

    daemon_one
        .wait_for_log("daemon synchronized coordinator config")
        .await?;
    daemon_two
        .wait_for_log("daemon synchronized coordinator config")
        .await?;
    daemon_one.wait_for_log("version=3").await?;
    daemon_two.wait_for_log("version=3").await?;

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
async fn legacy_atomic_routes_numeric_update_to_remote_primary_process() -> Result<()> {
    let tempdir = TempDir::new()?;
    let coordinator_port = reserve_port()?;
    let daemon_one_port = reserve_port()?;
    let daemon_two_port = reserve_port()?;
    let daemon_one_control_port = reserve_port()?;
    let daemon_two_control_port = reserve_port()?;
    let coordinator_address: SocketAddr = format!("127.0.0.1:{coordinator_port}").parse()?;
    let daemon_one_address: SocketAddr = format!("127.0.0.1:{daemon_one_port}").parse()?;
    let daemon_two_address: SocketAddr = format!("127.0.0.1:{daemon_two_port}").parse()?;

    let mut coordinator = ChildProcess::spawn(
        "coordinator",
        &[
            "coordinator".to_owned(),
            format!("--data={}", tempdir.path().join("coordinator").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={coordinator_port}"),
        ],
        tempdir.path(),
    )?;
    coordinator
        .wait_for_coordinator(coordinator_address)
        .await?;

    let mut daemon_one = ChildProcess::spawn(
        "daemon-one",
        &[
            "daemon".to_owned(),
            "--node-id=1".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-one").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={daemon_one_port}"),
            format!("--control-port={daemon_one_control_port}"),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={coordinator_port}"),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_one.wait_for_daemon(daemon_one_address).await?;

    let mut daemon_two = ChildProcess::spawn(
        "daemon-two",
        &[
            "daemon".to_owned(),
            "--node-id=2".to_owned(),
            "--threads=1".to_owned(),
            format!("--data={}", tempdir.path().join("daemon-two").display()),
            "--listen=127.0.0.1".to_owned(),
            format!("--listen-port={daemon_two_port}"),
            format!("--control-port={daemon_two_control_port}"),
            "--coordinator=127.0.0.1".to_owned(),
            format!("--coordinator-port={coordinator_port}"),
            "--transport=grpc".to_owned(),
        ],
        tempdir.path(),
    )?;
    daemon_two.wait_for_daemon(daemon_two_address).await?;

    daemon_one
        .wait_for_log("daemon internode gRPC listening")
        .await?;
    daemon_two
        .wait_for_log("daemon internode gRPC listening")
        .await?;

    let route_runtime = grpc_route_runtime(
        daemon_one_control_port,
        daemon_one_port,
        daemon_two_control_port,
        daemon_two_port,
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

    daemon_one
        .wait_for_log("daemon synchronized coordinator config")
        .await?;
    daemon_two
        .wait_for_log("daemon synchronized coordinator config")
        .await?;
    daemon_one.wait_for_log("version=3").await?;
    daemon_two.wait_for_log("version=3").await?;

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

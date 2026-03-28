use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use legacy_protocol::{
    config_mismatch_response, encode_identify_frame, encode_request_frame, encode_response_frame,
    RequestHeader, ResponseHeader, BUSYBEE_HEADER_IDENTIFY, BUSYBEE_HEADER_SIZE,
    LEGACY_REQUEST_HEADER_SIZE,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn maybe_capture_legacy_frontend_event(event: &str) {
    let Ok(path) = std::env::var("HYPERDEX_RS_LEGACY_FRONTEND_CAPTURE") else {
        return;
    };
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{event}")
        });
}

fn capture_hex_prefix(bytes: &[u8], width: usize) -> String {
    bytes
        .iter()
        .take(width)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_identify_remote_server_id(bytes: &[u8]) -> Option<u64> {
    let identify_end = BUSYBEE_HEADER_SIZE + 16;
    if bytes.len() < identify_end {
        return None;
    }

    let remote_server_id = [
        bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
    ];
    Some(u64::from_be_bytes(remote_server_id))
}

pub struct LegacyFrontend {
    listener: TcpListener,
    local_server_id: u64,
}

impl LegacyFrontend {
    pub async fn bind(address: SocketAddr) -> Result<Self> {
        Self::bind_with_server_id(address, 0).await
    }

    pub async fn bind_with_server_id(address: SocketAddr, local_server_id: u64) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(address).await?,
            local_server_id,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn serve_once(&self) -> Result<()> {
        self.serve_once_with(|header, _body| async move {
            Ok((config_mismatch_response(header), Vec::new()))
        })
        .await
    }

    pub async fn serve_once_with<H, F>(&self, handler: H) -> Result<()>
    where
        H: Fn(RequestHeader, Vec<u8>) -> F,
        F: std::future::Future<Output = Result<(ResponseHeader, Vec<u8>)>>,
    {
        let (mut stream, _) = self.listener.accept().await?;
        let Some((header, body)) = read_request_frame(&mut stream, self.local_server_id).await?
        else {
            return Ok(());
        };
        let (response, response_body) = handler(header, body).await?;
        maybe_capture_legacy_frontend_event(&format!(
            "response mt={:?} target_vsi={} nonce={} body_len={} body_prefix={}",
            response.message_type,
            response.target_virtual_server,
            response.nonce,
            response_body.len(),
            capture_hex_prefix(&response_body, 16)
        ));

        stream
            .write_all(&encode_response_frame(response, &response_body))
            .await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn serve_forever(&self) -> Result<()> {
        self.serve_forever_with(|header, _body| async move {
            Ok((config_mismatch_response(header), Vec::new()))
        })
        .await
    }

    pub async fn serve_forever_with<H, F>(&self, handler: H) -> Result<()>
    where
        H: Fn(RequestHeader, Vec<u8>) -> F + Send + Sync + 'static,
        F: std::future::Future<Output = Result<(ResponseHeader, Vec<u8>)>> + Send + 'static,
    {
        let handler = Arc::new(handler);

        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let handler = handler.clone();
            let local_server_id = self.local_server_id;
            tokio::spawn(async move {
                if let Err(err) = serve_connection_with(&mut stream, local_server_id, handler).await
                {
                    maybe_capture_legacy_frontend_event(&format!("connection_error {err:#}"));
                }
            });
        }
    }
}

async fn serve_connection_with<H, F>(
    stream: &mut TcpStream,
    local_server_id: u64,
    handler: Arc<H>,
) -> Result<()>
where
    H: Fn(RequestHeader, Vec<u8>) -> F + Send + Sync + 'static,
    F: std::future::Future<Output = Result<(ResponseHeader, Vec<u8>)>> + Send + 'static,
{
    while let Some((header, body)) = read_request_frame(stream, local_server_id).await? {
        let (response, response_body) = handler(header, body).await?;
        maybe_capture_legacy_frontend_event(&format!(
            "response mt={:?} target_vsi={} nonce={} body_len={} body_prefix={}",
            response.message_type,
            response.target_virtual_server,
            response.nonce,
            response_body.len(),
            capture_hex_prefix(&response_body, 16)
        ));
        stream
            .write_all(&encode_response_frame(response, &response_body))
            .await?;
        stream.flush().await?;
    }

    Ok(())
}

async fn read_request_frame(
    stream: &mut tokio::net::TcpStream,
    local_server_id: u64,
) -> Result<Option<(RequestHeader, Vec<u8>)>> {
    loop {
        let mut prefix = [0u8; BUSYBEE_HEADER_SIZE];
        match stream.read_exact(&mut prefix).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(err) => return Err(err.into()),
        }

        let raw_header = u32::from_be_bytes(prefix);
        let total_len = (raw_header & 0x00ff_ffff) as usize;
        if total_len < BUSYBEE_HEADER_SIZE {
            anyhow::bail!("busybee frame size {total_len} is too small");
        }

        let mut bytes = vec![0u8; total_len];
        bytes[..BUSYBEE_HEADER_SIZE].copy_from_slice(&prefix);
        stream
            .read_exact(&mut bytes[BUSYBEE_HEADER_SIZE..total_len])
            .await?;
        maybe_capture_legacy_frontend_event(&format!(
            "frame flags=0x{raw_header:08x} len={} prefix={}",
            bytes.len(),
            capture_hex_prefix(&bytes, 32)
        ));

        if raw_header & BUSYBEE_HEADER_IDENTIFY != 0 {
            let remote_server_id = decode_identify_remote_server_id(&bytes).unwrap_or(0);
            stream
                .write_all(&encode_identify_frame(local_server_id, remote_server_id))
                .await?;
            stream.flush().await?;
            maybe_capture_legacy_frontend_event(&format!(
                "identify local_server_id={} remote_server_id={remote_server_id}",
                local_server_id
            ));
            continue;
        }

        if bytes.len() < LEGACY_REQUEST_HEADER_SIZE {
            maybe_capture_legacy_frontend_event(&format!(
                "short_request len={} required={LEGACY_REQUEST_HEADER_SIZE}",
                bytes.len()
            ));
            anyhow::bail!(
                "legacy request frame is {} bytes, shorter than header {}",
                bytes.len(),
                LEGACY_REQUEST_HEADER_SIZE
            );
        }

        let header = RequestHeader::decode(&bytes[..LEGACY_REQUEST_HEADER_SIZE])?;
        maybe_capture_legacy_frontend_event(&format!(
            "request mt={:?} flags=0x{:02x} version={} target_vsi={} nonce={}",
            header.message_type,
            header.flags,
            header.version,
            header.target_virtual_server,
            header.nonce
        ));
        let handler_body_offset = LEGACY_REQUEST_HEADER_SIZE - std::mem::size_of::<u64>();
        let body = bytes[handler_body_offset..].to_vec();
        return Ok(Some((header, body)));
    }
}

pub async fn request_once(
    address: SocketAddr,
    header: RequestHeader,
    body: &[u8],
) -> Result<(ResponseHeader, Vec<u8>)> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream
        .write_all(&encode_request_frame(header, body))
        .await?;
    stream.flush().await?;

    let mut prefix = [0u8; BUSYBEE_HEADER_SIZE];
    stream.read_exact(&mut prefix).await?;
    let total_len = (u32::from_be_bytes(prefix) & 0x00ff_ffff) as usize;
    if total_len < BUSYBEE_HEADER_SIZE {
        anyhow::bail!("busybee frame size {total_len} is too small");
    }
    let mut response = vec![0u8; total_len];
    response[..BUSYBEE_HEADER_SIZE].copy_from_slice(&prefix);
    stream
        .read_exact(&mut response[BUSYBEE_HEADER_SIZE..total_len])
        .await?;
    let header = ResponseHeader::decode(&response)?;
    let body = response[legacy_protocol::LEGACY_RESPONSE_HEADER_SIZE..].to_vec();
    Ok((header, body))
}

#[cfg(test)]
mod tests;

use std::net::SocketAddr;

use anyhow::Result;
use legacy_protocol::{
    config_mismatch_response, RequestHeader, ResponseHeader, LEGACY_REQUEST_HEADER_SIZE,
    LEGACY_RESPONSE_HEADER_SIZE,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct LegacyFrontend {
    listener: TcpListener,
}

impl LegacyFrontend {
    pub async fn bind(address: SocketAddr) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(address).await?,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn serve_once(&self) -> Result<()> {
        let (mut stream, _) = self.listener.accept().await?;
        let header = read_request_header(&mut stream).await?;
        let response = config_mismatch_response(header);

        stream.write_all(&response.encode()).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn serve_forever(&self) -> Result<()> {
        loop {
            self.serve_once().await?;
        }
    }
}

async fn read_request_header(
    stream: &mut tokio::net::TcpStream,
) -> Result<RequestHeader> {
    let mut bytes = [0u8; LEGACY_REQUEST_HEADER_SIZE];
    stream.read_exact(&mut bytes).await?;
    Ok(RequestHeader::decode(&bytes)?)
}

pub async fn request_once(address: SocketAddr, header: RequestHeader) -> Result<ResponseHeader> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream.write_all(&header.encode()).await?;
    stream.flush().await?;

    let mut response = [0u8; LEGACY_RESPONSE_HEADER_SIZE];
    stream.read_exact(&mut response).await?;
    Ok(ResponseHeader::decode(&response)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legacy_protocol::{LegacyMessageType, RequestHeader};

    #[tokio::test]
    async fn serve_once_returns_config_mismatch() {
        let frontend = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move { frontend.serve_once().await.unwrap() });
        let response = request_once(
            address,
            RequestHeader {
                message_type: LegacyMessageType::ReqGet,
                flags: 0,
                version: 7,
                target_virtual_server: 11,
                nonce: 19,
            },
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::ConfigMismatch);
        assert_eq!(response.target_virtual_server, 11);
        assert_eq!(response.nonce, 19);
    }
}

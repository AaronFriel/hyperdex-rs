use std::net::SocketAddr;

use anyhow::Result;
use legacy_protocol::{
    config_mismatch_response, encode_request_frame, encode_response_frame, RequestHeader,
    ResponseHeader, BUSYBEE_HEADER_SIZE, LEGACY_REQUEST_HEADER_SIZE,
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
        let (header, body) = read_request_frame(&mut stream).await?;
        let (response, response_body) = handler(header, body).await?;

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
        H: Fn(RequestHeader, Vec<u8>) -> F,
        F: std::future::Future<Output = Result<(ResponseHeader, Vec<u8>)>>,
    {
        loop {
            self.serve_once_with(&handler).await?;
        }
    }
}

async fn read_request_frame(
    stream: &mut tokio::net::TcpStream,
) -> Result<(RequestHeader, Vec<u8>)> {
    let mut prefix = [0u8; BUSYBEE_HEADER_SIZE];
    stream.read_exact(&mut prefix).await?;

    let payload_len = u32::from_be_bytes(prefix) as usize;
    let mut bytes = vec![0u8; BUSYBEE_HEADER_SIZE + payload_len];
    bytes[..BUSYBEE_HEADER_SIZE].copy_from_slice(&prefix);
    stream.read_exact(&mut bytes[BUSYBEE_HEADER_SIZE..]).await?;

    let header = RequestHeader::decode(&bytes[..LEGACY_REQUEST_HEADER_SIZE])?;
    let body = bytes[LEGACY_REQUEST_HEADER_SIZE..].to_vec();
    Ok((header, body))
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
    let payload_len = u32::from_be_bytes(prefix) as usize;
    let mut response = vec![0u8; BUSYBEE_HEADER_SIZE + payload_len];
    response[..BUSYBEE_HEADER_SIZE].copy_from_slice(&prefix);
    stream
        .read_exact(&mut response[BUSYBEE_HEADER_SIZE..])
        .await?;
    let header = ResponseHeader::decode(&response)?;
    let body = response[legacy_protocol::LEGACY_RESPONSE_HEADER_SIZE..].to_vec();
    Ok((header, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use legacy_protocol::{
        AtomicRequest, AtomicResponse, CountRequest, CountResponse, GetAttribute, GetValue,
        LegacyMessageType, LegacyReturnCode, RequestHeader,
        LEGACY_ATOMIC_FLAG_HAS_ATTRIBUTES,
    };

    #[tokio::test]
    async fn serve_once_returns_config_mismatch() {
        let frontend = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move { frontend.serve_once().await.unwrap() });
        let (response, body) = request_once(
            address,
            RequestHeader {
                message_type: LegacyMessageType::ReqGet,
                flags: 0,
                version: 7,
                target_virtual_server: 11,
                nonce: 19,
            },
            &[],
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::ConfigMismatch);
        assert_eq!(response.target_virtual_server, 11);
        assert_eq!(response.nonce, 19);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn serve_once_with_handles_count() {
        let frontend = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move {
            frontend
                .serve_once_with(|header, body| async move {
                    assert_eq!(header.message_type, LegacyMessageType::ReqCount);
                    let request = CountRequest::decode_body(&body).unwrap();
                    assert_eq!(request.space, "profiles");

                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespCount,
                            target_virtual_server: header.target_virtual_server,
                            nonce: header.nonce,
                        },
                        CountResponse { count: 7 }.encode_body().to_vec(),
                    ))
                })
                .await
                .unwrap()
        });

        let (response, body) = request_once(
            address,
            RequestHeader {
                message_type: LegacyMessageType::ReqCount,
                flags: 0,
                version: 7,
                target_virtual_server: 11,
                nonce: 19,
            },
            &CountRequest {
                space: "profiles".to_owned(),
            }
            .encode_body(),
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::RespCount);
        assert_eq!(CountResponse::decode_body(&body).unwrap().count, 7);
    }

    #[tokio::test]
    async fn serve_once_with_handles_atomic() {
        let frontend = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move {
            frontend
                .serve_once_with(|header, body| async move {
                    assert_eq!(header.message_type, LegacyMessageType::ReqAtomic);
                    let request = AtomicRequest::decode_body(&body).unwrap();
                    assert_eq!(request.key, b"ada".to_vec());
                    assert_eq!(request.flags, LEGACY_ATOMIC_FLAG_HAS_ATTRIBUTES);
                    assert_eq!(
                        request.attributes,
                        vec![GetAttribute {
                            name: "first".to_owned(),
                            value: GetValue::String("Ada".to_owned()),
                        }]
                    );

                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespAtomic,
                            target_virtual_server: header.target_virtual_server,
                            nonce: header.nonce,
                        },
                        AtomicResponse {
                            status: LegacyReturnCode::Success,
                        }
                        .encode_body()
                        .to_vec(),
                    ))
                })
                .await
                .unwrap()
        });

        let (response, body) = request_once(
            address,
            RequestHeader {
                message_type: LegacyMessageType::ReqAtomic,
                flags: 0,
                version: 7,
                target_virtual_server: 11,
                nonce: 19,
            },
            &AtomicRequest {
                flags: LEGACY_ATOMIC_FLAG_HAS_ATTRIBUTES,
                key: b"ada".to_vec(),
                attributes: vec![GetAttribute {
                    name: "first".to_owned(),
                    value: GetValue::String("Ada".to_owned()),
                }],
            }
            .encode_body(),
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::RespAtomic);
        assert_eq!(
            AtomicResponse::decode_body(&body).unwrap().status,
            LegacyReturnCode::Success
        );
    }
}

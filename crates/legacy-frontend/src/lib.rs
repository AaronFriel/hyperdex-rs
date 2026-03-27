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

    let total_len = (u32::from_be_bytes(prefix) & 0x00ff_ffff) as usize;
    if total_len < BUSYBEE_HEADER_SIZE {
        anyhow::bail!("busybee frame size {total_len} is too small");
    }

    let mut bytes = vec![0u8; total_len];
    bytes[..BUSYBEE_HEADER_SIZE].copy_from_slice(&prefix);
    stream.read_exact(&mut bytes[BUSYBEE_HEADER_SIZE..total_len]).await?;

    let header = RequestHeader::decode(&bytes[..LEGACY_REQUEST_HEADER_SIZE])?;
    let handler_body_offset = LEGACY_REQUEST_HEADER_SIZE - std::mem::size_of::<u64>();
    let body = bytes[handler_body_offset..].to_vec();
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
mod tests {
    use super::*;
    use legacy_protocol::{
        decode_protocol_atomic_request, decode_protocol_atomic_response,
        decode_protocol_count_request, decode_protocol_count_response,
        decode_protocol_search_item, decode_protocol_search_start,
        encode_protocol_atomic_request, encode_protocol_count_request,
        encode_protocol_search_item, encode_protocol_search_start, ProtocolAttributeCheck,
        ProtocolFuncall, ProtocolKeyChange, ProtocolSearchItem, ProtocolSearchStart,
        LegacyMessageType, RequestHeader, ResponseHeader, FUNC_SET, HYPERDATATYPE_INT64,
        HYPERDATATYPE_STRING, HYPERPREDICATE_GREATER_EQUAL,
    };

    fn decode_handler_nonce(body: &[u8]) -> (u64, &[u8]) {
        let nonce = u64::from_be_bytes(body[..8].try_into().expect("fixed-width slice"));
        (nonce, &body[8..])
    }

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
                    let (nonce, request_body) = decode_handler_nonce(&body);
                    assert_eq!(nonce, header.nonce);
                    let request = decode_protocol_count_request(request_body).unwrap();
                    assert!(request.is_empty());

                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespCount,
                            target_virtual_server: header.target_virtual_server,
                            nonce: header.nonce,
                        },
                        legacy_protocol::encode_protocol_count_response(7).to_vec(),
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
            &encode_protocol_count_request(&[]),
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::RespCount);
        assert_eq!(decode_protocol_count_response(&body).unwrap(), 7);
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
                    let (nonce, request_body) = decode_handler_nonce(&body);
                    assert_eq!(nonce, header.nonce);
                    let request = decode_protocol_atomic_request(request_body).unwrap();
                    assert_eq!(request.key, b"ada".to_vec());
                    assert!(!request.erase);
                    assert!(!request.fail_if_not_found);
                    assert!(!request.fail_if_found);
                    assert!(request.checks.is_empty());
                    assert_eq!(
                        request.funcalls,
                        vec![ProtocolFuncall {
                            attr: 1,
                            name: FUNC_SET,
                            arg1: b"Ada".to_vec(),
                            arg1_datatype: HYPERDATATYPE_STRING,
                            arg2: Vec::new(),
                            arg2_datatype: 0,
                        }]
                    );

                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespAtomic,
                            target_virtual_server: header.target_virtual_server,
                            nonce: header.nonce,
                        },
                        legacy_protocol::encode_protocol_atomic_response(
                            legacy_protocol::LegacyReturnCode::Success as u16,
                        )
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
            &encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_SET,
                    arg1: b"Ada".to_vec(),
                    arg1_datatype: HYPERDATATYPE_STRING,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::RespAtomic);
        assert_eq!(
            decode_protocol_atomic_response(&body).unwrap(),
            legacy_protocol::LegacyReturnCode::Success as u16
        );
    }

    #[tokio::test]
    async fn serve_once_with_handles_search_start() {
        let frontend = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move {
            frontend
                .serve_once_with(|header, body| async move {
                    assert_eq!(header.message_type, LegacyMessageType::ReqSearchStart);
                    let (nonce, request_body) = decode_handler_nonce(&body);
                    assert_eq!(nonce, header.nonce);
                    let request = decode_protocol_search_start(request_body).unwrap();
                    assert_eq!(request.search_id, 41);
                    assert_eq!(request.checks.len(), 1);
                    assert_eq!(request.checks[0].attr, 2);
                    assert_eq!(request.checks[0].predicate, HYPERPREDICATE_GREATER_EQUAL);
                    assert_eq!(request.checks[0].datatype, HYPERDATATYPE_INT64);
                    assert_eq!(request.checks[0].value, 2_i64.to_le_bytes().to_vec());

                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespSearchItem,
                            target_virtual_server: header.target_virtual_server,
                            nonce: header.nonce,
                        },
                        encode_protocol_search_item(&ProtocolSearchItem {
                            key: b"ada".to_vec(),
                            values: vec![b"Ada".to_vec(), 2_i64.to_le_bytes().to_vec()],
                        }),
                    ))
                })
                .await
                .unwrap()
        });

        let (response, body) = request_once(
            address,
            RequestHeader {
                message_type: LegacyMessageType::ReqSearchStart,
                flags: 0,
                version: 7,
                target_virtual_server: 11,
                nonce: 19,
            },
            &encode_protocol_search_start(&ProtocolSearchStart {
                search_id: 41,
                checks: vec![ProtocolAttributeCheck {
                    attr: 2,
                    value: 2_i64.to_le_bytes().to_vec(),
                    datatype: HYPERDATATYPE_INT64,
                    predicate: HYPERPREDICATE_GREATER_EQUAL,
                }],
            }),
        )
        .await
        .unwrap();

        server.await.unwrap();

        assert_eq!(response.message_type, LegacyMessageType::RespSearchItem);
        let item = decode_protocol_search_item(&body).unwrap();
        assert_eq!(item.key, b"ada".to_vec());
        assert_eq!(item.values[0], b"Ada".to_vec());
    }
}

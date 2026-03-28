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
    bytes.iter()
        .take(width)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
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
                if let Err(err) =
                    serve_connection_with(&mut stream, local_server_id, handler).await
                {
                    maybe_capture_legacy_frontend_event(&format!(
                        "connection_error {err:#}"
                    ));
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
        stream.read_exact(&mut bytes[BUSYBEE_HEADER_SIZE..total_len]).await?;
        maybe_capture_legacy_frontend_event(&format!(
            "frame flags=0x{raw_header:08x} len={} prefix={}",
            bytes.len(),
            capture_hex_prefix(&bytes, 32)
        ));

        if raw_header & BUSYBEE_HEADER_IDENTIFY != 0 {
            let remote_server_id = if bytes.len() >= BUSYBEE_HEADER_SIZE + 16 {
                u64::from_be_bytes(bytes[12..20].try_into().expect("fixed-width slice"))
            } else {
                0
            };
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
mod tests {
    use super::*;
    use legacy_protocol::{
        decode_protocol_atomic_request, decode_protocol_atomic_response, decode_protocol_count_request,
        decode_protocol_count_response,
        decode_protocol_search_item, decode_protocol_search_start,
        encode_identify_frame, encode_protocol_atomic_request, encode_protocol_count_request,
        encode_protocol_search_item, encode_protocol_search_start, ProtocolAttributeCheck,
        ProtocolFuncall, ProtocolKeyChange, ProtocolSearchItem, ProtocolSearchStart,
        LegacyMessageType, RequestHeader, ResponseHeader, FUNC_SET, HYPERDATATYPE_INT64,
        HYPERDATATYPE_STRING, HYPERPREDICATE_GREATER_EQUAL,
    };

    async fn read_raw_frame(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut prefix = [0_u8; BUSYBEE_HEADER_SIZE];
        stream.read_exact(&mut prefix).await.unwrap();
        let total_len = (u32::from_be_bytes(prefix) & 0x00ff_ffff) as usize;
        let mut bytes = vec![0_u8; total_len];
        bytes[..BUSYBEE_HEADER_SIZE].copy_from_slice(&prefix);
        stream
            .read_exact(&mut bytes[BUSYBEE_HEADER_SIZE..total_len])
            .await
            .unwrap();
        bytes
    }

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
    async fn serve_once_with_handles_busybee_identify_before_request() {
        let frontend = LegacyFrontend::bind_with_server_id("127.0.0.1:0".parse().unwrap(), 7)
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

        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream
            .write_all(&encode_identify_frame(0, 0))
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let identify_response = read_raw_frame(&mut stream).await;
        assert_eq!(identify_response, encode_identify_frame(7, 0));

        stream
            .write_all(&encode_request_frame(
                RequestHeader {
                    message_type: LegacyMessageType::ReqCount,
                    flags: 0,
                    version: 7,
                    target_virtual_server: 11,
                    nonce: 19,
                },
                &encode_protocol_count_request(&[]),
            ))
            .await
            .unwrap();
        stream.flush().await.unwrap();

        let response = read_raw_frame(&mut stream).await;
        let header = ResponseHeader::decode(&response).unwrap();
        let body = response[legacy_protocol::LEGACY_RESPONSE_HEADER_SIZE..].to_vec();

        server.await.unwrap();

        assert_eq!(header.message_type, LegacyMessageType::RespCount);
        assert_eq!(decode_protocol_count_response(&body).unwrap(), 7);
    }

    #[tokio::test]
    async fn serve_forever_with_keeps_connection_open_for_multiple_requests() {
        let frontend = LegacyFrontend::bind_with_server_id("127.0.0.1:0".parse().unwrap(), 7)
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move {
            frontend
                .serve_forever_with(|header, body| async move {
                    assert_eq!(header.message_type, LegacyMessageType::ReqCount);
                    let (nonce, request_body) = decode_handler_nonce(&body);
                    let request = decode_protocol_count_request(request_body).unwrap();
                    assert!(request.is_empty());
                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespCount,
                            target_virtual_server: header.target_virtual_server,
                            nonce,
                        },
                        legacy_protocol::encode_protocol_count_response(nonce).to_vec(),
                    ))
                })
                .await
                .unwrap()
        });

        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream
            .write_all(&encode_identify_frame(0, 0))
            .await
            .unwrap();
        stream.flush().await.unwrap();
        let _ = read_raw_frame(&mut stream).await;

        for nonce in [19_u64, 29_u64] {
            stream
                .write_all(&encode_request_frame(
                    RequestHeader {
                        message_type: LegacyMessageType::ReqCount,
                        flags: 0,
                        version: 7,
                        target_virtual_server: 11,
                        nonce,
                    },
                    &encode_protocol_count_request(&[]),
                ))
                .await
                .unwrap();
            stream.flush().await.unwrap();

            let response = read_raw_frame(&mut stream).await;
            let header = ResponseHeader::decode(&response).unwrap();
            let body = response[legacy_protocol::LEGACY_RESPONSE_HEADER_SIZE..].to_vec();
            assert_eq!(header.message_type, LegacyMessageType::RespCount);
            assert_eq!(header.nonce, nonce);
            assert_eq!(decode_protocol_count_response(&body).unwrap(), nonce);
        }

        drop(stream);
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn serve_forever_with_accepts_second_connection_while_first_stays_open() {
        let frontend = LegacyFrontend::bind_with_server_id("127.0.0.1:0".parse().unwrap(), 7)
            .await
            .unwrap();
        let address = frontend.local_addr().unwrap();

        let server = tokio::spawn(async move {
            frontend
                .serve_forever_with(|header, body| async move {
                    assert_eq!(header.message_type, LegacyMessageType::ReqCount);
                    let (nonce, request_body) = decode_handler_nonce(&body);
                    let request = decode_protocol_count_request(request_body).unwrap();
                    assert!(request.is_empty());
                    Ok((
                        ResponseHeader {
                            message_type: LegacyMessageType::RespCount,
                            target_virtual_server: header.target_virtual_server,
                            nonce,
                        },
                        legacy_protocol::encode_protocol_count_response(nonce).to_vec(),
                    ))
                })
                .await
                .unwrap()
        });

        let mut stream1 = tokio::net::TcpStream::connect(address).await.unwrap();
        stream1
            .write_all(&encode_identify_frame(0, 0))
            .await
            .unwrap();
        stream1.flush().await.unwrap();
        let _ = read_raw_frame(&mut stream1).await;
        stream1
            .write_all(&encode_request_frame(
                RequestHeader {
                    message_type: LegacyMessageType::ReqCount,
                    flags: 0,
                    version: 7,
                    target_virtual_server: 11,
                    nonce: 19,
                },
                &encode_protocol_count_request(&[]),
            ))
            .await
            .unwrap();
        stream1.flush().await.unwrap();
        let response1 = read_raw_frame(&mut stream1).await;
        let header1 = ResponseHeader::decode(&response1).unwrap();
        assert_eq!(header1.nonce, 19);

        let mut stream2 = tokio::net::TcpStream::connect(address).await.unwrap();
        stream2
            .write_all(&encode_request_frame(
                RequestHeader {
                    message_type: LegacyMessageType::ReqCount,
                    flags: 0,
                    version: 7,
                    target_virtual_server: 11,
                    nonce: 29,
                },
                &encode_protocol_count_request(&[]),
            ))
            .await
            .unwrap();
        stream2.flush().await.unwrap();

        let response2 = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            read_raw_frame(&mut stream2),
        )
        .await
        .expect("second connection should be served while the first stays open");
        let header2 = ResponseHeader::decode(&response2).unwrap();
        let body2 = response2[legacy_protocol::LEGACY_RESPONSE_HEADER_SIZE..].to_vec();
        assert_eq!(header2.nonce, 29);
        assert_eq!(decode_protocol_count_response(&body2).unwrap(), 29);

        drop(stream1);
        drop(stream2);
        server.abort();
        let _ = server.await;
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

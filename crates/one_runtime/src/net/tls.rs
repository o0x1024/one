use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;

use one_core::{JsValue, OneResult};
use one_vm::Vm;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};

pub fn install_tls(vm: &mut Vm) {
    vm.register_host_fn("tls.getCertificate", tls_get_certificate);
    vm.register_host_fn("tls.checkExpiry", tls_check_expiry);
    vm.register_host_fn("tls.getProtocol", tls_get_protocol);
}

fn make_tls_config() -> Arc<ClientConfig> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Arc::new(config)
}

struct TlsInfo {
    peer_certs: Vec<Vec<u8>>,
    protocol_version: String,
}

fn connect_tls(host: &str, port: u16) -> Result<TlsInfo, String> {
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| format!("invalid server name: {e}"))?;

    let config = make_tls_config();
    let conn =
        ClientConnection::new(config, server_name).map_err(|e| format!("TLS error: {e}"))?;

    let tcp = TcpStream::connect_timeout(
        &format!("{host}:{port}")
            .to_socket_addrs()
            .map_err(|e| format!("DNS error: {e}"))?
            .next()
            .ok_or("no address found")?,
        std::time::Duration::from_secs(5),
    )
    .map_err(|e| format!("TCP connect error: {e}"))?;

    tcp.set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .ok();

    let mut tls = StreamOwned::new(conn, tcp);
    let mut buf = [0u8; 1];
    let _ = tls.read(&mut buf);

    let peer_certs: Vec<Vec<u8>> = tls
        .conn
        .peer_certificates()
        .map(|certs| certs.iter().map(|c| c.as_ref().to_vec()).collect())
        .unwrap_or_default();

    let proto = match tls.conn.protocol_version() {
        Some(rustls::ProtocolVersion::TLSv1_2) => "TLSv1.2",
        Some(rustls::ProtocolVersion::TLSv1_3) => "TLSv1.3",
        Some(_) => "unknown",
        None => "none",
    };

    Ok(TlsInfo {
        peer_certs,
        protocol_version: proto.into(),
    })
}

fn tls_get_certificate(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let host = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let port = args.get(1).map(|v| v.to_number() as u16).unwrap_or(443);

    match connect_tls(&host, port) {
        Ok(info) => {
            if let Some(der) = info.peer_certs.first() {
                match x509_parser::parse_x509_certificate(der) {
                    Ok((_, cert)) => {
                        let subject = vm.alloc_string(cert.subject().to_string());
                        let issuer = vm.alloc_string(cert.issuer().to_string());
                        let not_before =
                            vm.alloc_string(cert.validity().not_before.to_rfc2822().unwrap_or_default());
                        let not_after =
                            vm.alloc_string(cert.validity().not_after.to_rfc2822().unwrap_or_default());
                        let serial =
                            vm.alloc_string(cert.raw_serial_as_string());
                        let protocol = vm.alloc_string(info.protocol_version);

                        let result = vm.create_object_from_pairs(&[
                            ("ok".into(), JsValue::from_bool(true)),
                            ("subject".into(), subject),
                            ("issuer".into(), issuer),
                            ("notBefore".into(), not_before),
                            ("notAfter".into(), not_after),
                            ("serialNumber".into(), serial),
                            ("protocol".into(), protocol),
                        ]);
                        Ok(result)
                    }
                    Err(e) => {
                        let err = vm.alloc_string(format!("x509 parse error: {e}"));
                        Ok(vm.create_object_from_pairs(&[
                            ("ok".into(), JsValue::from_bool(false)),
                            ("error".into(), err),
                        ]))
                    }
                }
            } else {
                let err = vm.alloc_string("no peer certificate".into());
                Ok(vm.create_object_from_pairs(&[
                    ("ok".into(), JsValue::from_bool(false)),
                    ("error".into(), err),
                ]))
            }
        }
        Err(e) => {
            let err = vm.alloc_string(e);
            Ok(vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]))
        }
    }
}

fn tls_check_expiry(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let host = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let port = args.get(1).map(|v| v.to_number() as u16).unwrap_or(443);

    match connect_tls(&host, port) {
        Ok(info) => {
            if let Some(der) = info.peer_certs.first() {
                match x509_parser::parse_x509_certificate(der) {
                    Ok((_, cert)) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let expires = cert.validity().not_after.timestamp() as u64;
                        let days_remaining = if expires > now {
                            ((expires - now) / 86400) as i32
                        } else {
                            -(((now - expires) / 86400) as i32)
                        };

                        let expired = JsValue::from_bool(days_remaining < 0);
                        let days = JsValue::from_i32(days_remaining);
                        let not_after =
                            vm.alloc_string(cert.validity().not_after.to_rfc2822().unwrap_or_default());

                        let result = vm.create_object_from_pairs(&[
                            ("ok".into(), JsValue::from_bool(true)),
                            ("expired".into(), expired),
                            ("daysRemaining".into(), days),
                            ("notAfter".into(), not_after),
                        ]);
                        Ok(result)
                    }
                    Err(e) => {
                        let err = vm.alloc_string(format!("x509 parse error: {e}"));
                        Ok(vm.create_object_from_pairs(&[
                            ("ok".into(), JsValue::from_bool(false)),
                            ("error".into(), err),
                        ]))
                    }
                }
            } else {
                let err = vm.alloc_string("no peer certificate".into());
                Ok(vm.create_object_from_pairs(&[
                    ("ok".into(), JsValue::from_bool(false)),
                    ("error".into(), err),
                ]))
            }
        }
        Err(e) => {
            let err = vm.alloc_string(e);
            Ok(vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]))
        }
    }
}

fn tls_get_protocol(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let host = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let port = args.get(1).map(|v| v.to_number() as u16).unwrap_or(443);

    match connect_tls(&host, port) {
        Ok(info) => Ok(vm.alloc_string(info.protocol_version)),
        Err(e) => {
            let err = vm.alloc_string(e);
            Ok(vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]))
        }
    }
}

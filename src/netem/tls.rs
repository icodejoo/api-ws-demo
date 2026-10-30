//! 到上游的 TLS 客户端连接（rustls + ring + webpki 根证书）；未启用 `tls` 特性时返回不支持。
use super::relay::Duplex;
use std::io;
use tokio::net::TcpStream;

#[cfg(feature = "tls")]
mod imp {
    use super::*;
    use std::{
        pin::Pin,
        sync::{Arc, OnceLock},
        task::{Context, Poll},
    };
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
    use tokio_rustls::{
        client::TlsStream,
        rustls::{self, pki_types::ServerName, ClientConfig, RootCertStore},
        TlsConnector,
    };

    /// 进程内共享的客户端配置。
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

    /// 惰性构建配置：ring 后端、安全默认协议版本、webpki 内置根证书。
    fn config() -> Arc<ClientConfig> {
        CONFIG
            .get_or_init(|| {
                let roots = RootCertStore {
                    roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
                };
                let config = ClientConfig::builder_with_provider(Arc::new(
                    rustls::crypto::ring::default_provider(),
                ))
                .with_safe_default_protocol_versions()
                .expect("ring provider supports default protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
                Arc::new(config)
            })
            .clone()
    }

    /// 校验域名可作为 SNI。
    pub(in super::super) fn validate(name: &str) -> Result<(), String> {
        ServerName::try_from(name.to_string())
            .map(|_| ())
            .map_err(|_| format!("invalid TLS server name: {name}"))
    }

    /// 包装 TLS 流，使其满足 `Duplex`。
    struct Wrapped(TlsStream<TcpStream>);

    impl AsyncRead for Wrapped {
        /// 转发读取。
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for Wrapped {
        /// 转发写入。
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.0).poll_write(cx, buf)
        }
        /// 转发刷新。
        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_flush(cx)
        }
        /// 转发关闭（发送 close_notify）。
        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_shutdown(cx)
        }
    }

    impl Duplex for Wrapped {
        /// 对底层 TCP 注入 RST。
        fn reset(&self) {
            Duplex::reset(self.0.get_ref().0);
        }
    }

    /// 在已连上的 TCP 上以 `sni` 完成 TLS 握手。
    pub(in super::super) async fn connect(
        tcp: TcpStream,
        sni: &str,
    ) -> io::Result<Box<dyn Duplex>> {
        let name = ServerName::try_from(sni.to_string())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let stream = TlsConnector::from(config()).connect(name, tcp).await?;
        Ok(Box::new(Wrapped(stream)))
    }
}

#[cfg(not(feature = "tls"))]
mod imp {
    use super::*;

    /// 未启用 tls 特性，一律拒绝。
    pub(in super::super) fn validate(_name: &str) -> Result<(), String> {
        Err("https upstream requires the `tls` feature".into())
    }

    /// 未启用 tls 特性，握手不可用。
    pub(in super::super) async fn connect(
        _tcp: TcpStream,
        _sni: &str,
    ) -> io::Result<Box<dyn Duplex>> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

pub(super) use imp::{connect, validate};

#[cfg(feature = "http")]
pub use cclover_http::{DEFAULT_HTTP_BIND, HttpConfig, HttpServer};

#[cfg(not(feature = "http"))]
mod disabled {
    use std::io;
    use std::net::{IpAddr, SocketAddr};

    use cclover_runtime::{Shutdown, StateSource};

    pub const DEFAULT_HTTP_BIND: SocketAddr =
        SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 9847);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct HttpConfig {
        pub bind: SocketAddr,
    }

    impl Default for HttpConfig {
        fn default() -> Self {
            Self {
                bind: DEFAULT_HTTP_BIND,
            }
        }
    }

    pub struct HttpServer;

    impl HttpServer {
        pub fn start(
            _config: HttpConfig,
            _states: StateSource,
            _shutdown: Shutdown,
        ) -> io::Result<Self> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "HTTP monitoring is unavailable in this build; rebuild with the `http` feature",
            ))
        }
    }
}

#[cfg(not(feature = "http"))]
pub use disabled::{DEFAULT_HTTP_BIND, HttpConfig, HttpServer};

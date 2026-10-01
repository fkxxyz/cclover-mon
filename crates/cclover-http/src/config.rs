use std::net::{IpAddr, SocketAddr};

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

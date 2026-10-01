#![deny(unsafe_code)]

mod api;
mod assets;
mod config;
mod request;
mod routes;
mod server;
mod sse;
mod state;

pub use config::{DEFAULT_HTTP_BIND, HttpConfig};
pub use server::HttpServer;

#[cfg(test)]
mod tests;

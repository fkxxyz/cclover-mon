#![deny(unsafe_code)]

#[cfg(all(target_arch = "wasm32", feature = "http"))]
fn main() {
    if let Err(error) = cclover_mon::web_client::run() {
        web_sys::console::error_1(&error);
    }
}

#[cfg(not(all(target_arch = "wasm32", feature = "http")))]
fn main() {
    eprintln!("cclover-mon-web is only intended for wasm32-unknown-unknown");
}

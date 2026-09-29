#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
fn main() {
    if let Err(error) = cclover_mon::web_client::run() {
        web_sys::console::error_1(&error);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("cclover-mon-web is only intended for wasm32-unknown-unknown");
}

#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
fn main() -> iced::Result {
    cclover_mon::web_client::run()
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("cclover-mon-web is only intended for wasm32-unknown-unknown");
}

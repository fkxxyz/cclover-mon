#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("windows-etw-semantic must run on Windows");
    std::process::exit(2);
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = cclover_platform::run_windows_etw_semantic_validation() {
        eprintln!("windows-etw-semantic: {error}");
        std::process::exit(1);
    }
    println!("windows-etw-semantic: all completion semantics validated");
}

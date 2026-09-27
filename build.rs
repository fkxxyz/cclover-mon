use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target arch");
    if target_arch == "wasm32" {
        return;
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    if env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some(std::ffi::OsStr::new("linux")) {
        build_linux_bpf(&out, &target_arch);
    }
    build_web_bundle(&out);
}

fn build_linux_bpf(out: &Path, target_arch: &str) {
    let bpf_arch = match target_arch {
        "x86" | "x86_64" => "x86",
        "aarch64" => "arm64",
        "arm" => "arm",
        "riscv64" => "riscv",
        "powerpc64" => "powerpc",
        "s390x" => "s390",
        other => panic!("unsupported Linux eBPF target architecture: {other}"),
    };

    compile_bpf(
        Path::new("bpf/disk_attribution.bpf.c"),
        &out.join("disk_attribution.bpf.o"),
        bpf_arch,
    );
    compile_bpf(
        Path::new("bpf/network_attribution.bpf.c"),
        &out.join("network_attribution.bpf.o"),
        bpf_arch,
    );
    println!("cargo:rustc-link-lib=bpf");
}

fn build_web_bundle(out: &Path) {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=src");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let web_target_dir = out.join("web-target");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(&manifest_dir)
        .args([
            "build",
            "--release",
            "--locked",
            "--target",
            "wasm32-unknown-unknown",
            "--bin",
            "cclover-mon-web",
        ])
        .env("CARGO_TARGET_DIR", &web_target_dir)
        .status()
        .unwrap_or_else(|error| panic!("failed to build embedded web frontend: {error}"));
    assert!(
        status.success(),
        "failed to build embedded web frontend; install the target with `rustup target add wasm32-unknown-unknown`"
    );

    let wasm = web_target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("cclover-mon-web.wasm");
    let web_out = out.join("web");
    let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();
    bindgen
        .input_path(&wasm)
        .out_name("cclover_mon_web")
        .typescript(false);
    bindgen
        .web(true)
        .expect("failed to select wasm-bindgen web output");
    bindgen
        .generate(&web_out)
        .unwrap_or_else(|error| panic!("failed to generate embedded web bindings: {error}"));
}

fn compile_bpf(source: &Path, output: &Path, bpf_arch: &str) {
    println!("cargo:rerun-if-changed={}", source.display());
    let clang = env::var_os("BPF_CLANG").unwrap_or_else(|| "clang".into());
    let target_arch_define = format!("-D__TARGET_ARCH_{bpf_arch}");
    let status = Command::new(clang)
        .args([
            "-target",
            "bpf",
            "-O2",
            "-g",
            "-Wall",
            "-Werror",
            &target_arch_define,
            "-c",
        ])
        .arg(source)
        .arg("-o")
        .arg(output)
        .status()
        .unwrap_or_else(|error| panic!("failed to invoke clang for {}: {error}", source.display()));
    assert!(
        status.success(),
        "clang failed to compile {}",
        source.display()
    );
}

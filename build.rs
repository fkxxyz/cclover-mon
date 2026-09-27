use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if env::var_os("CARGO_CFG_TARGET_OS").as_deref() != Some(std::ffi::OsStr::new("linux")) {
        return;
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target arch");
    let bpf_arch = match target_arch.as_str() {
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

use std::env;
use std::fs;
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

    let disk_source = Path::new("bpf/disk_attribution.bpf.c");
    let network_source = Path::new("bpf/network_attribution.bpf.c");
    let clang = env::var_os("BPF_CLANG").unwrap_or_else(|| "clang".into());

    compile_bpf(
        disk_source,
        &out.join("disk_attribution.bpf.o"),
        bpf_arch,
        &clang,
    );
    compile_bpf(
        network_source,
        &out.join("network_attribution.bpf.o"),
        bpf_arch,
        &clang,
    );
    write_bpf_abi_layouts(out, bpf_arch, &clang, disk_source, network_source);
    println!("cargo:rustc-link-lib=bpf");
}

fn build_web_bundle(out: &Path) {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=src");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let web_target_dir = out.join("web-target");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
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
        .env("CARGO_TARGET_DIR", &web_target_dir);
    isolate_nested_cargo(&mut command);
    let status = command
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

fn isolate_nested_cargo(command: &mut Command) {
    command
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_LINKER");

    for (key, _) in env::vars_os() {
        let key_text = key.to_string_lossy();
        if key_text.starts_with("CARGO_CFG_") || key_text.starts_with("CARGO_FEATURE_") {
            command.env_remove(key);
        }
    }
}

fn compile_bpf(source: &Path, output: &Path, bpf_arch: &str, clang: &std::ffi::OsStr) {
    println!("cargo:rerun-if-changed={}", source.display());
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

struct RecordLayout<'a> {
    rust_module: &'a str,
    c_struct: &'a str,
    fields: &'a [&'a str],
}

fn write_bpf_abi_layouts(
    out: &Path,
    bpf_arch: &str,
    clang: &std::ffi::OsStr,
    disk_source: &Path,
    network_source: &Path,
) {
    let disk_layouts = [
        RecordLayout {
            rust_module: "disk_key",
            c_struct: "disk_key",
            fields: &["tgid", "dev", "direction", "pad"],
        },
        RecordLayout {
            rust_module: "disk_counter_value",
            c_struct: "counter_value",
            fields: &["process_start_time", "bytes"],
        },
    ];
    let network_layouts = [
        RecordLayout {
            rust_module: "network_key",
            c_struct: "network_key",
            fields: &["tgid", "ifindex", "direction", "pad"],
        },
        RecordLayout {
            rust_module: "network_counter_value",
            c_struct: "counter_value",
            fields: &["process_start_time", "bytes"],
        },
    ];

    let mut generated = String::from("// Generated from clang's BPF-target record layouts.\n");
    append_source_layouts(&mut generated, clang, bpf_arch, disk_source, &disk_layouts);
    append_source_layouts(
        &mut generated,
        clang,
        bpf_arch,
        network_source,
        &network_layouts,
    );
    fs::write(out.join("bpf_abi_layout.rs"), generated)
        .expect("failed to write generated BPF ABI layout constants");
}

fn append_source_layouts(
    generated: &mut String,
    clang: &std::ffi::OsStr,
    bpf_arch: &str,
    source: &Path,
    layouts: &[RecordLayout<'_>],
) {
    let target_arch_define = format!("-D__TARGET_ARCH_{bpf_arch}");
    let output = Command::new(clang)
        .args([
            "-target",
            "bpf",
            &target_arch_define,
            "-Xclang",
            "-fdump-record-layouts-complete",
            "-fsyntax-only",
        ])
        .arg(source)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to inspect BPF ABI layouts in {}: {error}",
                source.display()
            )
        });
    assert!(
        output.status.success(),
        "clang failed to inspect BPF ABI layouts in {}: {}{}",
        source.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let dump = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for layout in layouts {
        let (size, align, offsets) = parse_record_layout(&dump, layout.c_struct, layout.fields)
            .unwrap_or_else(|| {
                panic!(
                    "clang record-layout output for {} does not contain complete struct {} ABI data",
                    source.display(),
                    layout.c_struct
                )
            });
        generated.push_str(&format!("pub(crate) mod {} {{\n", layout.rust_module));
        generated.push_str(&format!("    pub(crate) const SIZE: usize = {size};\n"));
        generated.push_str(&format!("    pub(crate) const ALIGN: usize = {align};\n"));
        for (field, offset) in layout.fields.iter().zip(offsets) {
            generated.push_str(&format!(
                "    pub(crate) const {}_OFFSET: usize = {offset};\n",
                field.to_ascii_uppercase()
            ));
        }
        generated.push_str("}\n");
    }
}

fn parse_record_layout(
    dump: &str,
    struct_name: &str,
    fields: &[&str],
) -> Option<(usize, usize, Vec<usize>)> {
    let header = format!("| struct {struct_name}");
    let mut lines = dump.lines().skip_while(|line| !line.contains(&header));
    lines.next()?;

    let mut offsets = vec![None; fields.len()];
    for line in lines {
        let (_, detail) = line.split_once('|')?;
        let detail = detail.trim();
        if detail.starts_with("[sizeof=") {
            let values = detail.strip_prefix("[sizeof=")?.strip_suffix(']')?;
            let (size, align) = values.split_once(", align=")?;
            let offsets = offsets.into_iter().collect::<Option<Vec<_>>>()?;
            return Some((size.parse().ok()?, align.parse().ok()?, offsets));
        }

        let (offset, _) = line.split_once('|')?;
        let offset = offset.trim().parse::<usize>().ok()?;
        let field = detail.split_whitespace().last()?;
        if let Some(index) = fields.iter().position(|expected| *expected == field) {
            offsets[index] = Some(offset);
        }
    }
    None
}

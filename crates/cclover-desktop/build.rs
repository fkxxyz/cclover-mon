use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

macro_rules! define_native_abi {
    (
        constants {
            $( $rust_const:ident => $c_const:ident = $value:expr; )*
        }
        structs {
            $(
                $rust_struct:ident => $c_struct:ident {
                    $( $field:ident : $field_type:ident, )*
                }
            )*
        }
    ) => {{
        let mut header = String::from(
            "#ifndef CCLOVER_NATIVE_SCENE_H\n#define CCLOVER_NATIVE_SCENE_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n",
        );

        $(
            writeln!(
                &mut header,
                "#define {} {}u",
                stringify!($c_const),
                $value
            )
            .expect("write native ABI constant");
        )*
        header.push('\n');

        $(
            header.push_str("typedef struct {\n");
            $(
                writeln!(
                    &mut header,
                    "    {}",
                    c_field(stringify!($field), stringify!($field_type))
                )
                .expect("write native ABI field");
            )*
            writeln!(&mut header, "}} {};\n", stringify!($c_struct))
                .expect("write native ABI struct");
        )*

        header.push_str("#endif\n");
        header
    }};
}

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    generate_native_header(&out_dir);
    println!("cargo:rerun-if-changed=src/native_abi_spec.rs");

    match target_os.as_str() {
        "windows" => build_windows(&out_dir),
        "linux" => build_linux(&out_dir),
        _ => {}
    }
}

fn generate_native_header(out_dir: &Path) {
    fs::write(out_dir.join("native_scene.h"), native_header())
        .expect("write generated native scene ABI header");
}

fn native_header() -> String {
    include!("src/native_abi_spec.rs")
}

fn c_field(name: &str, field_type: &str) -> String {
    match field_type {
        "u32" => format!("uint32_t {name};"),
        "u64" => format!("uint64_t {name};"),
        "f32" => format!("float {name};"),
        "usize" => format!("size_t {name};"),
        "const_u8_ptr" => format!("const uint8_t *{name};"),
        "const_command_ptr" => format!("const CcloverCommand *{name};"),
        "const_point_ptr" => format!("const CcloverPoint *{name};"),
        "const_damage_rect_ptr" => format!("const CcloverDamageRect *{name};"),
        "poll_fn" => format!("uint32_t (*{name})(void *context);"),
        "scene_fn" => format!(
            "void (*{name})(void *context, void *measure_context, float (*measure_text)(void *measure_context, const uint8_t *text, size_t text_len, uint32_t text_size, uint32_t flags), CcloverScene *scene);"
        ),
        other => panic!("unsupported native ABI field type: {other}"),
    }
}

fn build_windows(out_dir: &Path) {
    println!("cargo:rerun-if-changed=native/windows_host.c");
    cc::Build::new()
        .file("native/windows_host.c")
        .include(out_dir)
        .include("native")
        .warnings(true)
        .compile("cclover_windows_host");

    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=shell32");
}

fn build_linux(out_dir: &Path) {
    println!("cargo:rerun-if-changed=native/linux_host.c");
    println!("cargo:rerun-if-changed=native/linux_tray.c");
    println!("cargo:rerun-if-changed=native/wayland/wlr-layer-shell-unstable-v1-protocol.c");
    println!("cargo:rerun-if-changed=native/wayland/wlr-layer-shell-unstable-v1-client-protocol.h");
    println!("cargo:rerun-if-changed=native/wayland/xdg-shell-protocol.c");

    let mut build = cc::Build::new();
    build
        .file("native/linux_host.c")
        .file("native/linux_tray.c")
        .file("native/wayland/wlr-layer-shell-unstable-v1-protocol.c")
        .file("native/wayland/xdg-shell-protocol.c")
        .include(out_dir)
        .include("native")
        .include("native/wayland")
        .warnings(true);

    for package in [
        "cairo",
        "x11",
        "xext",
        "xrender",
        "wayland-client",
        "gio-2.0",
    ] {
        let library = pkg_config::Config::new()
            .cargo_metadata(true)
            .probe(package)
            .unwrap_or_else(|error| panic!("native Linux desktop requires {package}: {error}"));
        for include in library.include_paths {
            build.include(include);
        }
    }

    build.compile("cclover_linux_host");
}

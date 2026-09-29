use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

macro_rules! define_native_abi {
    (
        structs {
            $(
                $rust_struct:ident => $c_struct:ident {
                    $( $field:ident : $field_type:ident, )*
                }
            )*
        }
    ) => {{
        let mut header = String::from(
            "#ifndef CCLOVER_TUI_NATIVE_H\n#define CCLOVER_TUI_NATIVE_H\n\n#include <stddef.h>\n#include <stdint.h>\n\n",
        );

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
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    generate_native_header(&out_dir);

    println!("cargo:rerun-if-changed=src/native_abi_spec.rs");
    println!("cargo:rerun-if-changed=native/tui.c");
    cc::Build::new()
        .file("native/tui.c")
        .include(&out_dir)
        .warnings(true)
        .compile("cclover_tui_native");
}

fn generate_native_header(out_dir: &Path) {
    fs::write(out_dir.join("native_tui.h"), native_header())
        .expect("write generated TUI native ABI header");
}

fn native_header() -> String {
    include!("src/native_abi_spec.rs")
}

fn c_field(name: &str, field_type: &str) -> String {
    match field_type {
        "usize" => format!("size_t {name};"),
        "const_u8_ptr" => format!("const uint8_t *{name};"),
        "const_u64_ptr" => format!("const uint64_t *{name};"),
        "native_text" => format!("CcloverText {name};"),
        "const_native_text_ptr" => format!("const CcloverText *{name};"),
        "native_panel" => format!("CcloverPanel {name};"),
        other => panic!("unsupported TUI native ABI field type: {other}"),
    }
}

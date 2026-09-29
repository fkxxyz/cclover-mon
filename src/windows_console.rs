#![allow(unsafe_code)]

use std::ffi::OsString;

use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole};

pub fn prepare() {
    // GUI-subsystem processes do not inherit a console automatically. Reuse the
    // caller's console when one exists; only create a new one for terminal-facing
    // modes launched without a console (for example from Explorer).
    let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 };
    if !attached && needs_console(std::env::args_os().skip(1)) {
        unsafe {
            AllocConsole();
        }
    }
}

fn needs_console(args: impl IntoIterator<Item = OsString>) -> bool {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return false;
    };

    if first == "--tui" || first == "--help" || first == "-h" {
        return true;
    }

    if !first.to_string_lossy().starts_with('-') {
        return true;
    }

    args.any(|arg| arg == "--tui")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args<'a>(values: &'a [&str]) -> impl Iterator<Item = OsString> + 'a {
        values.iter().map(OsString::from)
    }

    #[test]
    fn desktop_and_http_modes_do_not_allocate_console() {
        assert!(!needs_console(args(&[])));
        assert!(!needs_console(args(&["--desktop"])));
        assert!(!needs_console(args(&["--http"])));
        assert!(!needs_console(args(&[
            "--desktop",
            "--http",
            "--http-bind",
            "0.0.0.0:9847",
        ])));
    }

    #[test]
    fn tui_and_commands_require_console() {
        assert!(needs_console(args(&["--tui"])));
        assert!(needs_console(args(&["--desktop", "--tui"])));
        assert!(needs_console(args(&["help"])));
        assert!(needs_console(args(&["probe", "cpu"])));
        assert!(needs_console(args(&["--help"])));
    }
}

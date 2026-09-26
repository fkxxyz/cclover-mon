use crate::core::devlog;

pub(super) fn probe_note(notes: &mut Option<&mut Vec<String>>, message: impl FnOnce() -> String) {
    if let Some(notes) = notes.as_deref_mut() {
        notes.push(message());
    }
}

pub(super) fn report_issue(notes: &mut Option<&mut Vec<String>>, message: impl FnOnce() -> String) {
    if notes.is_none() && !devlog::enabled() {
        return;
    }

    let message = message();
    devlog::log(format_args!("collector issue: {message}"));
    if let Some(notes) = notes.as_deref_mut() {
        notes.push(message);
    }
}

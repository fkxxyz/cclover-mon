use crate::core::devlog;
use crate::core::model::CollectionUnavailable;

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

pub(super) fn unavailable_from_io(error: &std::io::Error) -> CollectionUnavailable {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        CollectionUnavailable::PermissionDenied
    } else {
        CollectionUnavailable::Unavailable
    }
}

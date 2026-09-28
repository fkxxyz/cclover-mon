use std::io;

use crate::core::model::CollectionUnavailable;

pub(super) fn report_issue(notes: &mut Option<&mut Vec<String>>, message: impl FnOnce() -> String) {
    if let Some(notes) = notes.as_deref_mut() {
        notes.push(message());
    }
}

pub(super) fn unavailable_from_io(error: &io::Error) -> CollectionUnavailable {
    match error.kind() {
        io::ErrorKind::PermissionDenied => CollectionUnavailable::PermissionDenied,
        io::ErrorKind::InvalidData => CollectionUnavailable::InvalidData,
        _ => CollectionUnavailable::Unavailable,
    }
}

use crate::model::*;

pub(super) fn map_status<S, T>(source: &Collection<S>, value: T) -> Collection<T> {
    match source {
        Collection::Available(_) => Collection::Available(value),
        Collection::Degraded(_) => Collection::Degraded(value),
        Collection::Unavailable(reason) => Collection::Unavailable(*reason),
    }
}

pub(super) fn combine_status<A, B, T>(
    first: &Collection<A>,
    second: &Collection<B>,
    value: T,
) -> Collection<T> {
    match (first, second) {
        (Collection::Unavailable(reason), _) | (_, Collection::Unavailable(reason)) => {
            Collection::Unavailable(*reason)
        }
        (Collection::Degraded(_), _) | (_, Collection::Degraded(_)) => Collection::Degraded(value),
        (Collection::Available(_), Collection::Available(_)) => Collection::Available(value),
    }
}

pub(super) fn collection_from_status<T>(status: CollectionStatus, value: T) -> Collection<T> {
    match status {
        CollectionStatus::Available => Collection::Available(value),
        CollectionStatus::Degraded => Collection::Degraded(value),
        CollectionStatus::Unavailable(reason) => Collection::Unavailable(reason),
    }
}

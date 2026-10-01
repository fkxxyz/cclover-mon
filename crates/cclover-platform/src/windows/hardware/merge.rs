use super::*;

impl Batch {
    pub(super) fn for_superio_unavailable(
        reason: CollectionUnavailable,
        projection: SuperIoProjection,
    ) -> Self {
        Self {
            temperatures: if matches!(
                projection,
                SuperIoProjection::All | SuperIoProjection::Temperatures
            ) {
                Collection::unavailable(reason)
            } else {
                Collection::unavailable(CollectionUnavailable::Unsupported)
            },
            fans: if matches!(projection, SuperIoProjection::All | SuperIoProjection::Fans) {
                Collection::unavailable(reason)
            } else {
                Collection::unavailable(CollectionUnavailable::Unsupported)
            },
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub(super) fn projected_collection<T>(
    requested: bool,
    degraded: bool,
    values: Vec<T>,
) -> Collection<Vec<T>> {
    if !requested {
        Collection::unavailable(CollectionUnavailable::Unsupported)
    } else if degraded {
        Collection::degraded(values)
    } else {
        Collection::available(values)
    }
}

pub(in crate::windows) fn merge_temperature_sources(
    sources: impl IntoIterator<Item = Collection<Vec<TemperatureSnapshot>>>,
) -> Collection<Vec<TemperatureSnapshot>> {
    merge_sources(sources)
}

pub(super) fn merge_fan_sources(
    sources: impl IntoIterator<Item = Collection<Vec<FanSnapshot>>>,
) -> Collection<Vec<FanSnapshot>> {
    merge_sources(sources)
}

pub(super) fn merge_sources<T>(
    sources: impl IntoIterator<Item = Collection<Vec<T>>>,
) -> Collection<Vec<T>> {
    let mut values = Vec::new();
    let mut observable = false;
    let mut degraded = false;
    let mut unavailable_reason = CollectionUnavailable::Unsupported;

    for source in sources {
        match source {
            Collection::Available(mut source_values) => {
                observable = true;
                values.append(&mut source_values);
            }
            Collection::Degraded(mut source_values) => {
                observable = true;
                degraded = true;
                values.append(&mut source_values);
            }
            Collection::Unavailable(CollectionUnavailable::Unsupported) => {}
            Collection::Unavailable(reason) => {
                degraded = true;
                unavailable_reason = reason;
            }
        }
    }

    if observable {
        if degraded {
            Collection::degraded(values)
        } else {
            Collection::available(values)
        }
    } else {
        Collection::unavailable(unavailable_reason)
    }
}

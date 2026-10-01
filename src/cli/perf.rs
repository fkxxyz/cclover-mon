use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PerfLimit {
    Unbounded,
    Duration(Duration),
    Samples(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PerfRequest {
    Headless(PerfLimit),
    Collector { kind: ProbeKind, limit: PerfLimit },
}

pub(super) fn parse_perf(mut args: impl Iterator<Item = String>) -> Result<PerfRequest, CliError> {
    let workload = args
        .next()
        .ok_or_else(|| CliError::new("perf requires a workload: headless or collector"))?;

    match workload.as_str() {
        "headless" => Ok(PerfRequest::Headless(parse_perf_limit(args)?)),
        "collector" => {
            let collector = args
                .next()
                .ok_or_else(|| CliError::new("perf collector requires a collector name"))?;
            let kind = collector.parse::<ProbeKind>().map_err(CliError::new)?;
            let limit = parse_perf_limit(args)?;
            Ok(PerfRequest::Collector { kind, limit })
        }
        other => Err(CliError::new(format!(
            "unknown perf workload {other:?}; expected headless or collector"
        ))),
    }
}

pub(super) fn perf(request: PerfRequest) {
    match request {
        PerfRequest::Headless(limit) => {
            prepare_pawnio_if_needed(true);
            let mut sampler = Sampler::new(Backend::new());
            run_perf(limit, || {
                std::hint::black_box(sampler.sample());
            });
        }
        PerfRequest::Collector { kind, limit } => {
            prepare_pawnio_if_needed(probe_needs_pawnio(kind));
            let mut backend = Backend::new();
            run_perf(limit, || backend.collect_for_perf(kind));
        }
    }
}

fn parse_perf_limit(mut args: impl Iterator<Item = String>) -> Result<PerfLimit, CliError> {
    let Some(option) = args.next() else {
        return Ok(PerfLimit::Unbounded);
    };
    let value = args
        .next()
        .ok_or_else(|| CliError::new(format!("{option} requires a positive integer")))?;
    if let Some(extra) = args.next() {
        return Err(CliError::new(format!("unexpected perf argument: {extra}")));
    }

    let value = value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| CliError::new(format!("{option} requires a positive integer")))?;

    match option.as_str() {
        "--duration" => Ok(PerfLimit::Duration(Duration::from_secs(value))),
        "--samples" => Ok(PerfLimit::Samples(value)),
        _ => Err(CliError::new(format!(
            "unexpected perf argument: {option}; expected --duration or --samples"
        ))),
    }
}

fn run_perf(limit: PerfLimit, mut work: impl FnMut()) {
    let run_started = Instant::now();
    let deadline = match limit {
        PerfLimit::Duration(duration) => Some(run_started + duration),
        PerfLimit::Unbounded | PerfLimit::Samples(_) => None,
    };
    let mut samples = 0_u64;

    loop {
        let cycle = SampleCycle::begin();
        work();
        samples += 1;

        if matches!(limit, PerfLimit::Samples(target) if samples >= target) {
            break;
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }

        let remaining_interval = cycle.remaining();
        let sleep_for = deadline.map_or(remaining_interval, |deadline| {
            remaining_interval.min(deadline.saturating_duration_since(Instant::now()))
        });
        if !sleep_for.is_zero() {
            std::thread::sleep(sleep_for);
        }
    }
}

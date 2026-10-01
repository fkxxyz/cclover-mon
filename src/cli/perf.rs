use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PerfLimit {
    Unbounded,
    Duration(Duration),
    Samples(u64),
}

pub(super) fn perf(mut args: impl Iterator<Item = String>) {
    let workload = args
        .next()
        .unwrap_or_else(|| fail("perf requires a workload: headless or collector"));

    match workload.as_str() {
        "headless" => {
            let limit = parse_perf_limit(args);
            prepare_pawnio_if_needed(true);
            let mut sampler = Sampler::new(Backend::new());
            run_perf(limit, || {
                std::hint::black_box(sampler.sample());
            });
        }
        "collector" => {
            let collector = args
                .next()
                .unwrap_or_else(|| fail("perf collector requires a collector name"));
            let kind = collector
                .parse::<ProbeKind>()
                .unwrap_or_else(|error| fail(&error));
            let limit = parse_perf_limit(args);
            prepare_pawnio_if_needed(probe_needs_pawnio(kind));
            let mut backend = Backend::new();
            run_perf(limit, || backend.collect_for_perf(kind));
        }
        other => fail(&format!(
            "unknown perf workload {other:?}; expected headless or collector"
        )),
    }
}

fn parse_perf_limit(mut args: impl Iterator<Item = String>) -> PerfLimit {
    let Some(option) = args.next() else {
        return PerfLimit::Unbounded;
    };
    let value = args
        .next()
        .unwrap_or_else(|| fail(&format!("{option} requires a positive integer")));
    if let Some(extra) = args.next() {
        fail(&format!("unexpected perf argument: {extra}"));
    }

    let value = value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or_else(|| fail(&format!("{option} requires a positive integer")));

    match option.as_str() {
        "--duration" => PerfLimit::Duration(Duration::from_secs(value)),
        "--samples" => PerfLimit::Samples(value),
        _ => fail(&format!(
            "unexpected perf argument: {option}; expected --duration or --samples"
        )),
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

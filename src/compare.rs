//! Side-by-side comparison of two finished runs (baseline and candidate).
use crate::{
    core::TestPlan,
    metrics::{Counts, Latency, RunState, Snapshot},
};

pub const DEFAULT_NOISE_PCT: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareMetric {
    Throughput,
    P50,
    P90,
    P95,
    P99,
    Mean,
    ErrorRate,
    DataRate,
}
impl CompareMetric {
    pub const ALL: [Self; 8] = [
        Self::Throughput,
        Self::P50,
        Self::P90,
        Self::P95,
        Self::P99,
        Self::Mean,
        Self::ErrorRate,
        Self::DataRate,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Throughput => "Throughput (avg req/s)",
            Self::P50 => "P50 latency (ms)",
            Self::P90 => "P90 latency (ms)",
            Self::P95 => "P95 latency (ms)",
            Self::P99 => "P99 latency (ms)",
            Self::Mean => "Mean latency (ms)",
            Self::ErrorRate => "Error rate (%)",
            Self::DataRate => "Data (avg bytes/s)",
        }
    }
    /// `Some(true)` when higher is better, `Some(false)` when lower is better,
    /// `None` for informational metrics that do not count toward the verdict.
    pub fn higher_is_better(self) -> Option<bool> {
        match self {
            Self::Throughput => Some(true),
            Self::DataRate => None,
            _ => Some(false),
        }
    }
    fn value(self, counts: &Counts, latency: &Latency, elapsed: f64) -> Option<f64> {
        let has_latency = latency.samples > 0;
        match self {
            Self::Throughput => (elapsed > 0.0).then(|| counts.completed as f64 / elapsed),
            Self::P50 => has_latency.then_some(latency.p50),
            Self::P90 => has_latency.then_some(latency.p90),
            Self::P95 => has_latency.then_some(latency.p95),
            Self::P99 => has_latency.then_some(latency.p99),
            Self::Mean => has_latency.then_some(latency.mean),
            Self::ErrorRate => (counts.completed > 0).then(|| counts.error_rate()),
            Self::DataRate => (elapsed > 0.0).then(|| counts.bytes as f64 / elapsed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Improved,
    Regressed,
    Unchanged,
    /// Moved beyond the noise threshold on an informational metric.
    Changed,
    NoData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    pub metric: CompareMetric,
    pub base: Option<f64>,
    pub candidate: Option<f64>,
    /// Relative change in percent; `None` without data or from a zero baseline.
    pub change_pct: Option<f64>,
    pub change: Change,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndpointDelta {
    pub id: u64,
    pub name: String,
    pub deltas: Vec<Delta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareVerdict {
    Improved,
    Regressed,
    Mixed,
    Unchanged,
    NoData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub overall: Vec<Delta>,
    pub endpoints: Vec<EndpointDelta>,
    /// Request names that appear in only one of the two runs.
    pub only_base: Vec<String>,
    pub only_candidate: Vec<String>,
    /// Overall verdict from whole-run metrics that have a direction.
    pub verdict: CompareVerdict,
    /// Reasons the runs are not like-for-like. Comparison still proceeds.
    pub warnings: Vec<String>,
}

fn delta(metric: CompareMetric, base: Option<f64>, candidate: Option<f64>, noise: f64) -> Delta {
    let (change_pct, change) = match (base, candidate) {
        (Some(b), Some(c)) => {
            let pct = if b == 0.0 {
                (c == 0.0).then_some(0.0)
            } else {
                Some((c - b) / b.abs() * 100.0)
            };
            let moved = pct.is_none_or(|pct| pct.abs() > noise);
            let change = match metric.higher_is_better() {
                _ if !moved => Change::Unchanged,
                None => Change::Changed,
                Some(higher) if (c > b) == higher => Change::Improved,
                Some(_) => Change::Regressed,
            };
            (pct, change)
        }
        _ => (None, Change::NoData),
    };
    Delta {
        metric,
        base,
        candidate,
        change_pct,
        change,
    }
}

fn deltas(
    base: (&Counts, &Latency, f64),
    candidate: (&Counts, &Latency, f64),
    noise: f64,
) -> Vec<Delta> {
    CompareMetric::ALL
        .into_iter()
        .map(|m| {
            delta(
                m,
                m.value(base.0, base.1, base.2),
                m.value(candidate.0, candidate.1, candidate.2),
                noise,
            )
        })
        .collect()
}

/// Compares whole-run results. Throughput and data use completed requests and
/// bytes over the measured window (elapsed time after warmup), not the trailing
/// five-second card values. A change within `noise_pct` percent counts as
/// unchanged; this is a tolerance band, not a statistical significance test.
pub fn compare(
    base: &Snapshot,
    base_plan: &TestPlan,
    candidate: &Snapshot,
    candidate_plan: &TestPlan,
    noise_pct: f64,
) -> Comparison {
    let noise = noise_pct.max(0.0);
    let overall = deltas(
        (&base.counts, &base.latency, base.measured_elapsed()),
        (
            &candidate.counts,
            &candidate.latency,
            candidate.measured_elapsed(),
        ),
        noise,
    );
    let endpoints = base
        .endpoints
        .iter()
        .filter_map(|b| {
            let c = candidate.endpoints.iter().find(|c| c.id == b.id)?;
            Some(EndpointDelta {
                id: b.id,
                name: c.name.clone(),
                deltas: deltas(
                    (&b.counts, &b.latency, base.measured_elapsed()),
                    (&c.counts, &c.latency, candidate.measured_elapsed()),
                    noise,
                ),
            })
        })
        .collect();
    let missing = |from: &Snapshot, other: &Snapshot| -> Vec<String> {
        from.endpoints
            .iter()
            .filter(|e| other.endpoints.iter().all(|o| o.id != e.id))
            .map(|e| e.name.clone())
            .collect()
    };
    let directional: Vec<_> = overall
        .iter()
        .filter(|d| d.metric.higher_is_better().is_some())
        .map(|d| d.change)
        .collect();
    let improved = directional.contains(&Change::Improved);
    let regressed = directional.contains(&Change::Regressed);
    let verdict = match (improved, regressed) {
        (true, true) => CompareVerdict::Mixed,
        (false, true) => CompareVerdict::Regressed,
        (true, false) => CompareVerdict::Improved,
        _ if directional.iter().all(|c| *c == Change::NoData) => CompareVerdict::NoData,
        _ => CompareVerdict::Unchanged,
    };
    Comparison {
        overall,
        endpoints,
        only_base: missing(base, candidate),
        only_candidate: missing(candidate, base),
        verdict,
        warnings: warnings(base, base_plan, candidate, candidate_plan),
    }
}

fn warnings(
    base: &Snapshot,
    base_plan: &TestPlan,
    candidate: &Snapshot,
    candidate_plan: &TestPlan,
) -> Vec<String> {
    let mut out = Vec::new();
    for (role, s) in [("Baseline", base), ("Candidate", candidate)] {
        if s.state != RunState::Completed {
            out.push(format!(
                "{role} run did not complete normally ({:?})",
                s.state
            ));
        }
    }
    let (b, c) = (&base_plan.load, &candidate_plan.load);
    let mut differs = |what: &str, left: String, right: String| {
        if left != right {
            out.push(format!("{what} differs: {left} vs {right}"));
        }
    };
    differs(
        "Virtual users",
        b.max_users().to_string(),
        c.max_users().to_string(),
    );
    differs(
        "Duration",
        format!("{}s", b.total_secs()),
        format!("{}s", c.total_secs()),
    );
    differs("Load profile", b.profile_label(), c.profile_label());
    differs("RPS cap", b.rps_limit.to_string(), c.rps_limit.to_string());
    differs(
        "Think time",
        format!("{} ms", b.think_ms),
        format!("{} ms", c.think_ms),
    );
    differs(
        "Timeout",
        format!("{} ms", b.timeout_ms),
        format!("{} ms", c.timeout_ms),
    );
    differs("Warmup", b.warmup.describe(), c.warmup.describe());
    let requests = |p: &TestPlan| -> Vec<(u64, String, String)> {
        p.requests
            .iter()
            .map(|r| (r.id, r.method.clone(), r.url.clone()))
            .collect()
    };
    if requests(base_plan) != requests(candidate_plan) {
        out.push("Request steps differ (IDs, methods or URLs)".into());
    }
    out
}

/// UTC start time encoded in a run ID (nanoseconds since the Unix epoch).
pub fn run_time_label(id: &str) -> String {
    let Ok(nanos) = id.parse::<u128>() else {
        return String::new();
    };
    let secs = (nanos / 1_000_000_000) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3_600,
        rem % 3_600 / 60
    )
}

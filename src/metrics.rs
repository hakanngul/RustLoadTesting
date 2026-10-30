use crate::core::{SloMetric, Threshold, Warmup};
use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const SERIES_LIMIT: usize = 900;
pub const LOG_LIMIT: usize = 5_000;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunState {
    #[default]
    Idle,
    Validating,
    Running,
    Stopping,
    Completed,
    Failed,
    Cancelled,
}
impl RunState {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

// Persisted structs default missing fields so history written by older
// versions keeps loading after fields are added.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Counts {
    pub started: u64,
    pub completed: u64,
    pub successful: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub network_errors: u64,
    pub timeouts: u64,
    pub assertion_errors: u64,
    pub http_errors: u64,
    pub status_errors: u64,
    pub body_errors: u64,
    pub preparation_errors: u64,
    /// Body bytes of fully read responses.
    pub bytes: u64,
    /// Arrival-rate iterations not started because every VU was busy. Not
    /// requests, so outside `started` and the error rate.
    pub dropped_iterations: u64,
}
impl Counts {
    pub fn error_rate(&self) -> f64 {
        if self.completed == 0 {
            0.0
        } else {
            self.failed as f64 / self.completed as f64 * 100.0
        }
    }
    pub fn in_flight(&self) -> u64 {
        self.started.saturating_sub(self.completed + self.cancelled)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Latency {
    pub samples: u64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub mean: f64,
}
pub fn histogram() -> Histogram<u64> {
    Histogram::new_with_bounds(1, 3_600_000_000, 3).expect("valid histogram bounds")
}
pub fn latency(h: &Histogram<u64>) -> Latency {
    if h.is_empty() {
        return Latency::default();
    }
    Latency {
        samples: h.len(),
        p50: h.value_at_quantile(0.50) as f64 / 1000.0,
        p90: h.value_at_quantile(0.90) as f64 / 1000.0,
        p95: h.value_at_quantile(0.95) as f64 / 1000.0,
        p99: h.value_at_quantile(0.99) as f64 / 1000.0,
        mean: h.mean() / 1000.0,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Point {
    pub interval_secs: f64,
    pub second: u64,
    pub elapsed: f64,
    pub users: u32,
    pub completed: u64,
    pub successful: u64,
    pub failed: u64,
    pub latency: Latency,
    pub bytes: u64,
    /// Completions of warmup requests in this interval (excluded from the counts above).
    pub warmup_completed: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LogEntry {
    pub elapsed: f64,
    pub level: String,
    pub message: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Endpoint {
    pub id: u64,
    pub name: String,
    pub method: String,
    pub counts: Counts,
    pub latency: Latency,
}
/// Measured failures of one request that share a sanitized reason. Reasons come
/// from the plan, the status code and fixed transport labels, never from
/// response content or transport error text (which can contain URLs).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ErrorGroup {
    pub request_id: u64,
    pub reason: String,
    pub count: u64,
    /// Elapsed seconds of the first and latest failure.
    pub first_at: f64,
    pub last_at: f64,
}
/// Distinct (request, reason) groups kept; later new reasons are only counted.
pub const ERROR_GROUP_LIMIT: usize = 100;

/// One bar of the latency distribution: responses with latency in `from_ms..to_ms`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LatencyBucket {
    pub from_ms: f64,
    pub to_ms: f64,
    pub count: u64,
}
/// Bars in the latency distribution; bounded so snapshots stay small.
pub const DISTRIBUTION_BUCKETS: usize = 40;

/// Whole-run latency distribution in log-spaced buckets between the fastest and
/// slowest recorded response. Counts come from the histogram's recorded values,
/// so they sum to its sample count.
pub fn distribution(h: &Histogram<u64>) -> Vec<LatencyBucket> {
    if h.is_empty() {
        return vec![];
    }
    let low = h.min().max(1) as f64;
    let ratio = (h.max() as f64 / low).max(1.0);
    // min/max are the ends of their equivalence ranges; one range is one bar.
    let n = if h.equivalent(h.min(), h.max()) {
        1
    } else {
        DISTRIBUTION_BUCKETS
    };
    let edge = |i: usize| low * ratio.powf(i as f64 / n as f64) / 1000.0;
    let mut buckets: Vec<_> = (0..n)
        .map(|i| LatencyBucket {
            from_ms: edge(i),
            to_ms: edge(i + 1),
            count: 0,
        })
        .collect();
    for v in h.iter_recorded() {
        let position = (v.value_iterated_to() as f64 / low).ln() / ratio.ln() * n as f64;
        // Saturating casts put rounding below 0 into the first bucket.
        let i = if n == 1 {
            0
        } else {
            (position as usize).min(n - 1)
        };
        buckets[i].count += v.count_at_value();
    }
    buckets
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub run_id: String,
    pub name: String,
    pub state: RunState,
    pub elapsed: f64,
    pub active_users: u32,
    pub target_users: u32,
    pub rps: f64,
    /// Response body bytes per second over the same trailing window as `rps`.
    pub bytes_rate: f64,
    pub counts: Counts,
    pub latency: Latency,
    pub statuses: BTreeMap<u16, u64>,
    pub endpoints: Vec<Endpoint>,
    pub series: VecDeque<Point>,
    pub logs: VecDeque<LogEntry>,
    pub issue: Option<String>,
    /// SLO evaluation of this snapshot; `None` when the plan has no thresholds.
    pub slo: Option<SloOutcome>,
    /// Warmup setting of the run.
    pub warmup: Warmup,
    /// Requests started during warmup; never part of `counts` or any histogram.
    pub warmup_counts: Counts,
    /// Seconds into the run where the measured window starts (0 without warmup).
    pub measured_from: f64,
    /// Warmup is configured and still in progress.
    pub warming_up: bool,
    /// Iterations per second of an arrival-rate (open model) run; `None` for
    /// closed-model runs.
    pub arrival_rate: Option<f64>,
    /// Whole-run response latency distribution (measured window only).
    pub latency_distribution: Vec<LatencyBucket>,
    /// Measured failures grouped by request and reason, most frequent first.
    /// Their counts plus `errors_overflow` equal `counts.failed`.
    pub errors: Vec<ErrorGroup>,
    /// Failures whose reason arrived after `ERROR_GROUP_LIMIT` groups existed.
    pub errors_overflow: u64,
}
impl Snapshot {
    /// Length of the measured window: elapsed time after warmup.
    pub fn measured_elapsed(&self) -> f64 {
        (self.elapsed - self.measured_from).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThresholdStatus {
    Pass,
    Fail,
    #[default]
    NoData,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThresholdResult {
    pub threshold: Threshold,
    pub actual: Option<f64>,
    pub status: ThresholdStatus,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Failed,
    #[default]
    Inconclusive,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SloOutcome {
    pub verdict: Verdict,
    /// The run did not complete normally (cancelled or failed), or is still running.
    pub partial: bool,
    pub results: Vec<ThresholdResult>,
}

/// Checks thresholds against whole-run figures: latency from the whole-run
/// histogram, throughput as completed requests over elapsed time (not the
/// trailing five-second RPS). Any failure fails the run; otherwise a threshold
/// without data makes it inconclusive.
pub fn evaluate_thresholds(s: &Snapshot, thresholds: &[Threshold]) -> Option<SloOutcome> {
    if thresholds.is_empty() {
        return None;
    }
    let results: Vec<_> = thresholds
        .iter()
        .map(|t| {
            let scope = match t.request_id {
                None => Some((&s.counts, &s.latency)),
                Some(id) => s
                    .endpoints
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| (&e.counts, &e.latency)),
            };
            let actual = scope.and_then(|(counts, latency)| {
                let has_latency = latency.samples > 0;
                match t.metric {
                    SloMetric::P50 => has_latency.then_some(latency.p50),
                    SloMetric::P90 => has_latency.then_some(latency.p90),
                    SloMetric::P95 => has_latency.then_some(latency.p95),
                    SloMetric::P99 => has_latency.then_some(latency.p99),
                    SloMetric::Mean => has_latency.then_some(latency.mean),
                    SloMetric::ErrorRate => (counts.completed > 0).then(|| counts.error_rate()),
                    SloMetric::Throughput => {
                        let window = s.measured_elapsed();
                        (window > 0.0).then(|| counts.completed as f64 / window)
                    }
                }
            });
            let status = match actual {
                None => ThresholdStatus::NoData,
                Some(v) => {
                    let met = if t.metric.higher_is_better() {
                        v >= t.value
                    } else {
                        v <= t.value
                    };
                    if met {
                        ThresholdStatus::Pass
                    } else {
                        ThresholdStatus::Fail
                    }
                }
            };
            ThresholdResult {
                threshold: t.clone(),
                actual,
                status,
            }
        })
        .collect();
    let verdict = if results.iter().any(|r| r.status == ThresholdStatus::Fail) {
        Verdict::Failed
    } else if results.iter().any(|r| r.status == ThresholdStatus::NoData) {
        Verdict::Inconclusive
    } else {
        Verdict::Passed
    };
    Some(SloOutcome {
        verdict,
        partial: s.state != RunState::Completed,
        results,
    })
}

impl SloOutcome {
    /// `Passed`, `Failed`, `Inconclusive`, with `(partial run)` when not completed.
    pub fn summary(&self) -> String {
        format!(
            "{:?}{}",
            self.verdict,
            if self.partial { " (partial run)" } else { "" }
        )
    }
}
impl ThresholdResult {
    /// Measured value with its unit, or `no data`.
    pub fn actual_label(&self) -> String {
        self.actual.map_or_else(
            || "no data".into(),
            |v| format!("{v:.2} {}", self.threshold.metric.unit()),
        )
    }
}
/// Name of a threshold's scope: the whole run or the request it targets.
pub fn scope_label(s: &Snapshot, request_id: Option<u64>) -> String {
    match request_id {
        None => "All requests".into(),
        Some(id) => s
            .endpoints
            .iter()
            .find(|e| e.id == id)
            .map_or_else(|| format!("Request #{id}"), |e| e.name.clone()),
    }
}

pub struct EndpointAccumulator {
    pub summary: Endpoint,
    pub histogram: Histogram<u64>,
}
pub struct Accumulator {
    pub counts: Counts,
    pub total: Histogram<u64>,
    pub bucket: Histogram<u64>,
    pub endpoints: BTreeMap<u64, EndpointAccumulator>,
    pub statuses: BTreeMap<u16, u64>,
    pub series: VecDeque<Point>,
    pub logs: VecDeque<LogEntry>,
    pub users: u32,
    last_completed: u64,
    last_successful: u64,
    last_failed: u64,
    last_bytes: u64,
    last_warmup_completed: u64,
    last_elapsed: f64,
    /// Requests started during warmup.
    pub warmup: Counts,
    /// Elapsed seconds when the first measured request started.
    pub warmup_ended_at: Option<f64>,
    pub errors: BTreeMap<(u64, String), ErrorGroup>,
    pub errors_overflow: u64,
}
impl Default for Accumulator {
    fn default() -> Self {
        Self {
            counts: Counts::default(),
            total: histogram(),
            bucket: histogram(),
            endpoints: BTreeMap::new(),
            statuses: BTreeMap::new(),
            series: VecDeque::new(),
            logs: VecDeque::new(),
            users: 0,
            last_completed: 0,
            last_successful: 0,
            last_failed: 0,
            last_bytes: 0,
            last_warmup_completed: 0,
            last_elapsed: 0.0,
            warmup: Counts::default(),
            warmup_ended_at: None,
            errors: BTreeMap::new(),
            errors_overflow: 0,
        }
    }
}
impl Accumulator {
    /// Groups a measured failure by request and sanitized reason.
    pub fn record_error(&mut self, request_id: u64, reason: String, at: f64) {
        let key = (request_id, reason);
        if let Some(group) = self.errors.get_mut(&key) {
            group.count += 1;
            group.last_at = at;
        } else if self.errors.len() < ERROR_GROUP_LIMIT {
            self.errors.insert(
                key.clone(),
                ErrorGroup {
                    request_id,
                    reason: key.1,
                    count: 1,
                    first_at: at,
                    last_at: at,
                },
            );
        } else {
            self.errors_overflow += 1;
        }
    }
    pub fn log(&mut self, elapsed: f64, level: &str, message: String) {
        if self.logs.len() == LOG_LIMIT {
            self.logs.pop_front();
        }
        let mut end = message.len().min(2048);
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        self.logs.push_back(LogEntry {
            elapsed,
            level: level.into(),
            message: message[..end].into(),
        });
    }
    pub fn point(&mut self, elapsed: f64) -> Point {
        let point = Point {
            interval_secs: (elapsed - self.last_elapsed).max(0.000001),
            second: elapsed.floor() as u64,
            elapsed,
            users: self.users,
            completed: self.counts.completed - self.last_completed,
            successful: self.counts.successful - self.last_successful,
            failed: self.counts.failed - self.last_failed,
            latency: latency(&self.bucket),
            bytes: self.counts.bytes - self.last_bytes,
            warmup_completed: self.warmup.completed - self.last_warmup_completed,
        };
        self.last_completed = self.counts.completed;
        self.last_successful = self.counts.successful;
        self.last_failed = self.counts.failed;
        self.last_bytes = self.counts.bytes;
        self.last_warmup_completed = self.warmup.completed;
        self.last_elapsed = elapsed;
        self.bucket.reset();
        if self.series.len() == SERIES_LIMIT {
            self.series.pop_front();
        }
        self.series.push_back(point.clone());
        point
    }
    /// Per-second rate over the trailing five seconds: whole intervals inside the
    /// window, a prorated partial interval at its start, plus the open interval.
    fn trailing_rate(&self, elapsed: f64, value: impl Fn(&Point) -> u64, open: u64) -> f64 {
        let window_start = (elapsed - 5.0).max(0.0);
        let count: f64 = self
            .series
            .iter()
            .rev()
            .filter(|p| p.elapsed > window_start)
            .map(|p| {
                value(p) as f64
                    * ((p.elapsed - window_start).min(p.interval_secs) / p.interval_secs)
            })
            .sum::<f64>()
            + open as f64;
        count / elapsed.clamp(0.001, 5.0)
    }
    pub fn snapshot(&self, elapsed: f64) -> Snapshot {
        Snapshot {
            elapsed,
            active_users: self.users,
            counts: self.counts.clone(),
            latency: latency(&self.total),
            latency_distribution: distribution(&self.total),
            rps: self.trailing_rate(
                elapsed,
                |p| p.completed,
                self.counts.completed - self.last_completed,
            ),
            bytes_rate: self.trailing_rate(
                elapsed,
                |p| p.bytes,
                self.counts.bytes - self.last_bytes,
            ),
            statuses: self.statuses.clone(),
            endpoints: self
                .endpoints
                .values()
                .map(|e| Endpoint {
                    latency: latency(&e.histogram),
                    ..e.summary.clone()
                })
                .collect(),
            series: self.series.clone(),
            logs: self.logs.clone(),
            warmup_counts: self.warmup.clone(),
            errors: {
                let mut errors: Vec<_> = self.errors.values().cloned().collect();
                errors.sort_by_key(|g| std::cmp::Reverse(g.count));
                errors
            },
            errors_overflow: self.errors_overflow,
            ..Snapshot::default()
        }
    }
}

/// Human-readable binary size, e.g. `512 B`, `1.5 KiB`, `3.2 MiB`.
pub fn format_bytes(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes.max(0.0);
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

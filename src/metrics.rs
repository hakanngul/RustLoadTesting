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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
pub struct Point {
    pub interval_secs: f64,
    pub second: u64,
    pub elapsed: f64,
    pub users: u32,
    pub completed: u64,
    pub successful: u64,
    pub failed: u64,
    pub latency: Latency,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub elapsed: f64,
    pub level: String,
    pub message: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Endpoint {
    pub id: u64,
    pub name: String,
    pub method: String,
    pub counts: Counts,
    pub latency: Latency,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub run_id: String,
    pub name: String,
    pub state: RunState,
    pub elapsed: f64,
    pub active_users: u32,
    pub target_users: u32,
    pub rps: f64,
    pub counts: Counts,
    pub latency: Latency,
    pub statuses: BTreeMap<u16, u64>,
    pub endpoints: Vec<Endpoint>,
    pub series: VecDeque<Point>,
    pub logs: VecDeque<LogEntry>,
    pub issue: Option<String>,
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
    last_elapsed: f64,
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
            last_elapsed: 0.0,
        }
    }
}
impl Accumulator {
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
        };
        self.last_completed = self.counts.completed;
        self.last_successful = self.counts.successful;
        self.last_failed = self.counts.failed;
        self.last_elapsed = elapsed;
        self.bucket.reset();
        if self.series.len() == SERIES_LIMIT {
            self.series.pop_front();
        }
        self.series.push_back(point.clone());
        point
    }
    pub fn snapshot(&self, elapsed: f64) -> Snapshot {
        let window_start = (elapsed - 5.0).max(0.0);
        let count: f64 = self
            .series
            .iter()
            .rev()
            .filter(|p| p.elapsed > window_start)
            .map(|p| {
                p.completed as f64
                    * ((p.elapsed - window_start).min(p.interval_secs) / p.interval_secs)
            })
            .sum::<f64>()
            + (self.counts.completed - self.last_completed) as f64;
        Snapshot {
            elapsed,
            active_users: self.users,
            counts: self.counts.clone(),
            latency: latency(&self.total),
            rps: count / elapsed.clamp(0.001, 5.0),
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
            ..Snapshot::default()
        }
    }
}

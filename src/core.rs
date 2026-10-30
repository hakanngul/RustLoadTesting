use serde::{Deserialize, Serialize, Serializer};
use std::{collections::BTreeMap, path::Path};

pub const SCHEMA_VERSION: u32 = 1;
pub type Variables = BTreeMap<String, String>;

pub fn sensitive(name: &str) -> bool {
    let n = name.to_ascii_lowercase().replace('-', "_");
    [
        "authorization",
        "cookie",
        "set_cookie",
        "password",
        "passwd",
        "token",
        "api_key",
        "secret",
    ]
    .iter()
    .any(|key| n.contains(key))
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub secret: bool,
}
impl Serialize for Field {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Saved<'a> {
            name: &'a str,
            value: &'a str,
            secret: bool,
        }
        let secret = self.secret || sensitive(&self.name);
        // A secret keeps its value only when it is one whole `{{name}}` reference.
        let keep = !secret || is_variable_reference(&self.value);
        Saved {
            name: &self.name,
            value: if keep { &self.value } else { "" },
            secret,
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum Profile {
    #[default]
    Constant,
    RampUp,
    /// Piecewise-linear user count over consecutive stages.
    Stages,
    /// Open model: scenario iterations start at a fixed rate from a pool of at
    /// most `users` virtual users, independent of response times.
    ArrivalRate,
}

/// Linear move from the previous user count (0 at the start) to `target_users`
/// over `duration_secs`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Stage {
    pub duration_secs: u64,
    pub target_users: u32,
}
pub const STAGE_LIMIT: usize = 20;

/// Linear move of the arrival rate from the previous rate (the start rate at
/// first) to `target_rate` iterations/s over `duration_secs`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RateStage {
    pub duration_secs: u64,
    pub target_rate: f64,
}
/// Highest iteration rate a plan may request, in iterations/s.
pub const MAX_ARRIVAL_RATE: f64 = 100_000.0;

/// Requests started during warmup are excluded from every result metric.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Warmup {
    #[default]
    None,
    /// Requests started in the first `secs` seconds of the run.
    Duration { secs: u64 },
    /// The first `count` requests started across all users.
    Requests { count: u64 },
}
impl Warmup {
    pub fn describe(self) -> String {
        match self {
            Self::None => "none".into(),
            Self::Duration { secs } => format!("first {secs}s"),
            Self::Requests { count } => format!("first {count} requests"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoadConfig {
    pub users: u32,
    pub duration_secs: u64,
    pub ramp_secs: u64,
    pub profile: Profile,
    pub rps_limit: u32,
    pub think_ms: u64,
    pub timeout_ms: u64,
    pub body_limit_bytes: usize,
    pub warmup: Warmup,
    /// Used when `profile` is `Stages`; then `users` and `duration_secs` are derived.
    pub stages: Vec<Stage>,
    /// Iterations started per second when `profile` is `ArrivalRate`; `users` is
    /// then the maximum VU pool. With `rate_stages` this is the start rate.
    pub arrival_rate: f64,
    /// Optional ramping of the arrival rate. Empty means a constant
    /// `arrival_rate`; otherwise `duration_secs` is derived from the stages.
    pub rate_stages: Vec<RateStage>,
}
impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            users: 10,
            duration_secs: 30,
            ramp_secs: 10,
            profile: Profile::Constant,
            rps_limit: 20,
            think_ms: 100,
            timeout_ms: 10_000,
            body_limit_bytes: 10 * 1024 * 1024,
            warmup: Warmup::None,
            stages: vec![],
            arrival_rate: 10.0,
            rate_stages: vec![],
        }
    }
}

impl LoadConfig {
    /// Virtual users the run needs: the peak of the schedule.
    pub fn max_users(&self) -> u32 {
        match self.profile {
            Profile::Stages => self
                .stages
                .iter()
                .map(|s| s.target_users)
                .max()
                .unwrap_or(0),
            _ => self.users,
        }
    }
    /// Run length in seconds; for stages the sum of their durations.
    pub fn total_secs(&self) -> u64 {
        match self.profile {
            Profile::Stages => self.stages.iter().map(|s| s.duration_secs).sum(),
            Profile::ArrivalRate if !self.rate_stages.is_empty() => {
                self.rate_stages.iter().map(|s| s.duration_secs).sum()
            }
            _ => self.duration_secs,
        }
    }
    /// The user count over time as (seconds, users) points joined linearly.
    /// Every profile is expressed this way, so the engine has one code path.
    pub fn schedule(&self) -> Vec<(f64, f64)> {
        let users = self.users as f64;
        let total = self.total_secs() as f64;
        match self.profile {
            // Arrival rate: the pool size bounds concurrency; starts follow the rate.
            Profile::Constant | Profile::ArrivalRate => vec![(0.0, users), (total, users)],
            Profile::RampUp => vec![(0.0, 0.0), (self.ramp_secs as f64, users), (total, users)],
            Profile::Stages => {
                let mut points = vec![(0.0, 0.0)];
                let mut at = 0.0;
                for s in &self.stages {
                    at += s.duration_secs as f64;
                    points.push((at, s.target_users as f64));
                }
                points
            }
        }
    }
    /// Load high enough to warrant a target/capacity warning before running.
    pub fn is_elevated(&self) -> bool {
        self.max_users() > 100
            || match self.profile {
                Profile::ArrivalRate => self.peak_rate() > 100.0,
                _ => self.rps_limit > 100,
            }
    }
    /// Short profile description, e.g. `ramp-up 10s` or `stages 60s to 50, 300s to 200`.
    pub fn profile_label(&self) -> String {
        match self.profile {
            Profile::Constant => "constant".into(),
            Profile::RampUp => format!("ramp-up {}s", self.ramp_secs),
            Profile::ArrivalRate if self.rate_stages.is_empty() => format!(
                "arrival rate {}/s, max {} VUs",
                self.arrival_rate, self.users
            ),
            Profile::ArrivalRate => format!(
                "arrival rate {}/s, stages {}, max {} VUs",
                self.arrival_rate,
                self.rate_stages
                    .iter()
                    .map(|s| format!("{}s to {}", s.duration_secs, s.target_rate))
                    .collect::<Vec<_>>()
                    .join(", "),
                self.users
            ),
            Profile::Stages => format!(
                "stages {}",
                self.stages
                    .iter()
                    .map(|s| format!("{}s to {}", s.duration_secs, s.target_users))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
    /// The arrival rate over time as (seconds, iterations/s) points joined
    /// linearly; a constant rate is two points.
    pub fn rate_schedule(&self) -> Vec<(f64, f64)> {
        let mut points = vec![(0.0, self.arrival_rate)];
        if self.rate_stages.is_empty() {
            points.push((self.duration_secs as f64, self.arrival_rate));
        }
        let mut at = 0.0;
        for s in &self.rate_stages {
            at += s.duration_secs as f64;
            points.push((at, s.target_rate));
        }
        points
    }
    /// Highest arrival rate of the schedule, in iterations/s.
    pub fn peak_rate(&self) -> f64 {
        self.rate_stages
            .iter()
            .map(|s| s.target_rate)
            .fold(self.arrival_rate, f64::max)
    }
    /// Seconds into the run when open-model iteration `k` (0-based) starts: the
    /// first moment the integrated arrival rate reaches `k`. A constant rate
    /// gives exactly `k / rate`. `None` once that is at or past the run end.
    pub fn arrival_start(&self, k: u64) -> Option<f64> {
        let total = self.total_secs() as f64;
        let at = if self.rate_stages.is_empty() {
            if self.arrival_rate.is_nan() || self.arrival_rate <= 0.0 {
                return None;
            }
            k as f64 / self.arrival_rate
        } else {
            let mut before = 0.0; // iterations due before the current segment
            let mut found = None;
            for w in self.rate_schedule().windows(2) {
                let ((ta, ra), (tb, rb)) = (w[0], w[1]);
                let (ra, rb, span) = (ra.max(0.0), rb.max(0.0), tb - ta);
                let area = (ra + rb) / 2.0 * span;
                let wanted = k as f64 - before;
                // Zero-rate segments start nothing, so they only shift time.
                if area <= 0.0 || wanted > area {
                    before += area;
                    continue;
                }
                // Solve ra*s + slope*s^2/2 = wanted for s in [0, span]; this
                // form is exact for slope 0 and stable for either slope sign.
                let wanted = wanted.max(0.0);
                let slope = (rb - ra) / span;
                let root = (ra * ra + 2.0 * slope * wanted).max(0.0).sqrt();
                let s = if wanted == 0.0 {
                    0.0
                } else {
                    2.0 * wanted / (ra + root)
                };
                found = Some(ta + s.clamp(0.0, span));
                break;
            }
            found?
        };
        (at < total).then_some(at)
    }
    /// Scheduled (fractional) user count at `t` seconds.
    pub fn target_at(&self, t: f64) -> f64 {
        let points = self.schedule();
        for w in points.windows(2) {
            let ((ta, ua), (tb, ub)) = (w[0], w[1]);
            if t <= tb {
                return if tb > ta {
                    ua + (ub - ua) * ((t - ta) / (tb - ta)).clamp(0.0, 1.0)
                } else {
                    ub
                };
            }
        }
        points.last().map_or(0.0, |p| p.1)
    }
    /// Earliest time at or after `after` when user `index` (0-based) is
    /// scheduled to run, i.e. the user count exceeds `index`; `None` if never.
    pub fn active_from(&self, index: u32, after: f64) -> Option<f64> {
        let index = index as f64;
        for w in self.schedule().windows(2) {
            let ((ta, ua), (tb, ub)) = (w[0], w[1]);
            if tb < after || ua.max(ub) <= index {
                continue;
            }
            let start = ta.max(after);
            if self.target_at(start) > index {
                return Some(start);
            }
            if ub > ua {
                // Rising segment: the count passes `index` at the crossing. A hair
                // after it, so the count is strictly above `index` on waking; and
                // never before `start`, which rounding could otherwise produce.
                let crossing = ta + (index - ua) / (ub - ua) * (tb - ta);
                if crossing <= tb {
                    return Some(crossing.max(start) + 1e-6);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum AuthKind {
    #[default]
    None,
    Basic,
    Bearer,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Auth {
    pub kind: AuthKind,
    pub username: String,
    #[serde(serialize_with = "serialize_secret_reference")]
    pub password: String,
    #[serde(serialize_with = "serialize_secret_reference")]
    pub bearer: String,
}

fn serialize_secret_reference<S: Serializer>(
    value: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    // Preserve a whole variable reference, never literal credentials or mixed text.
    serializer.serialize_str(if is_variable_reference(value) {
        value
    } else {
        ""
    })
}

/// True when `value` is exactly one `{{name}}` variable reference (name of
/// ASCII letters, digits and `_`). Such values are kept when a secret is
/// saved; literal secrets and mixed text never are.
pub fn is_variable_reference(value: &str) -> bool {
    value
        .strip_prefix("{{")
        .and_then(|s| s.strip_suffix("}}"))
        .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Assertions {
    pub status: Option<u16>,
    pub max_ms: Option<u64>,
    pub json_pointer: String,
    pub json_expected: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Extractor {
    pub variable: String,
    pub pointer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
    pub id: u64,
    pub name: String,
    pub method: String,
    pub url: String,
    pub query: Vec<Field>,
    pub headers: Vec<Field>,
    // Bodies are session-only unless explicitly approved for persistence.
    #[serde(serialize_with = "serialize_body")]
    pub body: Body,
    pub auth: Auth,
    pub assertions: Assertions,
    pub extractors: Vec<Extractor>,
    pub timeout_ms: Option<u64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Body {
    pub text: String,
    pub json: bool,
    pub persist: bool,
}
fn serialize_body<S: Serializer>(body: &Body, s: S) -> Result<S::Ok, S::Error> {
    Body {
        text: if body.persist {
            body.text.clone()
        } else {
            String::new()
        },
        ..body.clone()
    }
    .serialize(s)
}
impl Default for Request {
    fn default() -> Self {
        Self {
            id: 1,
            name: "HTTP Request".into(),
            method: "GET".into(),
            url: "http://127.0.0.1:8080/health".into(),
            query: vec![],
            headers: vec![],
            body: Body::default(),
            auth: Auth::default(),
            assertions: Assertions::default(),
            extractors: vec![],
            timeout_ms: None,
        }
    }
}

/// Metric a service-level objective is checked against. Latency and error-rate
/// targets are upper bounds; throughput is a lower bound.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SloMetric {
    P50,
    P90,
    #[default]
    P95,
    P99,
    Mean,
    ErrorRate,
    Throughput,
}
impl SloMetric {
    pub const ALL: [Self; 7] = [
        Self::P50,
        Self::P90,
        Self::P95,
        Self::P99,
        Self::Mean,
        Self::ErrorRate,
        Self::Throughput,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::P50 => "P50 latency",
            Self::P90 => "P90 latency",
            Self::P95 => "P95 latency",
            Self::P99 => "P99 latency",
            Self::Mean => "Mean latency",
            Self::ErrorRate => "Error rate",
            Self::Throughput => "Throughput",
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::ErrorRate => "%",
            Self::Throughput => "req/s",
            _ => "ms",
        }
    }
    pub fn higher_is_better(self) -> bool {
        self == Self::Throughput
    }
}

/// A pass/fail objective. `request_id` scopes it to one request; `None` is the whole run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Threshold {
    pub metric: SloMetric,
    pub value: f64,
    pub request_id: Option<u64>,
}
impl Default for Threshold {
    fn default() -> Self {
        Self {
            metric: SloMetric::P95,
            value: 300.0,
            request_id: None,
        }
    }
}
impl Threshold {
    /// Short form such as `P95 latency ≤ 300 ms`.
    pub fn describe(&self) -> String {
        format!(
            "{} {} {} {}",
            self.metric.label(),
            if self.metric.higher_is_better() {
                "≥"
            } else {
                "≤"
            },
            self.value,
            self.metric.unit()
        )
    }
}
pub const THRESHOLD_LIMIT: usize = 50;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CsvSource {
    pub path: String,
    pub reuse_rows: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TestPlan {
    pub schema_version: u32,
    pub name: String,
    pub load: LoadConfig,
    pub variables: Vec<Field>,
    pub csv: CsvSource,
    pub requests: Vec<Request>,
    pub thresholds: Vec<Threshold>,
}
impl Default for TestPlan {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: "Untitled Load Test".into(),
            load: LoadConfig::default(),
            variables: vec![],
            csv: CsvSource::default(),
            requests: vec![Request::default()],
            thresholds: vec![],
        }
    }
}

pub fn expand(input: &str, vars: &Variables) -> Result<String, String> {
    let mut result = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or("Unclosed variable placeholder")?;
        let key = after[..end].trim();
        result.push_str(
            vars.get(key)
                .ok_or_else(|| format!("Missing variable: {key}"))?,
        );
        rest = &after[end + 2..];
        if result.len() > 1024 * 1024 {
            return Err("Expanded field exceeds 1 MiB".into());
        }
    }
    result.push_str(rest);
    Ok(result)
}

pub fn validate_url(input: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(input).map_err(|_| "Invalid absolute URL".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only HTTP and HTTPS targets are supported".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Credentials in URLs are not allowed; use Auth".into());
    }
    if url.query_pairs().any(|(key, _)| sensitive(&key)) {
        return Err("Move sensitive URL parameters to Params and mark them secret".into());
    }
    Ok(url)
}

impl TestPlan {
    pub fn validate(&self, base: &Path) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err("Unsupported plan schema version".into());
        }
        let c = &self.load;
        if c.profile == Profile::Stages {
            if c.stages.is_empty() || c.stages.len() > STAGE_LIMIT {
                return Err(format!("A stage profile needs 1–{STAGE_LIMIT} stages"));
            }
            if c.stages
                .iter()
                .any(|s| s.duration_secs == 0 || s.target_users > 10_000)
            {
                return Err("Each stage needs at least 1s and at most 10,000 users".into());
            }
        }
        if c.profile == Profile::ArrivalRate {
            let rate_ok = |r: f64| r.is_finite() && (0.0..=MAX_ARRIVAL_RATE).contains(&r);
            if c.rate_stages.is_empty() {
                if !(rate_ok(c.arrival_rate) && c.arrival_rate >= 0.01) {
                    return Err("Arrival rate must be 0.01–100,000 iterations/s".into());
                }
            } else {
                if c.rate_stages.len() > STAGE_LIMIT {
                    return Err(format!("Rate stages are limited to {STAGE_LIMIT}"));
                }
                if !rate_ok(c.arrival_rate)
                    || c.rate_stages
                        .iter()
                        .any(|s| s.duration_secs == 0 || !rate_ok(s.target_rate))
                {
                    return Err(
                        "Each rate stage needs at least 1s and a rate of 0–100,000 iterations/s; the start rate is 0–100,000"
                            .into(),
                    );
                }
                if c.peak_rate() <= 0.0 {
                    return Err("Rate stages need a peak rate above 0 iterations/s".into());
                }
            }
        }
        if !(1..=10_000).contains(&c.max_users())
            || !(1..=86_400).contains(&c.total_secs())
            || !(1..=100_000).contains(&c.rps_limit)
            || !(1..=300_000).contains(&c.timeout_ms)
            || !(1..=100 * 1024 * 1024).contains(&c.body_limit_bytes)
        {
            return Err("Invalid limits: users 1–10,000; duration 1–86,400s; RPS 1–100,000; timeout 1–300,000ms; body 1–100MiB".into());
        }
        if c.profile == Profile::RampUp && (c.ramp_secs == 0 || c.ramp_secs > c.duration_secs) {
            return Err("Ramp-up must be between 1s and the total duration".into());
        }
        match c.warmup {
            Warmup::Duration { secs } if secs == 0 || secs >= c.total_secs() => {
                return Err(
                    "Warmup duration must be at least 1s and shorter than the total duration"
                        .into(),
                );
            }
            Warmup::Requests { count } if !(1..=100_000_000).contains(&count) => {
                return Err("Warmup request count must be 1–100,000,000".into());
            }
            _ => {}
        }
        if self.requests.is_empty() || self.requests.len() > 500 {
            return Err("A plan needs 1–500 requests".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for r in &self.requests {
            if !ids.insert(r.id) {
                return Err("Request IDs must be unique".into());
            }
            if !matches!(
                r.method.as_str(),
                "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
            ) {
                return Err("Unsupported HTTP method".into());
            }
            // Keep destination fixed: variables may be used in path/query, not scheme or authority.
            let prefix = r.url.split_once("://").ok_or("Invalid URL")?;
            let authority = prefix.1.split('/').next().unwrap_or_default();
            if prefix.0.contains("{{") || authority.contains("{{") {
                return Err("Target scheme and host cannot contain variables".into());
            }
            validate_url(&r.url.replace("{{", "var_").replace("}}", ""))?;
            if r.timeout_ms.is_some_and(|t| t == 0 || t > 300_000) {
                return Err("Request timeout must be 1–300,000 ms".into());
            }
            for field in r.headers.iter().chain(&r.query).chain(&self.variables) {
                if field.name.trim().is_empty() {
                    return Err("Field names cannot be empty".into());
                }
                if (field.secret || sensitive(&field.name)) && field.value.is_empty() {
                    return Err(format!("Re-enter secret value: {}", field.name));
                }
            }
            if r.auth.kind == AuthKind::Basic && r.auth.password.is_empty() {
                return Err("Re-enter Basic Auth password".into());
            }
            if r.auth.kind == AuthKind::Bearer && r.auth.bearer.is_empty() {
                return Err("Re-enter Bearer token or variable reference".into());
            }
            if r.assertions
                .status
                .is_some_and(|s| !(100..=599).contains(&s))
            {
                return Err("Expected status must be 100–599".into());
            }
            if !r.assertions.json_pointer.is_empty() {
                serde_json::from_str::<serde_json::Value>(&r.assertions.json_expected)
                    .map_err(|_| "JSON assertion expected value must be valid JSON")?;
            }
            for e in &r.extractors {
                if e.variable.trim().is_empty()
                    || (!e.pointer.is_empty() && !e.pointer.starts_with('/'))
                {
                    return Err(
                        "Extractor requires a variable name and RFC 6901 JSON pointer".into(),
                    );
                }
            }
        }
        if self.thresholds.len() > THRESHOLD_LIMIT {
            return Err(format!(
                "A plan can have at most {THRESHOLD_LIMIT} SLO thresholds"
            ));
        }
        for t in &self.thresholds {
            let valid = t.value.is_finite()
                && match t.metric {
                    SloMetric::ErrorRate => (0.0..=100.0).contains(&t.value),
                    _ => t.value > 0.0,
                };
            if !valid {
                return Err(format!(
                    "Invalid SLO target for {}: latency and throughput must be above 0, error rate 0–100%",
                    t.metric.label()
                ));
            }
            if t.request_id.is_some_and(|id| !ids.contains(&id)) {
                return Err("An SLO threshold refers to a request that no longer exists".into());
            }
        }
        self.csv_rows(base)?;
        Ok(())
    }

    pub fn csv_rows(&self, base: &Path) -> Result<Vec<Variables>, String> {
        if self.csv.path.is_empty() {
            return Ok(vec![]);
        }
        let path = base.join(&self.csv.path);
        let meta = std::fs::metadata(&path).map_err(|_| "CSV file cannot be opened".to_owned())?;
        if meta.len() > 16 * 1024 * 1024 {
            return Err("CSV exceeds 16 MiB".into());
        }
        let mut reader = csv::Reader::from_path(path).map_err(|_| "Cannot read CSV")?;
        let headers = reader.headers().map_err(|_| "Invalid CSV header")?.clone();
        let unique: std::collections::BTreeSet<_> = headers.iter().collect();
        if unique.len() != headers.len() || headers.iter().any(|h| h.is_empty()) {
            return Err("CSV headers must be nonempty and unique".into());
        }
        let rows = reader
            .records()
            .map(|r| {
                r.map(|r| {
                    headers
                        .iter()
                        .zip(r.iter())
                        .map(|(k, v)| (k.to_owned(), v.to_owned()))
                        .collect()
                })
                .map_err(|_| "Malformed CSV row or missing column".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty() || (!self.csv.reuse_rows && rows.len() < self.load.max_users() as usize)
        {
            return Err("CSV needs one row per user, or enable Reuse rows".into());
        }
        Ok(rows)
    }

    pub fn redacted(&self) -> Self {
        let mut plan = self.clone();
        for r in &mut plan.requests {
            // Stored snapshots do not need executable templates or request bodies.
            r.body.text.clear();
            r.auth.username.clear();
            r.assertions.json_expected = "null".into();
            if let Ok(mut u) = url::Url::parse(&r.url) {
                u.set_query(None);
                u.set_fragment(None);
                r.url = u.to_string();
            }
        }
        serde_json::from_value(serde_json::to_value(plan).expect("plan serialization"))
            .expect("plan deserialization")
    }
}

pub fn evaluate(assertions: &Assertions, status: u16, ms: u64, body: &[u8]) -> bool {
    let json = if assertions.json_pointer.is_empty() {
        None
    } else {
        serde_json::from_slice(body).ok()
    };
    evaluate_parsed(assertions, status, ms, json.as_ref())
}

/// Same as [`evaluate`] for a body that was already parsed; `None` means not JSON.
pub fn evaluate_parsed(
    assertions: &Assertions,
    status: u16,
    ms: u64,
    json: Option<&serde_json::Value>,
) -> bool {
    assertion_failure(assertions, status, ms, json).is_none()
}

/// The first assertion a response fails, checked as status, time, then JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssertionFailure {
    Status { actual: u16, expected: Option<u16> },
    Slower { max_ms: u64 },
    Json { pointer: String },
}
impl AssertionFailure {
    /// Failure reason built from the plan and status code only, never from
    /// response content.
    pub fn reason(&self) -> String {
        match self {
            Self::Status {
                actual,
                expected: Some(expected),
            } => format!("HTTP {actual}, expected {expected}"),
            Self::Status { actual, .. } => format!("HTTP {actual}, expected 2xx"),
            Self::Slower { max_ms } => format!("Slower than the {max_ms} ms limit"),
            Self::Json { pointer } => {
                format!("JSON at {} did not match", clip(pointer, 80))
            }
        }
    }
}
pub fn assertion_failure(
    assertions: &Assertions,
    status: u16,
    ms: u64,
    json: Option<&serde_json::Value>,
) -> Option<AssertionFailure> {
    let status_ok = assertions
        .status
        .map_or((200..300).contains(&status), |expected| expected == status);
    if !status_ok {
        return Some(AssertionFailure::Status {
            actual: status,
            expected: assertions.status,
        });
    }
    if let Some(max_ms) = assertions.max_ms.filter(|max| ms > *max) {
        return Some(AssertionFailure::Slower { max_ms });
    }
    let json_ok = assertions.json_pointer.is_empty()
        || json.is_some_and(|json| {
            serde_json::from_str::<serde_json::Value>(&assertions.json_expected)
                .ok()
                .is_some_and(|v| json.pointer(&assertions.json_pointer) == Some(&v))
        });
    (!json_ok).then(|| AssertionFailure::Json {
        pointer: assertions.json_pointer.clone(),
    })
}

/// At most `max` characters, with an ellipsis when cut.
pub fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

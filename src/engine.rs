use crate::{
    core::{self, AuthKind, Profile, Request, TestPlan, Variables},
    metrics::*,
    storage::History,
};
use base64::Engine as _;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
    time::Instant,
};
use tokio_util::sync::CancellationToken;

pub struct RunHandle {
    pub snapshots: watch::Receiver<Snapshot>,
    pub stop: CancellationToken,
}
enum Persist {
    Point(u64, Point),
    Finish(Box<Snapshot>),
}

pub fn start(
    runtime: &tokio::runtime::Runtime,
    plan: TestPlan,
    base: &Path,
    db: PathBuf,
) -> Result<RunHandle, String> {
    plan.validate(base)?;
    let rows = plan.csv_rows(base)?;
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    // Check persistence before sending any HTTP requests.
    let history = History::open(&db)?;
    history.begin(&id, &plan)?;
    let initial = Snapshot {
        run_id: id.clone(),
        name: plan.name.clone(),
        state: RunState::Running,
        target_users: plan.load.users,
        ..Snapshot::default()
    };
    let (tx, rx) = watch::channel(initial);
    let stop = CancellationToken::new();
    let (persist_tx, mut persist_rx) = mpsc::channel(32);
    let (persist_result_tx, persist_result_rx) = tokio::sync::oneshot::channel();
    let persist_id = id.clone();
    std::thread::spawn(move || {
        let result = (|| {
            while let Some(event) = persist_rx.blocking_recv() {
                match event {
                    Persist::Point(seq, point) => history.point(&persist_id, seq, &point)?,
                    Persist::Finish(snapshot) => {
                        history.finish(&snapshot)?;
                        break;
                    }
                }
            }
            Ok::<(), String>(())
        })();
        let _ = persist_result_tx.send(result);
    });
    runtime.spawn(coordinate(
        Arc::new(plan),
        rows,
        id,
        stop.clone(),
        tx,
        persist_tx,
        persist_result_rx,
    ));
    Ok(RunHandle {
        snapshots: rx,
        stop,
    })
}

#[derive(Clone)]
struct Context {
    plan: Arc<TestPlan>,
    client: reqwest::Client,
    local_client: reqwest::Client,
    metrics: Arc<Mutex<Accumulator>>,
    gate: Arc<tokio::sync::Mutex<Instant>>,
    stop: CancellationToken,
    force: CancellationToken,
    began: Instant,
    deadline: Instant,
}

async fn coordinate(
    plan: Arc<TestPlan>,
    rows: Vec<Variables>,
    id: String,
    stop: CancellationToken,
    tx: watch::Sender<Snapshot>,
    persistence: mpsc::Sender<Persist>,
    persisted: tokio::sync::oneshot::Receiver<Result<(), String>>,
) {
    let began = Instant::now();
    let metrics = Arc::new(Mutex::new(Accumulator::default()));
    {
        let mut m = metrics.lock().expect("metrics lock");
        for r in &plan.requests {
            m.endpoints.insert(
                r.id,
                EndpointAccumulator {
                    summary: Endpoint {
                        id: r.id,
                        name: r.name.clone(),
                        method: r.method.clone(),
                        ..Endpoint::default()
                    },
                    histogram: histogram(),
                },
            );
        }
        m.log(
            0.0,
            "INFO",
            "Run started; HTTP redirects, retries and cookies disabled".into(),
        );
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(plan.load.users.min(256) as usize)
        .build();
    let local_client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(plan.load.users.min(256) as usize)
        .build();
    let mut issue = None;
    let mut tasks = JoinSet::new();
    let force = CancellationToken::new();
    if let (Ok(client), Ok(local_client)) = (client, local_client) {
        let context = Context {
            plan: plan.clone(),
            client,
            local_client,
            metrics: metrics.clone(),
            gate: Arc::new(tokio::sync::Mutex::new(began)),
            stop: stop.clone(),
            force: force.clone(),
            began,
            deadline: began + Duration::from_secs(plan.load.duration_secs),
        };
        for user in 0..plan.load.users {
            let c = context.clone();
            let mut vars: Variables = plan
                .variables
                .iter()
                .map(|f| (f.name.clone(), f.value.clone()))
                .collect();
            if !rows.is_empty() {
                vars.extend(rows[user as usize % rows.len()].clone());
            }
            let delay = if plan.load.profile == Profile::RampUp {
                Duration::from_secs_f64(
                    plan.load.ramp_secs as f64 * user as f64 / plan.load.users as f64,
                )
            } else {
                Duration::ZERO
            };
            tasks.spawn(async move { user_loop(c, vars, delay).await });
        }
    } else {
        issue = Some("HTTP client initialization failed".to_owned());
    }
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let deadline = began + Duration::from_secs(plan.load.duration_secs);
    let mut draining: Option<Instant> = None;
    let mut user_stopped = false;
    let mut sequence = 0;
    let mut last_bucket = began;
    let mut persistence_failed = false;
    loop {
        tokio::select! {
            _ = interval.tick() => {},
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if joined.is_some_and(|j| j.is_err()) { issue = Some("A virtual user terminated unexpectedly".into()); stop.cancel(); }
            }
        }
        let now = Instant::now();
        if draining.is_none() && (stop.is_cancelled() || now >= deadline || issue.is_some()) {
            user_stopped = stop.is_cancelled() && issue.is_none();
            stop.cancel();
            draining = Some(now + Duration::from_secs(5));
        }
        if draining.is_some_and(|end| now >= end) {
            force.cancel();
        }
        let elapsed = began.elapsed().as_secs_f64();
        let point = if elapsed.floor() > last_bucket.duration_since(began).as_secs_f64().floor() {
            last_bucket = now;
            Some(metrics.lock().expect("metrics lock").point(elapsed))
        } else {
            None
        };
        if let Some(point) = point {
            if persistence
                .send(Persist::Point(sequence, point))
                .await
                .is_err()
            {
                persistence_failed = true;
            }
            sequence += 1;
        }
        let mut snapshot = metrics.lock().expect("metrics lock").snapshot(elapsed);
        snapshot.run_id.clone_from(&id);
        snapshot.name.clone_from(&plan.name);
        snapshot.target_users = plan.load.users;
        snapshot.state = if draining.is_some() {
            RunState::Stopping
        } else {
            RunState::Running
        };
        if persistence_failed {
            snapshot.issue =
                Some("History writer unavailable; export results before closing".into());
        }
        if tasks.is_empty() {
            let point = metrics.lock().expect("metrics lock").point(elapsed);
            if persistence
                .send(Persist::Point(sequence, point))
                .await
                .is_err()
            {
                persistence_failed = true;
            }
            snapshot = metrics.lock().expect("metrics lock").snapshot(elapsed);
            snapshot.run_id.clone_from(&id);
            snapshot.name.clone_from(&plan.name);
            snapshot.target_users = plan.load.users;
            snapshot.state = if issue.is_some() {
                RunState::Failed
            } else if user_stopped {
                RunState::Cancelled
            } else {
                RunState::Completed
            };
            snapshot.issue = issue;
            let _ = persistence
                .send(Persist::Finish(Box::new(snapshot.clone())))
                .await;
            drop(persistence);
            match persisted.await {
                Ok(Ok(())) if !persistence_failed => {}
                _ => {
                    snapshot.issue = Some(
                        "History persistence failed; export current results before closing".into(),
                    )
                }
            }
            tx.send_replace(snapshot);
            break;
        }
        tx.send_replace(snapshot);
    }
}

async fn user_loop(c: Context, mut vars: Variables, delay: Duration) {
    tokio::select! { _ = c.stop.cancelled() => return, _ = tokio::time::sleep(delay) => {} }
    if Instant::now() >= c.deadline {
        return;
    }
    c.metrics.lock().expect("metrics lock").users += 1;
    'scenario: loop {
        for request in &c.plan.requests {
            if !permit(&c).await {
                break 'scenario;
            }
            {
                let mut m = c.metrics.lock().expect("metrics lock");
                m.counts.started += 1;
                m.endpoints
                    .get_mut(&request.id)
                    .expect("endpoint")
                    .summary
                    .counts
                    .started += 1;
            }
            let started = Instant::now();
            let result = tokio::select! {
                biased;
                _ = c.force.cancelled() => Err(Failure::Cancelled),
                result = execute(&c, request, &vars) => result,
            };
            let elapsed_us = result
                .as_ref()
                .map(|(_, _, us)| *us)
                .unwrap_or_else(|_| started.elapsed().as_micros().clamp(1, 3_600_000_000) as u64);
            // Evaluate before taking the shared metrics lock; bodies can be up to the
            // configured limit and are parsed at most once.
            let mut assertion_failed = false;
            let mut status_failed = false;
            let mut extracted = Variables::new();
            if let Ok((status, body, _)) = &result {
                status_failed = request
                    .assertions
                    .status
                    .map_or(!(200..300).contains(status), |expected| expected != *status);
                let json = if request.assertions.json_pointer.is_empty()
                    && request.extractors.is_empty()
                {
                    None
                } else {
                    serde_json::from_slice::<serde_json::Value>(body).ok()
                };
                let mut content_assertions = request.assertions.clone();
                content_assertions.status = Some(*status);
                assertion_failed = !core::evaluate_parsed(
                    &content_assertions,
                    *status,
                    elapsed_us.div_ceil(1000),
                    json.as_ref(),
                );
                for e in &request.extractors {
                    match json.as_ref().and_then(|json| json.pointer(&e.pointer)) {
                        Some(serde_json::Value::String(s)) => {
                            extracted.insert(e.variable.clone(), s.clone());
                        }
                        Some(value) => {
                            extracted.insert(e.variable.clone(), value.to_string());
                        }
                        None => {
                            assertion_failed = true;
                        }
                    }
                }
            }
            let mut m = c.metrics.lock().expect("metrics lock");
            if let Ok((status, _, _)) = &result {
                let _ = m.total.record(elapsed_us);
                let _ = m.bucket.record(elapsed_us);
                *m.statuses.entry(*status).or_default() += 1;
                let _ = m
                    .endpoints
                    .get_mut(&request.id)
                    .expect("endpoint")
                    .histogram
                    .record(elapsed_us);
            }
            let success = result.is_ok() && !assertion_failed && !status_failed;
            apply_count(
                &mut m.counts,
                &result,
                success,
                assertion_failed,
                status_failed,
            );
            apply_count(
                &mut m
                    .endpoints
                    .get_mut(&request.id)
                    .expect("endpoint")
                    .summary
                    .counts,
                &result,
                success,
                assertion_failed,
                status_failed,
            );
            if !success || m.counts.completed.is_multiple_of(100) {
                let label = match &result {
                    Ok((status, _, _)) => format!(
                        "Request #{} · HTTP {} · {} · {:.1} ms",
                        request.id,
                        status,
                        if success {
                            "passed"
                        } else {
                            "expectation/extraction failed"
                        },
                        elapsed_us as f64 / 1000.0
                    ),
                    Err(e) => format!("Request #{} · {}", request.id, e.label()),
                };
                m.log(
                    c.began.elapsed().as_secs_f64(),
                    if success { "INFO" } else { "ERROR" },
                    label,
                );
            }
            drop(m);
            if !success {
                break;
            } // Failed step prevents dependent requests using stale tokens.
            vars.extend(extracted);
        }
        tokio::select! { _ = c.stop.cancelled() => break, _ = tokio::time::sleep(Duration::from_millis(c.plan.load.think_ms.max(1))) => {} }
        if Instant::now() >= c.deadline {
            break;
        }
    }
    c.metrics.lock().expect("metrics lock").users -= 1;
}

/// Schedule lag the gate may catch up on. Covers timer wake-up rounding without
/// letting idle periods bank a burst of starts.
const MAX_GATE_LAG: Duration = Duration::from_millis(10);

async fn permit(c: &Context) -> bool {
    let mut gate =
        tokio::select! { _ = c.stop.cancelled() => return false, guard = c.gate.lock() => guard };
    let now = Instant::now();
    let slot = (*gate).max(now.checked_sub(MAX_GATE_LAG).unwrap_or(now));
    if slot > now {
        tokio::select! { _ = c.stop.cancelled() => return false, _ = tokio::time::sleep_until(slot) => {} }
    }
    if c.stop.is_cancelled() || Instant::now() >= c.deadline {
        return false;
    }
    // Advance from the scheduled slot, not the wake-up time: timers round sleeps up
    // to whole milliseconds, which previously cut the achieved rate well below the limit.
    *gate = slot + Duration::from_secs_f64(1.0 / c.plan.load.rps_limit as f64);
    true
}

#[derive(Debug)]
enum Failure {
    Cancelled,
    Timeout,
    Network,
    BodyTooLarge,
    Template,
}
impl Failure {
    fn label(&self) -> &'static str {
        match self {
            Self::Cancelled => "Cancelled",
            Self::Timeout => "Timeout",
            Self::Network => "Network/TLS/body transport failure",
            Self::BodyTooLarge => "ResponseTooLarge",
            Self::Template => "Invalid template, URL or header (check variables)",
        }
    }
}
fn apply_count(
    count: &mut Counts,
    result: &Result<(u16, Vec<u8>, u64), Failure>,
    success: bool,
    assertion: bool,
    status_failed: bool,
) {
    if matches!(result, Err(Failure::Cancelled)) {
        count.cancelled += 1;
        return;
    }
    count.completed += 1;
    if let Ok((_, body, _)) = result {
        count.bytes += body.len() as u64;
    }
    if success {
        count.successful += 1;
    } else {
        count.failed += 1;
    }
    if assertion {
        count.assertion_errors += 1;
    }
    if status_failed {
        count.status_errors += 1;
    }
    match result {
        Ok((400..=599, _, _)) => count.http_errors += 1,
        Err(Failure::Timeout) => count.timeouts += 1,
        Err(Failure::Network) => count.network_errors += 1,
        Err(Failure::BodyTooLarge) => count.body_errors += 1,
        Err(Failure::Template) => count.preparation_errors += 1,
        _ => {}
    }
}
async fn execute(
    c: &Context,
    r: &Request,
    vars: &Variables,
) -> Result<(u16, Vec<u8>, u64), Failure> {
    let exp = |s: &str| core::expand(s, vars).map_err(|_| Failure::Template);
    let raw = exp(&r.url)?;
    let url = core::validate_url(&raw).map_err(|_| Failure::Template)?;
    let loopback = url.host_str().is_some_and(|h| {
        h == "localhost"
            || h.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
            || h == "[::1]"
    });
    let client = if loopback { &c.local_client } else { &c.client };
    let method = reqwest::Method::from_bytes(r.method.as_bytes()).map_err(|_| Failure::Template)?;
    let mut request = client.request(method, url);
    for q in &r.query {
        request = request.query(&[(q.name.clone(), exp(&q.value)?)]);
    }
    for h in &r.headers {
        let name = reqwest::header::HeaderName::from_bytes(h.name.as_bytes())
            .map_err(|_| Failure::Template)?;
        let value = reqwest::header::HeaderValue::from_str(&exp(&h.value)?)
            .map_err(|_| Failure::Template)?;
        request = request.header(name, value);
    }
    match r.auth.kind {
        AuthKind::None => {}
        AuthKind::Bearer => request = request.bearer_auth(exp(&r.auth.bearer)?),
        AuthKind::Basic => {
            request = request.header(
                reqwest::header::AUTHORIZATION,
                format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD.encode(format!(
                        "{}:{}",
                        exp(&r.auth.username)?,
                        exp(&r.auth.password)?
                    ))
                ),
            )
        }
    }
    if !r.body.text.is_empty() {
        let body = exp(&r.body.text)?;
        if r.body.json {
            serde_json::from_str::<serde_json::Value>(&body).map_err(|_| Failure::Template)?;
            request = request.header(reqwest::header::CONTENT_TYPE, "application/json");
        }
        request = request.body(body);
    }
    let timeout = Duration::from_millis(r.timeout_ms.unwrap_or(c.plan.load.timeout_ms));
    let http_started = Instant::now();
    tokio::time::timeout(timeout, async {
        let mut response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                Failure::Timeout
            } else {
                Failure::Network
            }
        })?;
        let status = response.status().as_u16();
        let limit = c.plan.load.body_limit_bytes;
        if response
            .content_length()
            .is_some_and(|len| len > limit as u64)
        {
            return Err(Failure::BodyTooLarge);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Network)? {
            if body.len().saturating_add(chunk.len()) > limit {
                return Err(Failure::BodyTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok((
            status,
            body,
            http_started.elapsed().as_micros().clamp(1, 3_600_000_000) as u64,
        ))
    })
    .await
    .map_err(|_| Failure::Timeout)?
}

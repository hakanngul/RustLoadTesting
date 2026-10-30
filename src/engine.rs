use crate::{
    core::{self, AuthKind, Request, TestPlan, Variables, Warmup},
    metrics::*,
    storage::History,
};
use base64::Engine as _;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
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
        target_users: plan.load.max_users(),
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
    /// Request starts so far, for request-count warmup.
    starts: Arc<AtomicU64>,
}

/// Marks the measured window: warmup setting, where measurement starts and
/// whether warmup is still running.
fn mark_warmup(s: &mut Snapshot, warmup: Warmup, ended_at: Option<f64>) {
    s.warmup = warmup;
    (s.measured_from, s.warming_up) = match warmup {
        Warmup::None => (0.0, false),
        Warmup::Duration { secs } => {
            let secs = secs as f64;
            (secs.min(s.elapsed), s.elapsed < secs)
        }
        Warmup::Requests { .. } => match ended_at {
            Some(at) => (at, false),
            None => (s.elapsed, true),
        },
    };
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
        .pool_max_idle_per_host(plan.load.max_users().min(256) as usize)
        .build();
    let local_client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(plan.load.max_users().min(256) as usize)
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
            deadline: began + Duration::from_secs(plan.load.total_secs()),
            starts: Arc::new(AtomicU64::new(0)),
        };
        let pool: Vec<Variables> = (0..plan.load.max_users())
            .map(|user| {
                let mut vars: Variables = plan
                    .variables
                    .iter()
                    .map(|f| (f.name.clone(), f.value.clone()))
                    .collect();
                if !rows.is_empty() {
                    vars.extend(rows[user as usize % rows.len()].clone());
                }
                vars
            })
            .collect();
        if plan.load.profile == core::Profile::ArrivalRate {
            tasks.spawn(arrivals(context, pool));
        } else {
            for (user, vars) in pool.into_iter().enumerate() {
                let c = context.clone();
                tasks.spawn(async move { user_loop(c, vars, user as u32).await });
            }
        }
    } else {
        issue = Some("HTTP client initialization failed".to_owned());
    }
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let deadline = began + Duration::from_secs(plan.load.total_secs());
    let mut draining: Option<Instant> = None;
    let mut user_stopped = false;
    let mut sequence = 0;
    let mut last_bucket = began;
    let mut persistence_failed = false;
    let arrival_rate =
        (plan.load.profile == core::Profile::ArrivalRate).then_some(plan.load.arrival_rate);
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
        let (mut snapshot, ended_at) = {
            let m = metrics.lock().expect("metrics lock");
            (m.snapshot(elapsed), m.warmup_ended_at)
        };
        mark_warmup(&mut snapshot, plan.load.warmup, ended_at);
        snapshot.arrival_rate = arrival_rate;
        snapshot.run_id.clone_from(&id);
        snapshot.name.clone_from(&plan.name);
        snapshot.target_users = plan.load.max_users();
        snapshot.state = if draining.is_some() {
            RunState::Stopping
        } else {
            RunState::Running
        };
        snapshot.slo = evaluate_thresholds(&snapshot, &plan.thresholds);
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
            let ended_at = {
                let m = metrics.lock().expect("metrics lock");
                snapshot = m.snapshot(elapsed);
                m.warmup_ended_at
            };
            mark_warmup(&mut snapshot, plan.load.warmup, ended_at);
            snapshot.arrival_rate = arrival_rate;
            snapshot.run_id.clone_from(&id);
            snapshot.name.clone_from(&plan.name);
            snapshot.target_users = plan.load.max_users();
            snapshot.state = if issue.is_some() {
                RunState::Failed
            } else if user_stopped {
                RunState::Cancelled
            } else {
                RunState::Completed
            };
            snapshot.issue = issue;
            // Final verdict is computed after the last counts and state are known.
            snapshot.slo = evaluate_thresholds(&snapshot, &plan.thresholds);
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

/// Virtual user `index` runs while the schedule's user count exceeds its index.
/// It joins or leaves only between iterations, so ramp-down lets the current
/// iteration finish.
async fn user_loop(c: Context, mut vars: Variables, index: u32) {
    let mut active = false;
    loop {
        let now = c.began.elapsed().as_secs_f64();
        if c.plan.load.target_at(now) <= index as f64 {
            if active {
                c.metrics.lock().expect("metrics lock").users -= 1;
                active = false;
            }
            let Some(at) = c.plan.load.active_from(index, now) else {
                break;
            };
            let wake = c.began + Duration::from_secs_f64(at);
            if wake >= c.deadline {
                break;
            }
            tokio::select! { _ = c.stop.cancelled() => break, _ = tokio::time::sleep_until(wake) => {} }
            continue;
        }
        if !active {
            c.metrics.lock().expect("metrics lock").users += 1;
            active = true;
        }
        if !run_iteration(&c, &mut vars).await {
            break;
        }
        tokio::select! { _ = c.stop.cancelled() => break, _ = tokio::time::sleep(Duration::from_millis(c.plan.load.think_ms.max(1))) => {} }
        if Instant::now() >= c.deadline {
            break;
        }
    }
    if active {
        c.metrics.lock().expect("metrics lock").users -= 1;
    }
}

/// One pass through the scenario. A failed step skips the rest of the
/// iteration; returns `false` once no further request may start (stop or deadline).
async fn run_iteration(c: &Context, vars: &mut Variables) -> bool {
    for request in &c.plan.requests {
        if !permit(c).await {
            return false;
        }
        // Classified by start: a warmup request stays warmup even if it ends later.
        let warm = match c.plan.load.warmup {
            Warmup::None => false,
            Warmup::Duration { secs } => c.began.elapsed() < Duration::from_secs(secs),
            Warmup::Requests { count } => c.starts.fetch_add(1, Ordering::Relaxed) < count,
        };
        {
            let mut m = c.metrics.lock().expect("metrics lock");
            if warm {
                m.warmup.started += 1;
            } else {
                if c.plan.load.warmup != Warmup::None && m.warmup_ended_at.is_none() {
                    m.warmup_ended_at = Some(c.began.elapsed().as_secs_f64());
                }
                m.counts.started += 1;
                m.endpoints
                    .get_mut(&request.id)
                    .expect("endpoint")
                    .summary
                    .counts
                    .started += 1;
            }
        }
        let started = Instant::now();
        let result = tokio::select! {
            biased;
            _ = c.force.cancelled() => Err(Failure::Cancelled),
            result = execute(c, request, vars) => result,
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
        // First failure reason, sanitized: never response content or error text.
        let mut reason = match &result {
            Err(Failure::Cancelled) | Ok(_) => None,
            Err(e) => Some(e.label().to_owned()),
        };
        if let Ok((status, body, _)) = &result {
            status_failed = request
                .assertions
                .status
                .map_or(!(200..300).contains(status), |expected| expected != *status);
            if status_failed {
                reason = Some(
                    core::AssertionFailure::Status {
                        actual: *status,
                        expected: request.assertions.status,
                    }
                    .reason(),
                );
            }
            let json =
                if request.assertions.json_pointer.is_empty() && request.extractors.is_empty() {
                    None
                } else {
                    serde_json::from_slice::<serde_json::Value>(body).ok()
                };
            let mut content_assertions = request.assertions.clone();
            content_assertions.status = Some(*status);
            if let Some(failure) = core::assertion_failure(
                &content_assertions,
                *status,
                elapsed_us.div_ceil(1000),
                json.as_ref(),
            ) {
                assertion_failed = true;
                reason.get_or_insert_with(|| failure.reason());
            }
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
                        reason.get_or_insert_with(|| {
                            format!("Extraction found nothing at {}", core::clip(&e.pointer, 80))
                        });
                    }
                }
            }
        }
        let mut m = c.metrics.lock().expect("metrics lock");
        let success = result.is_ok() && !assertion_failed && !status_failed;
        if warm {
            apply_count(
                &mut m.warmup,
                &result,
                success,
                assertion_failed,
                status_failed,
            );
        } else if let Ok((status, _, _)) = &result {
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
        if !warm {
            record_counts(
                &mut m,
                request.id,
                &result,
                success,
                assertion_failed,
                status_failed,
            );
            if let (false, Some(reason)) = (success, &reason) {
                m.record_error(request.id, reason.clone(), c.began.elapsed().as_secs_f64());
            }
        }
        let logged = if warm {
            m.warmup.completed
        } else {
            m.counts.completed
        };
        if !success || logged.is_multiple_of(100) {
            let label = match &result {
                Ok((status, _, _)) => format!(
                    "{}Request #{} · {} · {:.1} ms",
                    if warm { "Warmup · " } else { "" },
                    request.id,
                    match (success, &reason) {
                        // A status reason already names the status.
                        (false, Some(reason)) if status_failed => reason.clone(),
                        (false, Some(reason)) => format!("HTTP {status} · {reason}"),
                        _ => format!("HTTP {status} · passed"),
                    },
                    elapsed_us as f64 / 1000.0
                ),
                Err(e) => format!(
                    "{}Request #{} · {}",
                    if warm { "Warmup · " } else { "" },
                    request.id,
                    reason.as_deref().unwrap_or(e.label())
                ),
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
    true
}

/// Open model: starts an iteration every `1 / arrival_rate` seconds on an idle
/// VU from the pool. With every VU busy the iteration is dropped and counted,
/// so a slow target cannot lower the offered load silently.
async fn arrivals(c: Context, pool: Vec<Variables>) {
    let idle = Arc::new(Mutex::new(pool));
    let interval = 1.0 / c.plan.load.arrival_rate;
    let mut running = JoinSet::new();
    for k in 0u64.. {
        let at = c.began + Duration::from_secs_f64(k as f64 * interval);
        if at >= c.deadline {
            break;
        }
        tokio::select! { _ = c.stop.cancelled() => break, _ = tokio::time::sleep_until(at) => {} }
        while let Some(done) = running.try_join_next() {
            propagate_panic(done);
        }
        let vu = idle.lock().expect("pool lock").pop();
        let Some(mut vars) = vu else {
            let warm = match c.plan.load.warmup {
                Warmup::None => false,
                Warmup::Duration { secs } => c.began.elapsed() < Duration::from_secs(secs),
                Warmup::Requests { count } => c.starts.load(Ordering::Relaxed) < count,
            };
            let mut m = c.metrics.lock().expect("metrics lock");
            if warm {
                m.warmup.dropped_iterations += 1;
            } else {
                m.counts.dropped_iterations += 1;
            }
            continue;
        };
        let (c, idle) = (c.clone(), idle.clone());
        running.spawn(async move {
            c.metrics.lock().expect("metrics lock").users += 1;
            run_iteration(&c, &mut vars).await;
            c.metrics.lock().expect("metrics lock").users -= 1;
            idle.lock().expect("pool lock").push(vars);
        });
    }
    while let Some(done) = running.join_next().await {
        propagate_panic(done);
    }
}

/// Re-raises an iteration panic so the coordinator reports the run as failed.
fn propagate_panic(done: Result<(), tokio::task::JoinError>) {
    if let Err(e) = done
        && e.is_panic()
    {
        std::panic::resume_unwind(e.into_panic());
    }
}

/// Counts a measured (non-warmup) result for the run and its endpoint.
fn record_counts(
    m: &mut Accumulator,
    id: u64,
    result: &Result<(u16, Vec<u8>, u64), Failure>,
    success: bool,
    assertion_failed: bool,
    status_failed: bool,
) {
    apply_count(
        &mut m.counts,
        result,
        success,
        assertion_failed,
        status_failed,
    );
    let endpoint = &mut m.endpoints.get_mut(&id).expect("endpoint").summary.counts;
    apply_count(endpoint, result, success, assertion_failed, status_failed);
}

/// Schedule lag the gate may catch up on. Covers timer wake-up rounding without
/// letting idle periods bank a burst of starts.
const MAX_GATE_LAG: Duration = Duration::from_millis(10);

async fn permit(c: &Context) -> bool {
    if c.plan.load.profile == core::Profile::ArrivalRate {
        // The arrival schedule paces starts; the RPS cap does not apply.
        return !c.stop.is_cancelled() && Instant::now() < c.deadline;
    }
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
    Network(Network),
    BodyTooLarge,
    Template,
}
impl Failure {
    fn label(&self) -> &'static str {
        match self {
            Self::Cancelled => "Cancelled",
            Self::Timeout => "Timeout",
            Self::Network(cause) => cause.label(),
            Self::BodyTooLarge => "ResponseTooLarge",
            Self::Template => "Invalid template, URL or header (check variables)",
        }
    }
}
/// Transport failure cause, derived from error kinds only: reqwest's error
/// text can contain the request URL, so it is never shown or stored.
#[derive(Debug, Clone, Copy)]
enum Network {
    Refused,
    Closed,
    Connect,
    BodyRead,
    Other,
}
impl Network {
    fn classify(e: &reqwest::Error) -> Self {
        let mut source = std::error::Error::source(e);
        while let Some(err) = source {
            if let Some(io) = err.downcast_ref::<std::io::Error>() {
                use std::io::ErrorKind::*;
                match io.kind() {
                    ConnectionRefused => return Self::Refused,
                    ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof => {
                        return Self::Closed;
                    }
                    _ => {}
                }
            }
            source = err.source();
        }
        if e.is_connect() {
            Self::Connect
        } else {
            Self::Other
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Refused => "Connection refused",
            Self::Closed => "Connection reset or closed",
            Self::Connect => "Connection failed (DNS, TLS or TCP)",
            Self::BodyRead => "Response body read failed",
            Self::Other => "Request failed before a response",
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
        Err(Failure::Network(_)) => count.network_errors += 1,
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
                Failure::Network(Network::classify(&e))
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
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Failure::Network(Network::BodyRead))?
        {
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

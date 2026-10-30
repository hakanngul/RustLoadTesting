use rustload::{core::*, engine, fixture, metrics::*, report, storage::History};
use std::{
    path::Path,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap()
}
fn wait(runtime: &tokio::runtime::Runtime, handle: &mut engine::RunHandle) -> Snapshot {
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let s = handle.snapshots.borrow_and_update().clone();
                if s.state.terminal() {
                    return s;
                }
                handle.snapshots.changed().await.unwrap();
            }
        })
        .await
        .expect("run finished within deadline")
    })
}

#[test]
fn hundred_users_real_scenario_history_and_exports() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = fixture::demo_plan(server.address);
    plan.load.users = 100;
    plan.load.duration_secs = 3;
    plan.load.profile = Profile::RampUp;
    plan.load.ramp_secs = 1;
    plan.load.rps_limit = 300;
    plan.load.think_ms = 0;
    // CSV values are actually consumed by checkout; no example-only stub metrics.
    std::fs::write(
        dir.path().join("products.csv"),
        "product_id\nproduct-1\nproduct-2\n",
    )
    .unwrap();
    plan.csv = CsvSource {
        path: "products.csv".into(),
        reuse_rows: true,
    };
    let db = dir.path().join("history.sqlite3");
    let mut handle = engine::start(&rt, plan.clone(), dir.path(), db.clone()).unwrap();
    let s = wait(&rt, &mut handle);
    assert_eq!(s.state, RunState::Completed);
    assert!(s.counts.completed > 100);
    assert_eq!(s.counts.failed, 0);
    assert_eq!(s.counts.started, s.counts.completed + s.counts.cancelled);
    assert_eq!(s.counts.in_flight(), 0);
    assert_eq!(s.active_users, 0);
    assert!(
        s.series.iter().any(|p| p.users == 100),
        "all 100 users must be active after ramp-up"
    );
    assert_eq!(s.counts.completed, server.count.load(Ordering::Relaxed));
    assert_eq!(s.counts.completed, s.latency.samples);
    // The distribution widget's bars hold every whole-run response.
    assert_eq!(
        s.latency_distribution.iter().map(|b| b.count).sum::<u64>(),
        s.latency.samples
    );
    assert_eq!(s.endpoints.len(), 3);
    assert!(s.endpoints.iter().all(|e| e.counts.completed > 0));
    assert!(
        s.counts.started <= 3 * 300 + 1,
        "global limit must bound starts"
    );
    let history = History::open(&db).unwrap();
    assert_eq!(history.list().unwrap().len(), 1);
    let (saved, config) = history.read(&s.run_id).unwrap();
    assert_eq!(saved.counts.completed, s.counts.completed);
    let points = history.points(&s.run_id).unwrap();
    assert_eq!(
        points.iter().map(|p| p.completed).sum::<u64>(),
        s.counts.completed
    );
    for format in ["HTML", "JSON", "CSV"] {
        let path = dir.path().join(format!("report.{}", format.to_lowercase()));
        report::export(&path, format, &saved, &config, &points).unwrap();
        assert!(std::fs::metadata(path).unwrap().len() > 100);
    }
    server.stop.cancel();
}

#[test]
fn pacing_is_global_across_users() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.requests[0].url = format!("http://{}/health", server.address);
    plan.load.users = 25;
    plan.load.duration_secs = 2;
    plan.load.rps_limit = 10;
    plan.load.think_ms = 0;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    assert!(
        (10..=20).contains(&s.counts.started),
        "starts {}",
        s.counts.started
    );
    assert_eq!(s.counts.failed, 0);
    server.stop.cancel();
}

#[test]
fn pacing_reaches_limits_above_timer_resolution() {
    // Spacing of 2 ms is below what millisecond timer rounding tolerates when the
    // gate drifts; the schedule must still deliver the configured rate, no more.
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.requests[0].url = format!("http://{}/health", server.address);
    plan.load.users = 100;
    plan.load.duration_secs = 2;
    plan.load.rps_limit = 500;
    plan.load.think_ms = 0;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    assert!(
        (900..=1005).contains(&s.counts.started),
        "starts {}",
        s.counts.started
    );
    assert_eq!(s.counts.failed, 0);
    server.stop.cancel();
}

#[test]
fn five_methods_headers_query_json_and_basic_auth_reach_the_server() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.load.users = 2;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 100;
    plan.load.think_ms = 0;
    plan.variables = vec![Field {
        name: "value".into(),
        value: "hello world".into(),
        secret: false,
    }];
    plan.requests=["GET","POST","PUT","PATCH","DELETE"].iter().enumerate().map(|(i,method)|{
        let expected=serde_json::json!({"method":method,"target":"/echo?q=hello+world","content_type":"application/json","authorization":"Basic dXNlcjpwYXNz","custom":"hello world","body":"{\"message\":\"hello world\"}"});
        Request{id:i as u64,name:method.to_string(),method:method.to_string(),url:format!("http://{}/echo",server.address),query:vec![Field{name:"q".into(),value:"{{value}}".into(),secret:false}],headers:vec![Field{name:"X-Custom".into(),value:"{{value}}".into(),secret:false}],body:Body{text:r#"{"message":"{{value}}"}"#.into(),json:true,persist:false},auth:Auth{kind:AuthKind::Basic,username:"user".into(),password:"pass".into(),bearer:String::new()},assertions:Assertions{json_pointer:"/request".into(),json_expected:expected.to_string(),..Assertions::default()},..Request::default()}
    }).collect();
    let mut handle = engine::start(&rt, plan, dir.path(), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    assert_eq!(s.counts.failed, 0);
    assert!(s.endpoints.iter().all(|e| e.counts.successful > 0));
    server.stop.cancel();
}

#[test]
fn redirect_and_cookie_jar_are_disabled() {
    let redirected = run_endpoint("/redirect", |p| p.requests[0].assertions.status = Some(302));
    assert!(redirected.statuses.contains_key(&302));
    assert!(!redirected.statuses.contains_key(&200));
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.load.users = 1;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 10;
    plan.requests = vec![
        Request {
            url: format!("http://{}/cookie", server.address),
            ..Request::default()
        },
        Request {
            id: 2,
            url: format!("http://{}/no-cookie", server.address),
            assertions: Assertions {
                json_pointer: "/cookie_absent".into(),
                json_expected: "true".into(),
                ..Assertions::default()
            },
            ..Request::default()
        },
    ];
    let mut handle = engine::start(&rt, plan, dir.path(), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    assert_eq!(s.counts.failed, 0);
    assert!(s.endpoints[1].counts.successful > 0);
    server.stop.cancel();
}

#[test]
fn csv_rows_and_extracted_tokens_are_isolated_per_virtual_user() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let rows = format!(
        "user\n{}",
        (0..25).map(|i| format!("{i}\n")).collect::<String>()
    );
    std::fs::write(dir.path().join("users.csv"), rows).unwrap();
    let mut plan = TestPlan::default();
    plan.load.users = 25;
    plan.load.duration_secs = 2;
    plan.load.rps_limit = 200;
    plan.load.think_ms = 0;
    plan.csv = CsvSource {
        path: "users.csv".into(),
        reuse_rows: false,
    };
    plan.requests = vec![
        Request {
            id: 1,
            method: "POST".into(),
            url: format!("http://{}/identity-login", server.address),
            body: Body {
                text: r#"{"user":"{{user}}"}"#.into(),
                json: true,
                persist: false,
            },
            extractors: vec![Extractor {
                variable: "access_token".into(),
                pointer: "/access_token".into(),
            }],
            ..Request::default()
        },
        Request {
            id: 2,
            url: format!("http://{}/identity-check", server.address),
            query: vec![Field {
                name: "user".into(),
                value: "{{user}}".into(),
                secret: false,
            }],
            auth: Auth {
                kind: AuthKind::Bearer,
                bearer: "{{access_token}}".into(),
                ..Auth::default()
            },
            ..Request::default()
        },
    ];
    let mut handle = engine::start(&rt, plan, dir.path(), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    assert_eq!(s.counts.failed, 0);
    assert!(s.endpoints[1].counts.successful >= 25);
    server.stop.cancel();
}

fn run_endpoint(path: &str, configure: impl FnOnce(&mut TestPlan)) -> Snapshot {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.load.users = 1;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 3;
    plan.requests[0].url = format!("http://{}{path}", server.address);
    configure(&mut plan);
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    s
}

#[test]
fn http_errors_expected_status_and_bad_json_are_distinct() {
    for path in ["/products", "/status/429", "/status/500"] {
        let s = run_endpoint(path, |_| {});
        assert!(s.counts.failed > 0);
        assert_eq!(s.counts.failed, s.counts.http_errors);
        assert_eq!(s.counts.completed, s.latency.samples);
    }
    let expected = run_endpoint("/status/429", |p| {
        p.requests[0].assertions.status = Some(429)
    });
    assert_eq!(expected.counts.failed, 0);
    assert!(expected.counts.http_errors > 0);
    let malformed = run_endpoint("/malformed", |p| {
        p.requests[0].extractors.push(Extractor {
            variable: "test".into(),
            pointer: "/field".into(),
        })
    });
    assert!(malformed.counts.assertion_errors > 0);
    assert_eq!(malformed.counts.http_errors, 0);
}

#[test]
fn latency_includes_body_read_and_transport_failures_do_not_enter_histogram() {
    let s = run_endpoint("/body-delay", |_| {});
    assert!(s.latency.p50 >= 90.0);
    assert_eq!(s.counts.failed, 0);
    let timeout = run_endpoint("/slow", |p| p.load.timeout_ms = 25);
    assert!(timeout.counts.timeouts > 0);
    assert_eq!(timeout.latency.samples, 0);
    let disconnected = run_endpoint("/disconnect", |_| {});
    assert!(disconnected.counts.network_errors > 0);
    assert_eq!(disconnected.latency.samples, 0);
    let large = run_endpoint("/large", |p| p.load.body_limit_bytes = 1024);
    assert!(large.counts.body_errors > 0);
    assert_eq!(large.latency.samples, 0);
}

#[test]
fn stop_cancels_waiters_then_drains_with_bounded_grace() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.load.users = 100;
    plan.load.duration_secs = 30;
    plan.load.rps_limit = 10;
    plan.load.timeout_ms = 30_000;
    plan.requests[0].url = format!("http://{}/slow", server.address);
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    rt.block_on(async {
        tokio::time::sleep(Duration::from_millis(350)).await;
    });
    let before = Instant::now();
    handle.stop.cancel();
    let s = wait(&rt, &mut handle);
    assert_eq!(s.state, RunState::Cancelled);
    assert!(before.elapsed() < Duration::from_secs(7));
    assert!(s.counts.cancelled > 0);
    assert_eq!(s.counts.completed, 0);
    assert_eq!(s.counts.in_flight(), 0);
    assert_eq!(s.active_users, 0);
    let count = server.count.load(Ordering::Relaxed);
    assert!((1..=5).contains(&count));
    server.stop.cancel();
}

#[test]
fn response_bytes_are_counted_per_endpoint_and_interval() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db");
    let mut plan = TestPlan::default();
    plan.load.users = 2;
    plan.load.duration_secs = 2;
    plan.load.rps_limit = 20;
    plan.requests[0].url = format!("http://{}/health", server.address);
    plan.requests.push(Request {
        id: 2,
        url: format!("http://{}/large", server.address),
        ..Request::default()
    });
    let mut handle = engine::start(&rt, plan, Path::new("."), db.clone()).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    assert_eq!(s.counts.failed, 0);
    let small = &s.endpoints[0].counts;
    let large = &s.endpoints[1].counts;
    assert!(small.completed > 0 && large.completed > 0);
    // Fixture bodies: `{"ok":true}` and 16 KiB of `x`.
    assert_eq!(small.bytes, small.completed * 11);
    assert_eq!(large.bytes, large.completed * 16_384);
    assert_eq!(s.counts.bytes, small.bytes + large.bytes);
    let stored: u64 = History::open(&db)
        .unwrap()
        .points(&s.run_id)
        .unwrap()
        .iter()
        .map(|p| p.bytes)
        .sum();
    assert_eq!(stored, s.counts.bytes);
}

#[test]
fn slo_verdict_is_final_persisted_and_exported() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db");
    let run = |path: &str| {
        let mut plan = TestPlan::default();
        plan.load.users = 2;
        plan.load.duration_secs = 1;
        plan.load.rps_limit = 20;
        plan.requests[0].url = format!("http://{}{path}", server.address);
        plan.thresholds = vec![
            Threshold {
                metric: SloMetric::ErrorRate,
                value: 1.0,
                request_id: None,
            },
            Threshold {
                metric: SloMetric::P95,
                value: 5_000.0,
                request_id: Some(1),
            },
        ];
        let mut handle = engine::start(&rt, plan.clone(), Path::new("."), db.clone()).unwrap();
        (wait(&rt, &mut handle), plan)
    };
    let (ok, _) = run("/health");
    let ok_slo = ok.slo.as_ref().unwrap();
    assert_eq!((ok_slo.verdict, ok_slo.partial), (Verdict::Passed, false));
    let (bad, plan) = run("/status/500");
    let bad_slo = bad.slo.as_ref().unwrap();
    assert_eq!(bad_slo.verdict, Verdict::Failed);
    assert_eq!(bad_slo.results[0].status, ThresholdStatus::Fail);
    assert_eq!(bad_slo.results[0].actual, Some(100.0));
    server.stop.cancel();

    let (stored, config) = History::open(&db).unwrap().read(&bad.run_id).unwrap();
    assert_eq!(stored.slo.unwrap().verdict, Verdict::Failed);
    assert_eq!(config.thresholds, plan.thresholds);
    let html = dir.path().join("report.html");
    report::export(&html, "HTML", &bad, &plan, &[]).unwrap();
    let html = std::fs::read_to_string(html).unwrap();
    assert!(html.contains("<h2>SLO: Failed</h2>"));
    assert!(html.contains("Error rate ≤ 1 %"));
    let csv = dir.path().join("report.csv");
    report::export(&csv, "CSV", &bad, &plan, &[]).unwrap();
    let csv = std::fs::read_to_string(csv).unwrap();
    assert!(csv.lines().next().unwrap().ends_with(",slo_verdict"));
    assert!(csv.lines().nth(1).unwrap().ends_with(",Failed"));
}

#[test]
fn saved_runs_can_be_compared() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db");
    let mut plan = TestPlan::default();
    plan.load.users = 2;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 20;
    plan.requests[0].url = format!("http://{}/health", server.address);
    let mut ids = vec![];
    for users in [2, 4] {
        plan.load.users = users;
        let mut handle = engine::start(&rt, plan.clone(), Path::new("."), db.clone()).unwrap();
        ids.push(wait(&rt, &mut handle).run_id);
    }
    server.stop.cancel();
    let history = History::open(&db).unwrap();
    let (base, base_plan) = history.read(&ids[0]).unwrap();
    let (candidate, candidate_plan) = history.read(&ids[1]).unwrap();
    let c = rustload::compare::compare(&base, &base_plan, &candidate, &candidate_plan, 2.0);
    assert_eq!(c.endpoints.len(), 1);
    assert!(c.only_base.is_empty() && c.only_candidate.is_empty());
    assert_eq!(c.warnings, ["Virtual users differs: 2 vs 4"]);
    let throughput = &c.overall[0];
    assert_eq!(
        throughput.metric,
        rustload::compare::CompareMetric::Throughput
    );
    assert!(throughput.base.unwrap() > 0.0 && throughput.candidate.unwrap() > 0.0);
}

#[test]
fn imported_curl_requests_reach_the_server_as_written() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let base = format!("http://{}/echo", server.address);
    let text = format!(
        "curl '{base}?q=1' -H 'X-Custom: yes' -H 'accept-encoding: gzip' ;\n\
         curl '{base}' -X PUT -H 'content-type: application/json' --data-raw '{{\"x\":1}}' ;\n\
         curl '{base}' --data-raw 'plain'"
    );
    let mut imported = rustload::import::from_curl(&text, 1).unwrap();
    let expect = |pointer: &str, value: &str| Assertions {
        json_pointer: pointer.into(),
        json_expected: value.into(),
        ..Assertions::default()
    };
    imported.requests[0].assertions = expect("/request/custom", "\"yes\"");
    imported.requests[1].assertions = expect("/request/content_type", "\"application/json\"");
    imported.requests[2].assertions = expect("/request/body", "\"plain\"");
    let mut plan = TestPlan {
        requests: imported.requests,
        ..TestPlan::default()
    };
    plan.load.users = 1;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 30;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    assert_eq!(s.counts.failed, 0, "{:?}", s.logs);
    assert!(s.endpoints.iter().all(|e| e.counts.successful > 0));
}

fn warmup_run(warmup: Warmup, duration: u64) -> Snapshot {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.requests[0].url = format!("http://{}/health", server.address);
    plan.load.users = 5;
    plan.load.duration_secs = duration;
    plan.load.rps_limit = 40;
    plan.load.think_ms = 0;
    plan.load.warmup = warmup;
    plan.thresholds = vec![Threshold {
        metric: SloMetric::Throughput,
        value: 1.0,
        request_id: None,
    }];
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    s
}

#[test]
fn duration_warmup_is_excluded_from_every_result_metric() {
    let s = warmup_run(Warmup::Duration { secs: 1 }, 3);
    let (w, m) = (&s.warmup_counts, &s.counts);
    // ~40 starts in the first second are warmup; ~80 in the next two are measured.
    assert!(
        (30..=50).contains(&w.started),
        "warmup starts {}",
        w.started
    );
    assert!(
        (65..=95).contains(&m.started),
        "measured starts {}",
        m.started
    );
    // Each phase keeps started = completed + cancelled (+ in flight, none at the end).
    assert_eq!(w.started, w.completed + w.cancelled);
    assert_eq!(m.started, m.completed + m.cancelled);
    // Histograms, statuses and endpoints only see measured requests.
    assert_eq!(s.latency.samples, m.completed);
    assert_eq!(s.statuses.values().sum::<u64>(), m.completed);
    assert_eq!(s.endpoints[0].counts.started, m.started);
    assert_eq!(s.counts.bytes, m.completed * 11);
    assert_eq!(w.bytes, w.completed * 11);
    let charted: u64 = s.series.iter().map(|p| p.warmup_completed).sum();
    assert_eq!(charted, w.completed);
    assert_eq!((s.measured_from, s.warming_up), (1.0, false));
    // Throughput uses the measured window, not the whole run.
    let throughput = s.slo.unwrap().results[0].actual.unwrap();
    assert!(
        (30.0..=45.0).contains(&throughput),
        "throughput {throughput}"
    );
}

#[test]
fn request_count_warmup_excludes_exactly_the_first_starts() {
    let s = warmup_run(Warmup::Requests { count: 15 }, 2);
    assert_eq!(s.warmup_counts.started, 15);
    assert!(s.counts.started > 0);
    assert_eq!(s.latency.samples, s.counts.completed);
    assert!(s.measured_from > 0.0 && !s.warming_up);
}

#[test]
fn unfinished_warmup_leaves_no_measured_data() {
    let s = warmup_run(Warmup::Requests { count: 1_000_000 }, 1);
    assert!(s.warming_up);
    assert!(s.warmup_counts.completed > 0);
    assert_eq!((s.counts.started, s.latency.samples), (0, 0));
    let slo = s.slo.unwrap();
    assert_eq!(slo.verdict, Verdict::Inconclusive);
}

#[test]
fn stage_profile_ramps_users_up_and_down() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.requests[0].url = format!("http://{}/health", server.address);
    plan.load.profile = Profile::Stages;
    plan.load.stages = vec![
        Stage {
            duration_secs: 1,
            target_users: 4,
        },
        Stage {
            duration_secs: 1,
            target_users: 4,
        },
        Stage {
            duration_secs: 1,
            target_users: 1,
        },
    ];
    plan.load.rps_limit = 200;
    plan.load.think_ms = 0;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    assert_eq!(s.target_users, 4);
    assert_eq!(s.counts.failed, 0);
    assert!(
        s.elapsed < 3.9,
        "runs for the stage total, elapsed {}",
        s.elapsed
    );
    let users: Vec<_> = s.series.iter().map(|p| (p.elapsed, p.users)).collect();
    let at = |t: f64| {
        users
            .iter()
            .find(|(e, _)| *e >= t)
            .map(|(_, u)| *u)
            .unwrap()
    };
    assert_eq!(at(1.5), 4, "{users:?}");
    assert!(at(2.9) <= 2, "ramped down: {users:?}");
    assert!(users.iter().all(|(_, u)| *u <= 4));
}

fn arrival_run(path: &str, rate: f64, max_vus: u32) -> (Snapshot, u64) {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.requests[0].url = format!("http://{}{path}", server.address);
    plan.load.profile = Profile::ArrivalRate;
    plan.load.arrival_rate = rate;
    plan.load.users = max_vus;
    plan.load.duration_secs = 2;
    // Neither applies to the open model: starts follow the arrival schedule.
    plan.load.rps_limit = 1;
    plan.load.think_ms = 5_000;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    (s, server.count.load(Ordering::Relaxed))
}

#[test]
fn arrival_rate_starts_iterations_on_schedule() {
    let (s, served) = arrival_run("/health", 50.0, 10);
    assert_eq!(s.state, RunState::Completed);
    assert_eq!(s.arrival_rate, Some(50.0));
    assert_eq!(s.target_users, 10);
    // 2 s at 50/s: one start every 20 ms from t=0, ignoring RPS cap and think time.
    assert!(
        (95..=100).contains(&s.counts.started),
        "started {}",
        s.counts.started
    );
    assert_eq!(s.counts.dropped_iterations, 0);
    assert_eq!(s.counts.failed, 0);
    assert_eq!(s.counts.started, s.counts.completed + s.counts.cancelled);
    assert_eq!(s.counts.completed, served);
    assert_eq!(s.active_users, 0);
}

#[test]
fn arrival_rate_drops_iterations_when_every_vu_is_busy() {
    // Each iteration takes ~100 ms, so 2 VUs serve ~20/s of the 50/s offered.
    let (s, _) = arrival_run("/body-delay", 50.0, 2);
    assert_eq!(s.state, RunState::Completed);
    assert!(
        s.counts.dropped_iterations >= 40,
        "dropped {}",
        s.counts.dropped_iterations
    );
    // Every scheduled arrival either started or was dropped.
    let offered = s.counts.started + s.counts.dropped_iterations;
    assert!((95..=100).contains(&offered), "offered {offered}");
    assert_eq!(s.counts.failed, 0);
    assert!(s.series.iter().all(|p| p.users <= 2));
    // Dropped iterations are not requests, so they never count as errors.
    assert_eq!(s.counts.error_rate(), 0.0);
}

/// Arrival rate ramping 0 to 60 it/s over 3 s: 90 scheduled iterations.
fn rate_ramp_run(path: &str, max_vus: u32) -> Snapshot {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plan = TestPlan::default();
    plan.requests[0].url = format!("http://{}{path}", server.address);
    plan.load.profile = Profile::ArrivalRate;
    plan.load.arrival_rate = 0.0;
    plan.load.rate_stages = vec![RateStage {
        duration_secs: 3,
        target_rate: 60.0,
    }];
    plan.load.users = max_vus;
    plan.load.duration_secs = 30; // derived from the stages instead
    plan.load.rps_limit = 1;
    plan.load.think_ms = 5_000;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    s
}

#[test]
fn arrival_rate_stages_ramp_the_start_rate() {
    let s = rate_ramp_run("/health", 20);
    assert_eq!(s.state, RunState::Completed);
    assert!(s.elapsed < 4.5, "runs for the stage total, {}", s.elapsed);
    assert_eq!(s.target_rate, [(0.0, 0.0), (3.0, 60.0)]);
    assert_eq!(s.arrival_rate, Some(0.0));
    // The integral of the ramp: iteration k starts at sqrt(k / 10) s.
    assert!(
        (85..=90).contains(&s.counts.started),
        "started {}",
        s.counts.started
    );
    assert_eq!(s.counts.dropped_iterations, 0);
    assert_eq!(s.counts.failed, 0);
    assert_eq!(s.counts.started, s.counts.completed + s.counts.cancelled);
    // Starts speed up: about 10 in the first second, 50 in the last.
    let rate = |p: &Point| p.completed as f64 / p.interval_secs.max(0.001);
    let first = s.series.front().expect("a first interval");
    assert!(first.elapsed < 2.0, "first point at {}", first.elapsed);
    let peak = s.series.iter().map(rate).fold(0.0, f64::max);
    assert!(rate(first) < 25.0, "first interval {}/s", rate(first));
    assert!(peak > 35.0, "peak interval {peak}/s");
}

#[test]
fn arrival_rate_stages_still_drop_iterations_without_idle_vus() {
    // ~100 ms per iteration: one VU serves ~10/s of the ramp to 60/s.
    let s = rate_ramp_run("/body-delay", 1);
    assert_eq!(s.state, RunState::Completed);
    let offered = s.counts.started + s.counts.dropped_iterations;
    assert!((85..=90).contains(&offered), "offered {offered}");
    assert!(
        s.counts.dropped_iterations >= 40,
        "dropped {}",
        s.counts.dropped_iterations
    );
    assert_eq!(s.counts.error_rate(), 0.0);
    assert!(s.series.iter().all(|p| p.users <= 1));
}

#[test]
fn failures_are_grouped_by_sanitized_reason() {
    let reasons = |s: &Snapshot| -> Vec<String> {
        // Every measured failure is in exactly one group.
        assert_eq!(
            s.errors.iter().map(|g| g.count).sum::<u64>() + s.errors_overflow,
            s.counts.failed
        );
        for g in &s.errors {
            assert!(
                !g.reason.contains("127.0.0.1") && !g.reason.contains("http"),
                "no URL or host in reasons: {}",
                g.reason
            );
            assert!(g.first_at <= g.last_at);
        }
        s.errors.iter().map(|g| g.reason.clone()).collect()
    };
    let status = run_endpoint("/status/500", |_| {});
    assert_eq!(reasons(&status), ["HTTP 500, expected 2xx"]);
    assert_eq!(status.errors[0].request_id, status.endpoints[0].id);
    let expected = run_endpoint("/health", |p| p.requests[0].assertions.status = Some(201));
    assert_eq!(reasons(&expected), ["HTTP 200, expected 201"]);
    let json = run_endpoint("/health", |p| {
        p.requests[0].assertions.json_pointer = "/status".into();
        p.requests[0].assertions.json_expected = "\"down\"".into();
    });
    assert_eq!(reasons(&json), ["JSON at /status did not match"]);
    let extraction = run_endpoint("/malformed", |p| {
        p.requests[0].extractors.push(Extractor {
            variable: "token".into(),
            pointer: "/field".into(),
        })
    });
    assert_eq!(reasons(&extraction), ["Extraction found nothing at /field"]);
    let timeout = run_endpoint("/slow", |p| p.load.timeout_ms = 25);
    assert_eq!(reasons(&timeout), ["Timeout"]);
    let large = run_endpoint("/large", |p| p.load.body_limit_bytes = 1024);
    assert_eq!(reasons(&large), ["ResponseTooLarge"]);
    let disconnected = run_endpoint("/disconnect", |_| {});
    // The server closes without answering: no HTTP response, no I/O error kind.
    assert_eq!(reasons(&disconnected), ["Request failed before a response"]);
    // A port with no listener refuses the connection.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let refused = run_endpoint("/health", |p| {
        p.requests[0].url = format!("http://127.0.0.1:{port}/health")
    });
    assert_eq!(reasons(&refused), ["Connection refused"]);
    // Logs carry the same reason instead of a generic label.
    assert!(
        refused
            .logs
            .iter()
            .any(|l| l.level == "ERROR" && l.message.ends_with("Connection refused"))
    );
}

#[test]
fn imported_openapi_requests_reach_the_server() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let doc = r##"{
      "openapi": "3.0.3",
      "servers": [{"url": "https://api.invalid"}],
      "paths": {"/{endpoint}": {
        "parameters": [{"name": "endpoint", "in": "path", "required": true, "example": "echo"}],
        "get": {"operationId": "echoGet", "parameters": [
          {"name": "q", "in": "query", "required": true, "schema": {"default": "1"}},
          {"name": "X-Custom", "in": "header", "required": true, "example": "yes"}
        ]},
        "post": {"operationId": "echoPost", "requestBody": {"content": {"application/json": {
          "schema": {"$ref": "#/components/schemas/Item"}}}}}
      }},
      "components": {"schemas": {"Item": {"properties": {"x": {"example": 1}}}}}
    }"##;
    let base = format!("http://{}", server.address);
    let mut imported = rustload::import::from_openapi(doc.as_bytes(), 1, &base).unwrap();
    let expect = |pointer: &str, value: &str| Assertions {
        json_pointer: pointer.into(),
        json_expected: value.into(),
        ..Assertions::default()
    };
    imported.requests[0].assertions = expect("/request/target", "\"/echo?q=1\"");
    imported.requests[1].assertions = expect("/request/content_type", "\"application/json\"");
    let mut plan = TestPlan {
        requests: imported.requests,
        variables: imported.variables,
        ..TestPlan::default()
    };
    plan.load.users = 1;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 30;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    assert_eq!(s.counts.failed, 0, "{:?} {:?}", s.errors, s.logs);
    assert!(s.endpoints.iter().all(|e| e.counts.successful > 0));
}

#[test]
fn selected_openapi_operations_with_api_keys_reach_the_server() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let doc = r##"{
      "openapi": "3.0.3",
      "servers": [{"url": "https://api.invalid"}],
      "components": {
        "securitySchemes": {
          "header": {"type": "apiKey", "in": "header", "name": "X-Custom"},
          "query": {"type": "apiKey", "in": "query", "name": "key"}
        },
        "schemas": {"Order": {"allOf": [
          {"type": "object", "required": ["id"], "properties": {"id": {"type": "string", "format": "uuid"}}},
          {"type": "object", "required": ["qty"], "properties": {"qty": {"type": "integer", "minimum": 1}}}
        ]}}
      },
      "security": [{"header": [], "query": []}],
      "paths": {
        "/echo": {
          "get": {"operationId": "echoGet", "tags": ["echo"]},
          "post": {"operationId": "echoPost", "tags": ["echo"], "requestBody": {"content": {
            "application/json": {"schema": {"$ref": "#/components/schemas/Order"}}}}}
        },
        "/missing": {"get": {"operationId": "notThere", "tags": ["other"]}}
      }
    }"##;
    let base = format!("http://{}", server.address);
    let spec = rustload::import::parse_openapi(doc.as_bytes(), &base).unwrap();
    assert_eq!(spec.operations.len(), 3);
    let selected = spec
        .operations
        .iter()
        .filter(|o| o.tags.iter().any(|t| t == "echo"))
        .map(|o| o.key.clone())
        .collect();
    let mut imported = spec.build(&selected, 1);
    assert_eq!(imported.requests.len(), 2, "/missing is not selected");
    let expect = |pointer: &str, value: &str| Assertions {
        json_pointer: pointer.into(),
        json_expected: value.into(),
        ..Assertions::default()
    };
    // Header and query keys reach the server; the placeholder body is valid JSON.
    imported.requests[0].assertions = expect("/request/target", "\"/echo?key=query-key\"");
    imported.requests[1].assertions = expect("/request/custom", "\"header-key\"");
    let body: serde_json::Value = serde_json::from_str(&imported.requests[1].body.text).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"id": "00000000-0000-0000-0000-000000000000", "qty": 1})
    );
    let mut plan = TestPlan {
        requests: imported.requests,
        variables: imported.variables,
        ..TestPlan::default()
    };
    for v in &mut plan.variables {
        v.value = if v.name == "api_key" {
            "header-key"
        } else {
            "query-key"
        }
        .into();
    }
    plan.load.users = 1;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 30;
    // The saved form keeps the references, not the keys.
    let saved = serde_json::to_string(&plan).unwrap();
    assert!(saved.contains("{{api_key}}") && saved.contains("{{api_key_2}}"));
    let restored: TestPlan = serde_json::from_str(&saved).unwrap();
    assert!(
        restored
            .variables
            .iter()
            .all(|v| v.secret && v.value.is_empty())
    );
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    assert_eq!(s.counts.failed, 0, "{:?} {:?}", s.errors, s.logs);
    assert_eq!(s.endpoints.len(), 2);
    assert!(s.endpoints.iter().all(|e| e.counts.successful > 0));
}

#[test]
fn imported_multi_file_openapi_spec_runs_with_a_chosen_server() {
    let rt = runtime();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let port = server.address.port();
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "paths/echo.json",
        r##"{
          "get": {"operationId": "echoGet"},
          "post": {"operationId": "echoPost",
            "parameters": [{"$ref": "../components.json#/parameters/Custom"}],
            "requestBody": {"content": {"application/json": {
              "schema": {"$ref": "../schemas/order.json"}}}}}
        }"##,
    );
    write(
        "components.json",
        r##"{"parameters": {"Custom": {"name": "X-Custom", "in": "header", "required": true,
          "example": "{{method}}"}}}"##,
    );
    write(
        "schemas/order.json",
        r##"{"type": "object", "required": ["qty", "item"], "properties": {
          "qty": {"type": "integer", "minimum": 2}, "item": {"$ref": "item.json"}}}"##,
    );
    write(
        "schemas/item.json",
        r##"{"type": "string", "example": "book"}"##,
    );
    let doc = format!(
        r##"{{
          "openapi": "3.0.3",
          "servers": [
            {{"url": "https://api.invalid"}},
            {{"url": "http://127.0.0.1:{{port}}", "variables": {{"port": {{"default": "1", "enum": ["1", "{port}"]}}}}}}
          ],
          "paths": {{"/echo": {{"$ref": "paths/echo.json"}}, "/health": {{"get": {{}}}}}}
        }}"##
    );
    let mut spec = rustload::import::load_openapi(doc.as_bytes(), Some(dir.path())).unwrap();
    assert!(spec.warnings.is_empty(), "{:?}", spec.warnings);
    assert_eq!(spec.referenced_files, 4);
    let mut choice = rustload::import::ServerChoice::defaults(&spec, 1);
    choice.values[0] = port.to_string();
    spec.select_server(&choice, "").unwrap();
    let selected = ["GET /echo", "POST /echo"]
        .iter()
        .map(|k| k.to_string())
        .collect();
    let mut imported = spec.build(&selected, 1);
    assert_eq!(imported.requests.len(), 2);
    assert_eq!(
        imported.requests[1].url,
        format!("http://127.0.0.1:{port}/echo")
    );
    let body: serde_json::Value = serde_json::from_str(&imported.requests[1].body.text).unwrap();
    assert_eq!(body, serde_json::json!({"qty": 2, "item": "book"}));
    // The first step extracts a value the second sends as its header.
    imported.requests[0].extractors.push(Extractor {
        variable: "method".into(),
        pointer: "/request/method".into(),
    });
    imported.requests[1].assertions = Assertions {
        json_pointer: "/request/custom".into(),
        json_expected: "\"GET\"".into(),
        ..Assertions::default()
    };
    let mut plan = TestPlan {
        requests: imported.requests,
        variables: imported.variables,
        ..TestPlan::default()
    };
    plan.load.users = 1;
    plan.load.duration_secs = 1;
    plan.load.rps_limit = 30;
    let mut handle = engine::start(&rt, plan, Path::new("."), dir.path().join("db")).unwrap();
    let s = wait(&rt, &mut handle);
    server.stop.cancel();
    assert_eq!(s.counts.failed, 0, "{:?} {:?}", s.errors, s.logs);
    assert_eq!(s.endpoints.len(), 2);
    assert!(s.endpoints.iter().all(|e| e.counts.successful > 0));
}

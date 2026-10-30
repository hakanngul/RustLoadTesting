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

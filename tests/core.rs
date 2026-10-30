use rustload::{core::*, metrics::*, report};

#[test]
fn plan_round_trip_and_secret_omission() {
    let mut p = TestPlan::default();
    p.variables.push(Field {
        name: "private_value".into(),
        value: "dont-store-this".into(),
        secret: true,
    });
    p.requests[0].auth = Auth {
        kind: AuthKind::Basic,
        username: "user".into(),
        password: "hidden-password".into(),
        ..Auth::default()
    };
    p.requests[0].headers.push(Field {
        name: "Authorization".into(),
        value: "Bearer confidential".into(),
        secret: false,
    });
    p.requests[0].body.text = "session-only-body".into();
    let json = serde_json::to_string(&p).unwrap();
    for secret in [
        "dont-store-this",
        "hidden-password",
        "Bearer confidential",
        "session-only-body",
    ] {
        assert!(!json.contains(secret));
    }
    let restored: TestPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.requests[0].id, p.requests[0].id);
    assert_eq!(restored.requests[0].method, "GET");
    assert!(restored.variables[0].secret);
    assert!(restored.validate(std::path::Path::new(".")).is_err());
}

#[test]
fn templates_are_scoped_and_missing_values_fail() {
    let vars = Variables::from([("name".into(), "Alice".into())]);
    assert_eq!(expand("hello {{ name }}", &vars).unwrap(), "hello Alice");
    assert!(expand("{{missing}}", &vars).is_err());
    assert!(expand("{{name", &vars).is_err());
    assert!(Variables::new().is_empty());
}

#[test]
fn saved_scenarios_preserve_auth_variable_references_but_not_credentials() {
    let plan = rustload::fixture::demo_plan("127.0.0.1:8080".parse().unwrap());
    let restored: TestPlan = serde_json::from_str(&serde_json::to_string(&plan).unwrap()).unwrap();
    assert_eq!(restored.requests[1].auth.bearer, "{{access_token}}");
    assert_eq!(restored.requests[2].body.text, plan.requests[2].body.text);
    assert!(restored.validate(std::path::Path::new(".")).is_ok());
    let auth = Auth {
        kind: AuthKind::Bearer,
        bearer: "literal-secret{{token}}".into(),
        ..Auth::default()
    };
    assert!(
        !serde_json::to_string(&auth)
            .unwrap()
            .contains("literal-secret")
    );
}

#[test]
fn rolling_buffers_and_interval_rates_remain_bounded() {
    let mut m = Accumulator::default();
    for i in 1..=2000 {
        m.counts.completed += 10;
        m.counts.successful += 10;
        m.point(i as f64);
    }
    assert_eq!(m.series.len(), SERIES_LIMIT);
    assert_eq!(m.series.front().unwrap().interval_secs, 1.0);
    assert!((m.snapshot(2000.0).rps - 10.0).abs() < 0.001);
    for i in 0..6000 {
        m.log(i as f64, "ERROR", "x".repeat(3000));
    }
    assert_eq!(m.logs.len(), LOG_LIMIT);
    assert_eq!(m.logs.front().unwrap().message.len(), 2048);
}

#[test]
fn validation_rejects_unsafe_or_incoherent_plans() {
    for url in [
        "file:///tmp/example",
        "ftp://example.com",
        "https://user:pass@example.com",
        "https://example.com/?token=secret",
    ] {
        assert!(validate_url(url).is_err());
    }
    assert!(validate_url("http://127.0.0.1:8080/health").is_ok());
    let mut p = TestPlan::default();
    p.load.profile = Profile::RampUp;
    p.load.ramp_secs = 31;
    assert!(p.validate(std::path::Path::new(".")).is_err());
    p.load.ramp_secs = 30;
    p.requests[0].url = "http://{{host}}/".into();
    assert!(p.validate(std::path::Path::new(".")).is_err());
    p.requests[0].url = "http://127.0.0.1/{{path}}".into();
    assert!(p.validate(std::path::Path::new(".")).is_ok());
}

#[test]
fn assertions_use_explicit_status_and_json_pointer() {
    assert!(!evaluate(&Assertions::default(), 500, 1, b"{}"));
    let a = Assertions {
        status: Some(401),
        max_ms: Some(20),
        json_pointer: "/ok".into(),
        json_expected: "true".into(),
    };
    assert!(evaluate(&a, 401, 10, br#"{"ok":true}"#));
    assert!(!evaluate(&a, 401, 21, br#"{"ok":true}"#));
    assert!(!evaluate(&a, 401, 10, b"not json"));
}

#[test]
fn csv_requires_unique_headers_and_enough_rows() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.csv"), "id,name\n1,Alice\n2,Bob\n").unwrap();
    let mut p = TestPlan::default();
    p.load.users = 3;
    p.csv.path = "data.csv".into();
    assert!(p.csv_rows(dir.path()).is_err());
    p.csv.reuse_rows = true;
    let rows = p.csv_rows(dir.path()).unwrap();
    assert_eq!(rows[0]["name"], "Alice");
    assert_eq!(rows[1]["id"], "2");
    std::fs::write(dir.path().join("data.csv"), "id,id\n1,2\n").unwrap();
    assert!(p.csv_rows(dir.path()).is_err());
}

#[test]
fn percentiles_are_merged_samples_not_averages() {
    let mut h = histogram();
    for _ in 0..99 {
        h.record(1_000).unwrap();
    }
    h.record(100_000).unwrap();
    let l = latency(&h);
    assert_eq!(l.samples, 100);
    assert!(l.p95 < 1.1);
    assert!(l.p99 < 1.1);
    assert_eq!(latency(&histogram()).samples, 0);
    let c = Counts {
        started: 12,
        completed: 9,
        successful: 7,
        failed: 2,
        cancelled: 1,
        ..Counts::default()
    };
    assert_eq!(c.in_flight(), 2);
    assert!((c.error_rate() - 22.222).abs() < 0.01);
}

#[test]
fn offline_reports_escape_html_and_csv_formulas() {
    let dir = tempfile::tempdir().unwrap();
    let s = Snapshot {
        name: "<script>alert(1)</script>".into(),
        state: RunState::Cancelled,
        endpoints: vec![Endpoint {
            name: "=1+2".into(),
            ..Endpoint::default()
        }],
        ..Snapshot::default()
    };
    report::export(
        &dir.path().join("report.html"),
        "HTML",
        &s,
        &TestPlan::default(),
        &[],
    )
    .unwrap();
    let html = std::fs::read_to_string(dir.path().join("report.html")).unwrap();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("Cancelled"));
    report::export(
        &dir.path().join("report.csv"),
        "CSV",
        &s,
        &TestPlan::default(),
        &[],
    )
    .unwrap();
    assert!(
        std::fs::read_to_string(dir.path().join("report.csv"))
            .unwrap()
            .contains("'=1+2")
    );
}

#[test]
fn history_written_before_byte_metrics_still_loads() {
    // Rows in the v0.1 shape: no `bytes`, `bytes_rate` or other later fields.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("history.sqlite3");
    let history = rustload::storage::History::open(&db).unwrap();
    history.begin("1", &TestPlan::default()).unwrap();
    drop(history);
    let counts = r#"{"started":3,"completed":3,"successful":2,"failed":1,"cancelled":0,
        "network_errors":0,"timeouts":0,"assertion_errors":0,"http_errors":1,
        "status_errors":1,"body_errors":0,"preparation_errors":0}"#;
    let latency = r#"{"samples":3,"p50":1.5,"p90":2.0,"p95":2.0,"p99":2.0,"mean":1.6}"#;
    let point = format!(
        r#"{{"interval_secs":1.0,"second":1,"elapsed":1.0,"users":1,"completed":3,
        "successful":2,"failed":1,"latency":{latency}}}"#
    );
    let summary = format!(
        r#"{{"run_id":"1","name":"Old run","state":"Completed","elapsed":1.0,
        "active_users":0,"target_users":1,"rps":3.0,"counts":{counts},"latency":{latency},
        "statuses":{{"200":2,"500":1}},"endpoints":[{{"id":1,"name":"HTTP Request",
        "method":"GET","counts":{counts},"latency":{latency}}}],"series":[{point}],
        "logs":[{{"elapsed":0.0,"level":"INFO","message":"Run started"}}],"issue":null}}"#
    );
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute(
            "UPDATE runs SET state='Completed', summary=?1 WHERE id='1'",
            [&summary],
        )
        .unwrap();
    connection
        .execute("INSERT INTO points VALUES ('1', 0, ?1)", [&point])
        .unwrap();
    let history = rustload::storage::History::open(&db).unwrap();
    let (snapshot, _) = history.read("1").unwrap();
    assert_eq!(snapshot.counts.completed, 3);
    assert_eq!(snapshot.counts.bytes, 0);
    assert_eq!(snapshot.bytes_rate, 0.0);
    assert_eq!(snapshot.endpoints[0].counts.successful, 2);
    let points = history.points("1").unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!((points[0].completed, points[0].bytes), (3, 0));
}

#[test]
fn byte_sizes_use_binary_units() {
    assert_eq!(format_bytes(0.0), "0 B");
    assert_eq!(format_bytes(1023.0), "1023 B");
    assert_eq!(format_bytes(1536.0), "1.5 KiB");
    assert_eq!(format_bytes(3.0 * 1024.0 * 1024.0), "3.0 MiB");
}

#[test]
fn slo_thresholds_round_trip_and_older_plans_still_load() {
    let plan = TestPlan {
        thresholds: vec![
            Threshold::default(),
            Threshold {
                metric: SloMetric::ErrorRate,
                value: 1.0,
                request_id: Some(1),
            },
        ],
        ..TestPlan::default()
    };
    let json = serde_json::to_value(&plan).unwrap();
    let back: TestPlan = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back.thresholds, plan.thresholds);
    // Stored run configurations keep the thresholds too.
    assert_eq!(plan.redacted().thresholds, plan.thresholds);
    let mut old = json;
    old.as_object_mut().unwrap().remove("thresholds");
    let old: TestPlan = serde_json::from_value(old).unwrap();
    assert!(old.thresholds.is_empty());
    assert!(old.validate(std::path::Path::new(".")).is_ok());
}

#[test]
fn slo_threshold_validation() {
    let base = std::path::Path::new(".");
    let check = |t: Threshold| {
        let mut plan = TestPlan::default();
        plan.thresholds.push(t);
        plan.validate(base)
    };
    assert!(check(Threshold::default()).is_ok());
    let rate = |value| Threshold {
        metric: SloMetric::ErrorRate,
        value,
        request_id: None,
    };
    assert!(check(rate(0.0)).is_ok());
    assert!(check(rate(100.5)).is_err());
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            check(Threshold {
                value,
                ..Threshold::default()
            })
            .is_err()
        );
    }
    assert!(
        check(Threshold {
            request_id: Some(99),
            ..Threshold::default()
        })
        .unwrap_err()
        .contains("no longer exists")
    );
    let plan = TestPlan {
        thresholds: vec![Threshold::default(); THRESHOLD_LIMIT + 1],
        ..TestPlan::default()
    };
    assert!(plan.validate(base).is_err());
}

fn slo_snapshot() -> Snapshot {
    let latency = Latency {
        samples: 10,
        p50: 10.0,
        p90: 20.0,
        p95: 30.0,
        p99: 40.0,
        mean: 12.0,
    };
    let counts = Counts {
        completed: 100,
        successful: 98,
        failed: 2,
        ..Counts::default()
    };
    Snapshot {
        state: RunState::Completed,
        elapsed: 10.0,
        counts: counts.clone(),
        latency: latency.clone(),
        endpoints: vec![
            Endpoint {
                id: 1,
                name: "Login".into(),
                counts,
                latency,
                ..Endpoint::default()
            },
            Endpoint {
                id: 2,
                name: "Unused".into(),
                ..Endpoint::default()
            },
        ],
        ..Snapshot::default()
    }
}

#[test]
fn slo_evaluation_uses_bounds_scopes_and_whole_run_throughput() {
    let s = slo_snapshot();
    let t = |metric, value, request_id| Threshold {
        metric,
        value,
        request_id,
    };
    assert!(evaluate_thresholds(&s, &[]).is_none());
    let outcome = evaluate_thresholds(
        &s,
        &[
            t(SloMetric::P95, 30.0, None),         // equal to target passes
            t(SloMetric::P99, 39.9, None),         // above an upper bound fails
            t(SloMetric::ErrorRate, 2.0, Some(1)), // 2 % on the Login endpoint
            t(SloMetric::Throughput, 10.0, None),  // 100 completed / 10 s
            t(SloMetric::Throughput, 10.1, None),  // below a lower bound fails
            t(SloMetric::Mean, 50.0, Some(2)),     // endpoint without samples
            t(SloMetric::P50, 50.0, Some(7)),      // request not in the run
        ],
    )
    .unwrap();
    let statuses: Vec<_> = outcome.results.iter().map(|r| r.status).collect();
    use ThresholdStatus::*;
    assert_eq!(statuses, [Pass, Fail, Pass, Pass, Fail, NoData, NoData]);
    assert_eq!(outcome.results[3].actual, Some(10.0));
    assert_eq!(outcome.verdict, Verdict::Failed);
    assert!(!outcome.partial);

    let passing = evaluate_thresholds(&s, &[t(SloMetric::P95, 100.0, None)]).unwrap();
    assert_eq!(passing.verdict, Verdict::Passed);
    let missing = evaluate_thresholds(
        &s,
        &[
            t(SloMetric::P95, 100.0, None),
            t(SloMetric::P95, 100.0, Some(2)),
        ],
    )
    .unwrap();
    assert_eq!(missing.verdict, Verdict::Inconclusive);
    let cancelled = Snapshot {
        state: RunState::Cancelled,
        ..s
    };
    let partial = evaluate_thresholds(&cancelled, &[t(SloMetric::P95, 100.0, None)]).unwrap();
    assert!(partial.partial);
    assert_eq!(partial.summary(), "Passed (partial run)");
}

#[test]
fn warmup_settings_validate_and_older_plans_have_none() {
    let base = std::path::Path::new(".");
    let with = |warmup| {
        let mut plan = TestPlan::default();
        plan.load.duration_secs = 30;
        plan.load.warmup = warmup;
        plan.validate(base)
    };
    assert!(with(Warmup::None).is_ok());
    assert!(with(Warmup::Duration { secs: 29 }).is_ok());
    assert!(with(Warmup::Duration { secs: 30 }).is_err());
    assert!(with(Warmup::Duration { secs: 0 }).is_err());
    assert!(with(Warmup::Requests { count: 1 }).is_ok());
    assert!(with(Warmup::Requests { count: 0 }).is_err());

    let mut plan = TestPlan::default();
    plan.load.warmup = Warmup::Requests { count: 50 };
    let mut json = serde_json::to_value(&plan).unwrap();
    let back: TestPlan = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back.load.warmup, Warmup::Requests { count: 50 });
    json["load"].as_object_mut().unwrap().remove("warmup");
    let old: TestPlan = serde_json::from_value(json).unwrap();
    assert_eq!(old.load.warmup, Warmup::None);
}

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

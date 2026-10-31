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
fn validation_errors_name_the_request_or_plan_variables() {
    let base = std::path::Path::new(".");
    let mut p = TestPlan::default();
    let mut second = p.requests[0].clone();
    second.id = 2;
    second.name = "Pi".into();
    second.url = "test.k6.io/pi.php".into();
    p.requests.push(second);
    assert_eq!(p.validate(base).unwrap_err(), "Request \"Pi\": Invalid URL");
    p.requests[1].name = "  ".into();
    assert_eq!(p.validate(base).unwrap_err(), "Request #2: Invalid URL");
    p.requests[1].url = "https://test.k6.io/pi.php".into();
    p.variables.push(Field {
        name: "api_key".into(),
        value: String::new(),
        secret: true,
    });
    assert_eq!(
        p.validate(base).unwrap_err(),
        "Plan variables: Re-enter secret value: api_key"
    );
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
fn json_assertions_compare_objects_regardless_of_key_order() {
    // serde_json keeps document key order (preserve_order); object equality
    // must still ignore it while array order still counts.
    let a = Assertions {
        json_pointer: "/user".into(),
        json_expected: r#"{"name": "a", "id": 1, "tags": ["x", "y"]}"#.into(),
        ..Assertions::default()
    };
    assert!(evaluate(
        &a,
        200,
        1,
        br#"{"user": {"id": 1, "tags": ["x", "y"], "name": "a"}}"#
    ));
    assert!(!evaluate(
        &a,
        200,
        1,
        br#"{"user": {"id": 1, "tags": ["y", "x"], "name": "a"}}"#
    ));
    // Pointers use RFC 6901 escapes, unaffected by key order.
    let escaped = Assertions {
        json_pointer: "/a~1b/c~0d".into(),
        json_expected: "2".into(),
        ..Assertions::default()
    };
    assert!(evaluate(
        &escaped,
        200,
        1,
        br#"{"z": 0, "a/b": {"c~d": 2}}"#
    ));
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
        // Reasons can include plan text such as a JSON pointer.
        errors: vec![ErrorGroup {
            reason: "JSON at /<img> did not match".into(),
            count: 3,
            ..ErrorGroup::default()
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
    assert!(html.contains("<h2>Failure reasons</h2>"));
    assert!(html.contains("JSON at /&lt;img&gt; did not match"));
    assert!(!html.contains("<img>"));
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

fn stages(spec: &[(u64, u32)]) -> LoadConfig {
    LoadConfig {
        profile: Profile::Stages,
        stages: spec
            .iter()
            .map(|&(duration_secs, target_users)| Stage {
                duration_secs,
                target_users,
            })
            .collect(),
        ..LoadConfig::default()
    }
}

#[test]
fn every_profile_is_one_schedule() {
    // Constant: everyone from the start.
    let constant = LoadConfig::default();
    assert_eq!(constant.active_from(9, 0.0), Some(0.0));
    assert_eq!(constant.active_from(10, 0.0), None);
    // Ramp-up: user i starts at ramp * i / users, as before.
    let ramp = LoadConfig {
        profile: Profile::RampUp,
        users: 10,
        ramp_secs: 10,
        ..LoadConfig::default()
    };
    for i in 0..10 {
        let at = ramp.active_from(i, 0.0).unwrap();
        assert!((at - i as f64).abs() < 1e-3, "user {i} at {at}");
    }
    // Stages: up to 10, down to 0, up to 10 again.
    let s = stages(&[(10, 10), (10, 0), (10, 10)]);
    assert_eq!((s.max_users(), s.total_secs()), (10, 30));
    assert!((s.target_at(5.0) - 5.0).abs() < 1e-9);
    assert!((s.target_at(15.0) - 5.0).abs() < 1e-9);
    assert!((s.active_from(5, 0.0).unwrap() - 5.0).abs() < 1e-3);
    // After ramping down past it at 15 s, user 5 returns on the next rise at 25 s.
    assert!(s.target_at(15.5) <= 5.0);
    assert!((s.active_from(5, 15.5).unwrap() - 25.0).abs() < 1e-3);
    assert_eq!(stages(&[(10, 3)]).active_from(5, 0.0), None);
    assert_eq!(s.profile_label(), "stages 10s to 10, 10s to 0, 10s to 10");
}

#[test]
fn stage_profiles_validate() {
    let check = |load: LoadConfig| {
        TestPlan {
            load,
            ..TestPlan::default()
        }
        .validate(std::path::Path::new("."))
    };
    assert!(check(stages(&[(5, 10), (20, 10), (5, 0)])).is_ok());
    assert!(check(stages(&[])).is_err());
    assert!(check(stages(&[(0, 10)])).is_err());
    assert!(check(stages(&[(5, 10_001)])).is_err());
    assert!(
        check(stages(&[(5, 0)])).is_err(),
        "peak must be at least one user"
    );
    assert!(check(stages(&vec![(1, 1); STAGE_LIMIT + 1])).is_err());
    // Warmup limits use the stage total, not the unused duration field.
    let mut warm = stages(&[(5, 10), (5, 10)]);
    warm.duration_secs = 1;
    warm.warmup = Warmup::Duration { secs: 8 };
    assert!(check(warm).is_ok());

    let plan = TestPlan {
        load: stages(&[(5, 10)]),
        ..TestPlan::default()
    };
    let mut json = serde_json::to_value(&plan).unwrap();
    let back: TestPlan = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back.load.stages, plan.load.stages);
    json["load"].as_object_mut().unwrap().remove("stages");
    json["load"]["profile"] = "RampUp".into();
    let old: TestPlan = serde_json::from_value(json).unwrap();
    assert!(old.load.stages.is_empty());
}

#[test]
fn arrival_rate_profile_validates_and_labels() {
    let check = |rate: f64| {
        TestPlan {
            load: LoadConfig {
                profile: Profile::ArrivalRate,
                arrival_rate: rate,
                ..LoadConfig::default()
            },
            ..TestPlan::default()
        }
        .validate(std::path::Path::new("."))
    };
    assert!(check(0.5).is_ok());
    assert!(check(100_000.0).is_ok());
    for bad in [0.0, -1.0, 0.001, 100_001.0, f64::NAN, f64::INFINITY] {
        assert!(check(bad).is_err(), "{bad} must be rejected");
    }
    let open = LoadConfig {
        profile: Profile::ArrivalRate,
        arrival_rate: 2.5,
        users: 40,
        rps_limit: 1_000,
        ..LoadConfig::default()
    };
    assert_eq!(open.profile_label(), "arrival rate 2.5/s, max 40 VUs");
    assert_eq!(open.max_users(), 40);
    // The RPS cap does not apply to the open model; the arrival rate does.
    assert!(!open.is_elevated());
    assert!(
        LoadConfig {
            arrival_rate: 150.0,
            ..open.clone()
        }
        .is_elevated()
    );
    // Plans saved before the open model keep loading with the default rate.
    let mut json = serde_json::to_value(TestPlan::default()).unwrap();
    json["load"].as_object_mut().unwrap().remove("arrival_rate");
    let old: TestPlan = serde_json::from_value(json).unwrap();
    assert_eq!(old.load.arrival_rate, LoadConfig::default().arrival_rate);
}

fn rate_ramp(start: f64, spec: &[(u64, f64)]) -> LoadConfig {
    LoadConfig {
        profile: Profile::ArrivalRate,
        arrival_rate: start,
        duration_secs: 999,
        users: 50,
        rate_stages: spec
            .iter()
            .map(|&(duration_secs, target_rate)| RateStage {
                duration_secs,
                target_rate,
            })
            .collect(),
        ..LoadConfig::default()
    }
}

#[test]
fn arrival_starts_follow_the_integrated_rate() {
    let close = |a: Option<f64>, b: f64| {
        let a = a.expect("iteration is scheduled");
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    };
    // Constant rate: exactly k / rate as before, nothing at or after the end.
    let constant = LoadConfig {
        profile: Profile::ArrivalRate,
        arrival_rate: 7.0,
        duration_secs: 3,
        ..LoadConfig::default()
    };
    for k in 0..21 {
        assert_eq!(constant.arrival_start(k), Some(k as f64 / 7.0));
    }
    assert_eq!(constant.arrival_start(21), None);
    assert_eq!(constant.rate_schedule(), [(0.0, 7.0), (3.0, 7.0)]);
    // Linear ramp 0 to R over T: the integral R t^2 / (2T) reaches k at sqrt(2kT/R).
    let ramp = rate_ramp(0.0, &[(10, 50.0)]);
    for k in 0..250 {
        close(ramp.arrival_start(k), (2.0 * k as f64 * 10.0 / 50.0).sqrt());
    }
    assert_eq!(ramp.arrival_start(250), None, "the area is exactly 250");
    // From a start rate: 10 t + t^2 = k, so t = sqrt(25 + k) - 5.
    let start = rate_ramp(10.0, &[(10, 30.0)]);
    close(start.arrival_start(11), 1.0);
    close(start.arrival_start(56), 4.0);
    // Hold 10/s for 2 s, fall to 0 over 1 s, pause 3 s, rise to 10/s over 1 s.
    let multi = rate_ramp(10.0, &[(2, 10.0), (1, 0.0), (3, 0.0), (1, 10.0)]);
    assert_eq!(multi.total_secs(), 7);
    close(multi.arrival_start(19), 1.9);
    close(multi.arrival_start(20), 2.0);
    close(multi.arrival_start(21), 2.0 + 2.0 / (10.0 + 80f64.sqrt()));
    close(multi.arrival_start(25), 3.0);
    // The zero-rate stage starts nothing; starts resume on the next rise.
    close(multi.arrival_start(26), 6.0 + 0.2f64.sqrt());
    close(multi.arrival_start(29), 6.0 + 0.8f64.sqrt());
    assert_eq!(multi.arrival_start(30), None);
    let times: Vec<f64> = (0..30).map_while(|k| multi.arrival_start(k)).collect();
    assert_eq!(times.len(), 30);
    assert!(!times.iter().any(|t| *t > 3.0 + 1e-9 && *t < 6.0));
    // Start times never go backwards, whatever the slopes.
    let mixed = rate_ramp(3.3, &[(7, 91.7), (5, 0.4), (11, 250.0), (2, 0.0)]);
    let mut last = 0.0;
    let mut k = 0;
    while let Some(t) = mixed.arrival_start(k) {
        assert!(t >= last && t < 25.0, "iteration {k} at {t} after {last}");
        last = t;
        k += 1;
    }
    let area = (3.3 + 91.7) / 2.0 * 7.0
        + (91.7 + 0.4) / 2.0 * 5.0
        + (0.4 + 250.0) / 2.0 * 11.0
        + 250.0 / 2.0 * 2.0;
    assert!((k as f64 - area).abs() <= 1.0, "{k} starts for area {area}");
}

#[test]
fn rate_stages_validate_label_and_load_old_plans() {
    let check = |load: LoadConfig| {
        TestPlan {
            load,
            ..TestPlan::default()
        }
        .validate(std::path::Path::new("."))
    };
    assert!(check(rate_ramp(0.0, &[(60, 100.0), (240, 100.0), (60, 0.0)])).is_ok());
    assert!(
        check(rate_ramp(5.0, &[(1, 0.0)])).is_ok(),
        "start rate counts"
    );
    assert!(check(rate_ramp(0.0, &vec![(1, 1.0); STAGE_LIMIT])).is_ok());
    assert!(check(rate_ramp(0.0, &vec![(1, 1.0); STAGE_LIMIT + 1])).is_err());
    assert!(check(rate_ramp(0.0, &[(0, 10.0)])).is_err());
    assert!(
        check(rate_ramp(0.0, &[(5, 0.0)])).is_err(),
        "peak must be > 0"
    );
    assert!(check(rate_ramp(-1.0, &[(5, 10.0)])).is_err());
    assert!(check(rate_ramp(f64::NAN, &[(5, 10.0)])).is_err());
    for bad in [-0.5, 100_001.0, f64::NAN, f64::INFINITY] {
        assert!(check(rate_ramp(0.0, &[(5, bad)])).is_err(), "{bad}");
    }
    assert!(
        check(rate_ramp(0.0, &[(90_000, 1.0)])).is_err(),
        "over 86,400s"
    );
    // Without stages a zero rate stays invalid.
    assert!(check(rate_ramp(0.0, &[])).is_err());
    // The duration and warmup limits follow the stage total.
    let mut warm = rate_ramp(0.0, &[(5, 10.0), (5, 10.0)]);
    warm.duration_secs = 1;
    warm.warmup = Warmup::Duration { secs: 8 };
    assert_eq!(warm.total_secs(), 10);
    assert!(check(warm.clone()).is_ok());
    warm.warmup = Warmup::Duration { secs: 10 };
    assert!(check(warm).is_err());

    let staged = rate_ramp(10.0, &[(60, 100.0), (240, 100.0), (60, 0.0)]);
    assert_eq!(
        staged.profile_label(),
        "arrival rate 10/s, stages 60s to 100, 240s to 100, 60s to 0, max 50 VUs"
    );
    assert_eq!(staged.peak_rate(), 100.0);
    assert_eq!(
        staged.rate_schedule(),
        [(0.0, 10.0), (60.0, 100.0), (300.0, 100.0), (360.0, 0.0)]
    );
    assert!(!staged.is_elevated());
    assert!(rate_ramp(10.0, &[(5, 150.0), (5, 0.0)]).is_elevated());
    // Rate stages are ignored by the closed-model profiles.
    let closed = LoadConfig {
        profile: Profile::Constant,
        ..staged.clone()
    };
    assert_eq!(closed.total_secs(), 999);

    let plan = TestPlan {
        load: staged,
        ..TestPlan::default()
    };
    let mut json = serde_json::to_value(&plan).unwrap();
    let back: TestPlan = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back.load.rate_stages, plan.load.rate_stages);
    // Plans saved before rate stages keep a constant rate.
    json["load"].as_object_mut().unwrap().remove("rate_stages");
    let old: TestPlan = serde_json::from_value(json).unwrap();
    assert!(old.load.rate_stages.is_empty());
    assert_eq!(old.load.total_secs(), 999);
    assert_eq!(old.load.arrival_start(3), Some(0.3));
}

#[test]
fn arrival_summary_names_constant_and_staged_rates() {
    let mut s = Snapshot::default();
    assert_eq!(s.arrival_summary(), None);
    s.arrival_rate = Some(50.0);
    s.target_rate = vec![(0.0, 50.0), (2.0, 50.0)];
    assert_eq!(s.arrival_summary().as_deref(), Some("Arrival rate 50/s"));
    s.arrival_rate = Some(0.0);
    s.target_rate = vec![(0.0, 0.0), (60.0, 100.0), (120.0, 0.0)];
    assert_eq!(
        s.arrival_summary().as_deref(),
        Some("Arrival rate 0 to 100/s (stages)")
    );
    // History saved before rate stages has no schedule.
    s.target_rate.clear();
    s.arrival_rate = Some(20.0);
    assert_eq!(s.arrival_summary().as_deref(), Some("Arrival rate 20/s"));
}

#[test]
fn latency_distribution_covers_every_sample_in_log_buckets() {
    let mut h = histogram();
    assert!(distribution(&h).is_empty());
    // 1 ms to 2 s across three orders of magnitude.
    for us in [1_000, 1_200, 5_000, 5_100, 20_000, 250_000, 2_000_000] {
        h.record(us).unwrap();
    }
    h.record_n(5_000, 10).unwrap();
    let buckets = distribution(&h);
    assert_eq!(buckets.len(), DISTRIBUTION_BUCKETS);
    assert_eq!(buckets.iter().map(|b| b.count).sum::<u64>(), h.len());
    assert!((buckets[0].from_ms - 1.0).abs() < 0.01);
    assert!((buckets.last().unwrap().to_ms - 2000.0).abs() < 2.0);
    for w in buckets.windows(2) {
        assert!((w[0].to_ms - w[1].from_ms).abs() < 1e-9, "contiguous");
        // Equal ratios: log-spaced.
        let ratio = |b: &LatencyBucket| b.to_ms / b.from_ms;
        assert!((ratio(&w[0]) - ratio(&w[1])).abs() < 1e-9);
    }
    // The 12 responses around 5 ms land in one bar.
    let peak = buckets.iter().max_by_key(|b| b.count).unwrap();
    assert_eq!(peak.count, 12);
    assert!(peak.from_ms <= 5.0 && peak.to_ms >= 5.1, "{peak:?}");
    // One distinct value: a single bar holding every sample.
    let mut single = histogram();
    single.record_n(3_000, 7).unwrap();
    let one = distribution(&single);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].count, 7);
}

#[test]
fn error_groups_are_bounded_and_ordered() {
    let mut m = Accumulator::default();
    m.record_error(1, "HTTP 500, expected 2xx".into(), 1.0);
    m.record_error(1, "HTTP 500, expected 2xx".into(), 2.5);
    m.record_error(2, "HTTP 500, expected 2xx".into(), 3.0);
    let s = m.snapshot(4.0);
    assert_eq!(
        s.errors.len(),
        2,
        "same reason on another request is its own group"
    );
    assert_eq!(
        (
            s.errors[0].request_id,
            s.errors[0].count,
            s.errors[0].first_at,
            s.errors[0].last_at
        ),
        (1, 2, 1.0, 2.5),
        "most frequent first"
    );
    for i in 0..ERROR_GROUP_LIMIT as u64 {
        m.record_error(10 + i, "Timeout".into(), 5.0);
    }
    // Known groups keep counting at the limit; only new reasons overflow.
    m.record_error(1, "HTTP 500, expected 2xx".into(), 6.0);
    let s = m.snapshot(7.0);
    assert_eq!(s.errors.len(), ERROR_GROUP_LIMIT);
    assert_eq!(s.errors_overflow, 2);
    assert_eq!(s.errors[0].count, 3);
    assert_eq!(clip("abcdef", 3), "abc…");
    assert_eq!(clip("abc", 3), "abc");
}

#[test]
fn secret_fields_keep_whole_variable_references_only() {
    let field = |name: &str, value: &str, secret| Field {
        name: name.into(),
        value: value.into(),
        secret,
    };
    let mut plan = TestPlan {
        variables: vec![
            field("api_key", "literal-variable", true),
            field("ref_var", "{{other}}", true),
        ],
        ..TestPlan::default()
    };
    plan.requests[0].headers = vec![
        field("X-API-Key", "{{api_key}}", true),
        field("Authorization", "Bearer {{token}}", false),
        field("Cookie", "{{session_cookie}}", false),
        field("X-Plain", "visible", false),
        field("X-Literal", "literal-header", true),
        field("X-Spaced", "{{ api_key }}", true),
        field("X-Two", "{{a}}{{b}}", true),
        field("X-Dash", "{{api-key}}", true),
    ];
    plan.requests[0].query = vec![
        field("api_key", "{{api_key}}", false),
        field("token", "literal-query", false),
    ];
    assert!(is_variable_reference("{{api_key_2}}"));
    for not_ref in [
        "",
        "{{}}",
        "{{a}}{{b}}",
        "{{ a }}",
        "x{{a}}",
        "{{a-b}}",
        "{a}",
    ] {
        assert!(!is_variable_reference(not_ref), "{not_ref}");
    }
    let json = serde_json::to_string(&plan).unwrap();
    for literal in [
        "literal-variable",
        "Bearer {{token}}",
        "literal-header",
        "{{ api_key }}",
        "{{a}}{{b}}",
        "{{api-key}}",
        "literal-query",
    ] {
        assert!(!json.contains(literal), "{literal} was saved");
    }
    let restored: TestPlan = serde_json::from_str(&json).unwrap();
    let values = |fields: &[Field]| -> Vec<(String, String, bool)> {
        fields
            .iter()
            .map(|f| (f.name.clone(), f.value.clone(), f.secret))
            .collect()
    };
    let h = values(&restored.requests[0].headers);
    assert_eq!(h[0], ("X-API-Key".into(), "{{api_key}}".into(), true));
    assert_eq!(h[1].1, "");
    assert_eq!(h[2], ("Cookie".into(), "{{session_cookie}}".into(), true));
    assert_eq!(h[3].1, "visible");
    assert!(h[4..].iter().all(|f| f.1.is_empty() && f.2));
    let q = values(&restored.requests[0].query);
    assert_eq!(q[0], ("api_key".into(), "{{api_key}}".into(), true));
    assert_eq!(q[1].1, "");
    let v = values(&restored.variables);
    assert_eq!(v[0].1, "");
    assert_eq!(v[1].1, "{{other}}");

    // History and reports use the redacted plan: same rule.
    let redacted = serde_json::to_string(&plan.redacted()).unwrap();
    assert!(redacted.contains("{{api_key}}"));
    for literal in [
        "literal-variable",
        "literal-header",
        "literal-query",
        "Bearer {{token}}",
    ] {
        assert!(!redacted.contains(literal), "{literal} in history");
    }
}

#[test]
fn saved_plans_load_and_bad_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plan.json");
    let plan = TestPlan {
        name: "Checkout".into(),
        ..TestPlan::default()
    };
    std::fs::write(&path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    assert_eq!(TestPlan::load(&path).unwrap().name, "Checkout");
    // A missing file is an error, so the app forgets the remembered path.
    assert!(TestPlan::load(&dir.path().join("gone.json")).is_err());
    let mut json = serde_json::to_value(&plan).unwrap();
    json["schema_version"] = 99.into();
    std::fs::write(&path, json.to_string()).unwrap();
    assert!(TestPlan::load(&path).unwrap_err().contains("schema"));
    std::fs::write(&path, "not json").unwrap();
    assert!(TestPlan::load(&path).is_err());
    let big = dir.path().join("big.json");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(TestPlan::load(&big).unwrap_err().contains("16 MiB"));
}

#[test]
fn k6_demo_plan_is_valid_and_keeps_its_token_out_of_saved_plans() {
    let base = std::path::Path::new(".");
    let plan = rustload::fixture::k6_demo_plan("0123456789abcdef");
    plan.validate(base).unwrap();
    assert!(
        plan.requests
            .iter()
            .all(|r| r.url.starts_with(rustload::fixture::K6_DEMO_ORIGIN)
                && r.assertions.status == Some(200)
                && r.assertions.max_ms.is_some())
    );
    assert_eq!(plan.requests[2].assertions.json_pointer, "/doughs/0/name");
    assert_eq!(plan.requests[3].extractors[0].variable, "pizza_name");
    assert_eq!(plan.thresholds.len(), 4);
    assert_eq!(plan.load.warmup, Warmup::Duration { secs: 10 });
    let json = serde_json::to_string(&plan).unwrap();
    assert!(!json.contains("0123456789abcdef"));
    assert!(json.contains("{{pizza_token}}"));
    assert!(json.contains("maxCaloriesPerSlice"));
    // Reopened from disk, the secret token must be entered again.
    let restored: TestPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored.validate(base).unwrap_err(),
        "Plan variables: Re-enter secret value: pizza_token"
    );
}

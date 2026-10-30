use rustload::{compare::*, core::*, metrics::*};

fn run(completed: u64, failed: u64, p95: f64, elapsed: f64) -> Snapshot {
    let latency = Latency {
        samples: completed,
        p50: p95 / 2.0,
        p90: p95 * 0.9,
        p95,
        p99: p95 * 1.5,
        mean: p95 / 2.0,
    };
    let counts = Counts {
        completed,
        successful: completed - failed,
        failed,
        bytes: completed * 100,
        ..Counts::default()
    };
    Snapshot {
        state: RunState::Completed,
        elapsed,
        counts: counts.clone(),
        latency: latency.clone(),
        endpoints: vec![Endpoint {
            id: 1,
            name: "HTTP Request".into(),
            counts,
            latency,
            ..Endpoint::default()
        }],
        ..Snapshot::default()
    }
}

fn row(c: &Comparison, metric: CompareMetric) -> &Delta {
    c.overall.iter().find(|d| d.metric == metric).unwrap()
}

#[test]
fn directions_noise_band_and_verdicts() {
    let plan = TestPlan::default();
    let base = run(1000, 0, 100.0, 10.0);

    // Identical runs: everything unchanged.
    let same = compare(&base, &plan, &base, &plan, DEFAULT_NOISE_PCT);
    assert_eq!(same.verdict, CompareVerdict::Unchanged);
    assert!(same.warnings.is_empty());

    // Latency up 2 % sits on the noise boundary and stays unchanged; 2.5 % regresses.
    let edge = compare(&base, &plan, &run(1000, 0, 102.0, 10.0), &plan, 2.0);
    assert_eq!(row(&edge, CompareMetric::P95).change, Change::Unchanged);
    let slower = compare(&base, &plan, &run(1000, 0, 102.5, 10.0), &plan, 2.0);
    let p95 = row(&slower, CompareMetric::P95);
    assert_eq!(p95.change, Change::Regressed);
    assert!((p95.change_pct.unwrap() - 2.5).abs() < 1e-9);
    assert_eq!(slower.verdict, CompareVerdict::Regressed);

    // Throughput up and latency down: improved; data rate is informational only.
    let faster = compare(&base, &plan, &run(1200, 0, 80.0, 10.0), &plan, 2.0);
    assert_eq!(
        row(&faster, CompareMetric::Throughput).change,
        Change::Improved
    );
    assert_eq!(row(&faster, CompareMetric::P95).change, Change::Improved);
    assert_eq!(
        row(&faster, CompareMetric::DataRate).change,
        Change::Changed
    );
    assert_eq!(faster.verdict, CompareVerdict::Improved);

    // More throughput but slower: mixed.
    let mixed = compare(&base, &plan, &run(1200, 0, 150.0, 10.0), &plan, 2.0);
    assert_eq!(mixed.verdict, CompareVerdict::Mixed);
}

#[test]
fn zero_baseline_and_missing_data() {
    let plan = TestPlan::default();
    let clean = run(1000, 0, 100.0, 10.0);
    let errors = compare(&clean, &plan, &run(1000, 10, 100.0, 10.0), &plan, 2.0);
    let rate = row(&errors, CompareMetric::ErrorRate);
    // From 0 % there is no relative change, but the move is still a regression.
    assert_eq!((rate.change_pct, rate.change), (None, Change::Regressed));

    let empty = Snapshot {
        state: RunState::Completed,
        ..Snapshot::default()
    };
    let none = compare(&empty, &plan, &empty, &plan, 2.0);
    assert_eq!(none.verdict, CompareVerdict::NoData);
    assert!(none.overall.iter().all(|d| d.change == Change::NoData));
}

#[test]
fn endpoints_match_by_request_id() {
    let plan = TestPlan::default();
    let base = run(1000, 0, 100.0, 10.0);
    let mut candidate = run(1000, 0, 200.0, 10.0);
    candidate.endpoints[0].name = "Renamed".into();
    candidate.endpoints.push(Endpoint {
        id: 2,
        name: "Checkout".into(),
        ..Endpoint::default()
    });
    let c = compare(&base, &plan, &candidate, &plan, 2.0);
    assert_eq!(c.endpoints.len(), 1);
    assert_eq!(c.endpoints[0].name, "Renamed");
    let p95 = c.endpoints[0]
        .deltas
        .iter()
        .find(|d| d.metric == CompareMetric::P95)
        .unwrap();
    assert_eq!(p95.change, Change::Regressed);
    assert!(c.only_base.is_empty());
    assert_eq!(c.only_candidate, ["Checkout"]);
}

#[test]
fn different_setups_are_compared_with_warnings() {
    let base_plan = TestPlan::default();
    let mut candidate_plan = TestPlan::default();
    candidate_plan.load.users = 50;
    candidate_plan.load.profile = Profile::RampUp;
    candidate_plan.requests[0].method = "POST".into();
    let base = run(1000, 0, 100.0, 10.0);
    let candidate = Snapshot {
        state: RunState::Cancelled,
        ..run(1000, 0, 100.0, 10.0)
    };
    let c = compare(&base, &base_plan, &candidate, &candidate_plan, 2.0);
    let text = c.warnings.join("\n");
    assert!(text.contains("Candidate run did not complete normally (Cancelled)"));
    assert!(text.contains("Virtual users differs: 10 vs 50"));
    assert!(text.contains("Load profile differs: constant vs ramp-up 10s"));
    assert!(text.contains("Request steps differ"));
    assert_eq!(c.verdict, CompareVerdict::Unchanged);
}

#[test]
fn run_ids_show_their_utc_start_time() {
    // 2026-10-30 01:56:21.5 UTC in nanoseconds.
    assert_eq!(
        run_time_label("1793325381500000000"),
        "2026-10-30 01:56 UTC"
    );
    assert_eq!(run_time_label("0"), "1970-01-01 00:00 UTC");
    assert_eq!(run_time_label("951782400000000000"), "2000-02-29 00:00 UTC");
    assert_eq!(run_time_label("not-a-run"), "");
}

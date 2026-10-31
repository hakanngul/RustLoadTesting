//! History database: schema versioning, migration of old files, the queryable
//! `run_*` tables and export. Runs are real HTTP traffic to the localhost fixture.
use rusqlite::{Connection, OptionalExtension};
use rustload::{
    core::*,
    engine, fixture,
    metrics::*,
    storage::{History, SCHEMA_VERSION},
};
use std::{path::Path, time::Duration};

/// Exact schema written by versions before schema versioning (user_version 0).
const V0_SCHEMA: &str = "PRAGMA journal_mode=WAL;
    CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, name TEXT NOT NULL, state TEXT NOT NULL,
        elapsed REAL NOT NULL, completed INTEGER NOT NULL, config TEXT NOT NULL, summary TEXT);
    CREATE TABLE IF NOT EXISTS points (run_id TEXT NOT NULL, sequence INTEGER NOT NULL, data TEXT NOT NULL,
        PRIMARY KEY(run_id, sequence));";

type SloRow = (String, Option<u64>, f64, Option<f64>, String);

const SECRET: &str = "s3cr3t-history-value";

/// A finished real run with warmup, two endpoints (one always failing), a
/// secret header, a secret query value and two SLOs.
fn real_run(db: &Path) -> Snapshot {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(fixture::spawn(0)).unwrap();
    let mut plan = TestPlan {
        name: "History run".into(),
        ..TestPlan::default()
    };
    plan.requests[0].name = "Health".into();
    plan.requests[0].url = format!("http://{}/health", server.address);
    plan.requests[0].query.push(Field {
        name: "api_key".into(),
        value: SECRET.into(),
        secret: true,
    });
    plan.requests[0].headers.push(Field {
        name: "X-Token".into(),
        value: SECRET.into(),
        secret: true,
    });
    plan.requests.push(Request {
        id: 2,
        name: "Broken".into(),
        method: "POST".into(),
        url: format!("http://{}/status/500", server.address),
        ..Request::default()
    });
    plan.load.users = 4;
    plan.load.duration_secs = 3;
    plan.load.rps_limit = 40;
    plan.load.think_ms = 0;
    plan.load.warmup = Warmup::Duration { secs: 1 };
    plan.thresholds = vec![
        Threshold {
            metric: SloMetric::P95,
            value: 5_000.0,
            request_id: Some(1),
        },
        Threshold {
            metric: SloMetric::ErrorRate,
            value: 1.0,
            request_id: None,
        },
    ];
    let mut handle = engine::start(&rt, plan, Path::new("."), db.to_owned()).unwrap();
    let s = rt.block_on(async {
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
    });
    server.stop.cancel();
    assert_eq!(s.state, RunState::Completed);
    assert!(s.warmup_counts.started > 0 && s.counts.completed > 0);
    assert!(s.counts.failed > 0 && s.endpoints.len() == 2);
    s
}

fn user_version(db: &Path) -> i64 {
    Connection::open(db)
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

fn count(c: &Connection, table: &str, run: &str) -> i64 {
    c.query_row(
        &format!("SELECT count(*) FROM {table} WHERE run_id = ?1"),
        [run],
        |r| r.get(0),
    )
    .unwrap()
}

/// Every derived row of `run` matches the snapshot it came from.
fn assert_tables_match(db: &Path, run: &str, s: &Snapshot) {
    let c = Connection::open(db).unwrap();
    type Row = (String, String, i64, i64, i64, i64, f64, f64, Option<f64>);
    let row: Row = c
        .query_row(
            "SELECT name, state, started, completed, failed, warmup_started, p95, measured_secs,
                throughput FROM run_metrics WHERE run_id = ?1",
            [run],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(row.0, s.name);
    assert_eq!(row.1, "Completed");
    // Warmup is excluded: the measured counters only, warmup kept apart.
    assert_eq!(row.2 as u64, s.counts.started);
    assert_eq!(row.3 as u64, s.counts.completed);
    assert_eq!(row.4 as u64, s.counts.failed);
    assert_eq!(row.5 as u64, s.warmup_counts.started);
    assert_eq!(row.6, s.latency.p95);
    assert_eq!(row.7, s.measured_elapsed());
    assert!(
        s.measured_elapsed() < s.elapsed,
        "warmup window not measured"
    );
    let throughput = row.8.unwrap();
    assert!((throughput - s.counts.completed as f64 / s.measured_elapsed()).abs() < 1e-9);
    let verdict: Option<String> = c
        .query_row(
            "SELECT slo_verdict FROM run_metrics WHERE run_id = ?1",
            [run],
            |r| r.get(0),
        )
        .unwrap();
    let slo = s.slo.as_ref().unwrap();
    assert_eq!(
        verdict.as_deref(),
        Some(format!("{:?}", slo.verdict).as_str())
    );
    // Start time comes from the nanosecond run id.
    let started_at: String = c
        .query_row(
            "SELECT started_at FROM run_metrics WHERE run_id = ?1",
            [run],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        &started_at[..16],
        &rustload::compare::run_time_label(run)[..16]
    );

    assert_eq!(count(&c, "run_endpoints", run) as usize, s.endpoints.len());
    for e in &s.endpoints {
        let (name, method, completed, failed, p95): (String, String, i64, i64, Option<f64>) = c
            .query_row(
                "SELECT name, method, completed, failed, p95 FROM run_endpoints
                    WHERE run_id = ?1 AND request_id = ?2",
                rusqlite::params![run, e.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!((name, method), (e.name.clone(), e.method.clone()));
        assert_eq!(
            (completed as u64, failed as u64),
            (e.counts.completed, e.counts.failed)
        );
        assert_eq!(p95, (e.latency.samples > 0).then_some(e.latency.p95));
    }

    assert_eq!(count(&c, "run_slo", run) as usize, slo.results.len());
    let mut query = c
        .prepare(
            "SELECT metric, scope_request_id, target, actual, status FROM run_slo
                WHERE run_id = ?1 ORDER BY position",
        )
        .unwrap();
    let rows: Vec<SloRow> = query
        .query_map([run], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for (row, result) in rows.iter().zip(&slo.results) {
        assert_eq!(row.0, format!("{:?}", result.threshold.metric));
        assert_eq!(row.1, result.threshold.request_id);
        assert_eq!(row.2, result.threshold.value);
        assert_eq!(row.3, result.actual);
        assert_eq!(row.4, format!("{:?}", result.status));
    }

    assert_eq!(count(&c, "run_errors", run) as usize, s.errors.len());
    let total: i64 = c
        .query_row(
            "SELECT coalesce(sum(count), 0) FROM run_errors WHERE run_id = ?1",
            [run],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(total as u64 + s.errors_overflow, s.counts.failed);
    let reason: String = c
        .query_row(
            "SELECT reason FROM run_errors WHERE run_id = ?1 AND request_id = 2",
            [run],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reason, "HTTP 500, expected 2xx");

    // No secret, query string or URL reached the new tables.
    let mut text = String::new();
    for (table, columns) in [
        ("run_metrics", "name || state || coalesce(slo_verdict, '')"),
        ("run_endpoints", "name || method"),
        ("run_slo", "metric || unit || status"),
        ("run_errors", "reason"),
    ] {
        let mut q = c
            .prepare(&format!("SELECT {columns} FROM {table}"))
            .unwrap();
        for value in q.query_map([], |r| r.get::<_, String>(0)).unwrap() {
            text.push_str(&value.unwrap());
        }
    }
    assert!(!text.is_empty());
    for leaked in [SECRET, "api_key", "http", "127.0.0.1", "?"] {
        assert!(!text.contains(leaked), "{leaked} in derived tables: {text}");
    }
}

#[test]
fn finished_run_fills_query_tables_and_refinish_replaces_them() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("history.sqlite3");
    let s = real_run(&db);
    assert_eq!(user_version(&db), SCHEMA_VERSION);
    assert_tables_match(&db, &s.run_id, &s);
    // The JSON summary stays the source of truth, written as before.
    let history = History::open(&db).unwrap();
    assert!(history.migration_issue().is_none());
    let (saved, config) = history.read(&s.run_id).unwrap();
    assert_eq!(saved.counts.completed, s.counts.completed);
    assert!(!serde_json::to_string(&config).unwrap().contains(SECRET));

    // Finishing again replaces the derived rows instead of adding to them.
    let mut changed = s.clone();
    changed.endpoints.truncate(1);
    changed.errors.clear();
    changed.errors_overflow = changed.counts.failed;
    history.finish(&changed).unwrap();
    let c = Connection::open(&db).unwrap();
    assert_eq!(count(&c, "run_endpoints", &s.run_id), 1);
    assert_eq!(count(&c, "run_errors", &s.run_id), 0);
    assert_eq!(count(&c, "run_metrics", &s.run_id), 1);
}

#[test]
fn version_zero_database_migrates_backfills_once_and_stays_readable() {
    let dir = tempfile::tempdir().unwrap();
    // A real run's redacted summary and plan, stored in a file with the old schema.
    let s = real_run(&dir.path().join("source.sqlite3"));
    let (summary, config) = {
        let c = Connection::open(dir.path().join("source.sqlite3")).unwrap();
        c.query_row(
            "SELECT summary, config FROM runs WHERE id = ?1",
            [&s.run_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap()
    };
    let db = dir.path().join("old.sqlite3");
    {
        let c = Connection::open(&db).unwrap();
        c.execute_batch(V0_SCHEMA).unwrap();
        let insert = "INSERT INTO runs VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
        c.execute(
            insert,
            rusqlite::params![
                s.run_id,
                s.name,
                "Completed",
                s.elapsed,
                s.counts.completed,
                config,
                summary
            ],
        )
        .unwrap();
        // An interrupted run (no summary) and a damaged summary are skipped.
        c.execute(
            insert,
            rusqlite::params![
                "1",
                "Interrupted",
                "Running",
                0.0,
                0,
                config,
                None::<String>
            ],
        )
        .unwrap();
        c.execute(
            insert,
            rusqlite::params!["2", "Damaged", "Completed", 1.0, 1, config, "{not json"],
        )
        .unwrap();
        c.execute(
            "INSERT INTO points VALUES (?1, 0, ?2)",
            rusqlite::params![
                s.run_id,
                serde_json::to_string(s.series.front().unwrap()).unwrap()
            ],
        )
        .unwrap();
        assert_eq!(
            c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    let history = History::open(&db).unwrap();
    assert!(history.migration_issue().is_none());
    assert_eq!(history.schema_version(), SCHEMA_VERSION);
    assert_eq!(user_version(&db), SCHEMA_VERSION);
    // The history list, summaries and points read as before.
    assert_eq!(history.list().unwrap().len(), 3);
    assert_eq!(
        history.read(&s.run_id).unwrap().0.counts.completed,
        s.counts.completed
    );
    assert_eq!(history.points(&s.run_id).unwrap().len(), 1);
    drop(history);
    assert_tables_match(&db, &s.run_id, &s);
    let c = Connection::open(&db).unwrap();
    let backfilled: i64 = c
        .query_row("SELECT count(*) FROM run_metrics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        backfilled, 1,
        "only the readable finished run is backfilled"
    );

    // Opening again is a no-op: a hand-edited derived value survives.
    c.execute("UPDATE run_metrics SET p95 = -1", []).unwrap();
    let history = History::open(&db).unwrap();
    assert_eq!(history.list().unwrap().len(), 3);
    drop(history);
    let p95: f64 = c
        .query_row("SELECT p95 FROM run_metrics", [], |r| r.get(0))
        .unwrap();
    assert_eq!(p95, -1.0);
    assert_eq!(user_version(&db), SCHEMA_VERSION);
}

#[test]
fn runs_finished_by_an_older_app_are_filled_on_next_open() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("history.sqlite3");
    let s = real_run(&db);
    // An older app writes only the JSON row; the derived rows are missing.
    let c = Connection::open(&db).unwrap();
    for table in ["run_metrics", "run_endpoints", "run_slo", "run_errors"] {
        c.execute(&format!("DELETE FROM {table}"), []).unwrap();
    }
    History::open(&db).unwrap();
    assert_tables_match(&db, &s.run_id, &s);
}

#[test]
fn failed_migration_keeps_history_usable_and_reports_it() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("history.sqlite3");
    let s = real_run(&dir.path().join("source.sqlite3"));
    {
        let c = Connection::open(&db).unwrap();
        c.execute_batch(V0_SCHEMA).unwrap();
        // A foreign table with the same name makes the backfill fail.
        c.execute_batch("CREATE TABLE run_metrics (unrelated TEXT)")
            .unwrap();
        c.execute(
            "INSERT INTO runs VALUES (?1, 'Old', 'Completed', 1.0, 1, ?2, ?3)",
            rusqlite::params![
                "1",
                serde_json::to_string(&TestPlan::default().redacted()).unwrap(),
                serde_json::to_string(&s).unwrap()
            ],
        )
        .unwrap();
    }
    let history = History::open(&db).unwrap();
    let issue = history
        .migration_issue()
        .expect("migration failure reported");
    assert!(issue.contains("could not be upgraded"), "{issue}");
    // SQLite's reason only, not the statement text rusqlite appends.
    assert!(!issue.contains("CREATE") && issue.len() < 200, "{issue}");
    assert_eq!(history.schema_version(), 0);
    assert_eq!(user_version(&db), 0, "failed upgrade rolled back");
    // Saved runs still list, open and finish.
    assert_eq!(history.list().unwrap().len(), 1);
    assert_eq!(
        history.read("1").unwrap().0.counts.completed,
        s.counts.completed
    );
    history.begin("2", &TestPlan::default()).unwrap();
    let mut finished = s.clone();
    finished.run_id = "2".into();
    history.finish(&finished).unwrap();
    assert_eq!(history.read("2").unwrap().0.counts.failed, s.counts.failed);
    let tables: Option<String> = Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT name FROM sqlite_master WHERE name = 'run_endpoints'",
            [],
            |r| r.get(0),
        )
        .optional()
        .unwrap();
    assert_eq!(tables, None, "partial schema rolled back");
}

#[test]
fn export_writes_a_readable_copy_and_refuses_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("history.sqlite3");
    let s = real_run(&db);
    let history = History::open(&db).unwrap();
    let target = dir.path().join("rustload-history.sqlite3");
    history.export(&target).unwrap();

    let copy = History::open(&target).unwrap();
    assert_eq!(copy.schema_version(), SCHEMA_VERSION);
    let ids = |h: &History| {
        h.list()
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&copy), ids(&history));
    assert_eq!(
        copy.read(&s.run_id).unwrap().0.counts.completed,
        s.counts.completed
    );
    assert_eq!(
        copy.points(&s.run_id).unwrap().len(),
        history.points(&s.run_id).unwrap().len()
    );
    drop(copy);
    assert_tables_match(&target, &s.run_id, &s);

    // An existing file is never replaced.
    let before = std::fs::read(&target).unwrap();
    let error = history.export(&target).unwrap_err();
    assert!(error.contains("already exists"), "{error}");
    assert_eq!(std::fs::read(&target).unwrap(), before);
    let other = dir.path().join("notes.txt");
    std::fs::write(&other, "keep me").unwrap();
    assert!(history.export(&other).is_err());
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "keep me");
}

#[test]
fn unusable_derived_tables_never_lose_a_finished_run() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("history.sqlite3");
    let s = real_run(&db);
    // A v1 file whose derived table vanished after the upgrade: the next open
    // cannot backfill and reports it, but finishing a run must still save it.
    Connection::open(&db)
        .unwrap()
        .execute_batch("DROP TABLE run_endpoints; DELETE FROM run_metrics;")
        .unwrap();
    let history = History::open(&db).unwrap();
    assert!(history.migration_issue().is_some());
    history.begin("2", &TestPlan::default()).unwrap();
    let mut finished = s.clone();
    finished.run_id = "2".into();
    history.finish(&finished).unwrap();
    assert_eq!(
        history.read("2").unwrap().0.counts.completed,
        s.counts.completed
    );
    // Same when the table disappears while a healthy connection is open.
    let db2 = dir.path().join("second.sqlite3");
    let s2 = real_run(&db2);
    let history = History::open(&db2).unwrap();
    assert!(history.migration_issue().is_none());
    Connection::open(&db2)
        .unwrap()
        .execute_batch("DROP TABLE run_errors")
        .unwrap();
    history.begin("3", &TestPlan::default()).unwrap();
    let mut finished = s2.clone();
    finished.run_id = "3".into();
    history.finish(&finished).unwrap();
    assert_eq!(
        history.read("3").unwrap().0.counts.completed,
        s2.counts.completed
    );
}

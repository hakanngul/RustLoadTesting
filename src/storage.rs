use crate::{
    core::TestPlan,
    metrics::{Point, Snapshot},
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn data_directory() -> PathBuf {
    directories::ProjectDirs::from("dev", "RustLoad", "RustLoad")
        .map(|p| p.data_dir().to_owned())
        .unwrap_or_else(|| PathBuf::from(".rustload"))
}
pub fn default_db() -> PathBuf {
    data_directory().join("history.sqlite3")
}
/// Schema version kept in `PRAGMA user_version`. Version 0 is the original
/// `runs`/`points` layout; version 1 adds the queryable `run_*` tables, which
/// are derived from `runs.summary` (still the source of truth).
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = "
    CREATE TABLE IF NOT EXISTS run_metrics (
        run_id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        started_at TEXT,
        state TEXT NOT NULL,
        elapsed REAL NOT NULL,
        measured_secs REAL NOT NULL,
        started INTEGER NOT NULL,
        completed INTEGER NOT NULL,
        successful INTEGER NOT NULL,
        failed INTEGER NOT NULL,
        cancelled INTEGER NOT NULL,
        error_rate REAL,
        throughput REAL,
        latency_samples INTEGER NOT NULL,
        p50 REAL,
        p90 REAL,
        p95 REAL,
        p99 REAL,
        mean REAL,
        bytes INTEGER NOT NULL,
        warmup_started INTEGER NOT NULL,
        warmup_completed INTEGER NOT NULL,
        dropped_iterations INTEGER NOT NULL,
        errors_overflow INTEGER NOT NULL,
        slo_verdict TEXT,
        slo_partial INTEGER
    );
    CREATE INDEX IF NOT EXISTS run_metrics_started_at ON run_metrics(started_at);
    CREATE INDEX IF NOT EXISTS run_metrics_name_started_at ON run_metrics(name, started_at);
    CREATE TABLE IF NOT EXISTS run_endpoints (
        run_id TEXT NOT NULL,
        request_id INTEGER NOT NULL,
        name TEXT NOT NULL,
        method TEXT NOT NULL,
        completed INTEGER NOT NULL,
        successful INTEGER NOT NULL,
        failed INTEGER NOT NULL,
        error_rate REAL,
        latency_samples INTEGER NOT NULL,
        p50 REAL,
        p90 REAL,
        p95 REAL,
        p99 REAL,
        mean REAL,
        bytes INTEGER NOT NULL,
        PRIMARY KEY (run_id, request_id)
    );
    CREATE TABLE IF NOT EXISTS run_slo (
        run_id TEXT NOT NULL,
        position INTEGER NOT NULL,
        metric TEXT NOT NULL,
        scope_request_id INTEGER,
        target REAL NOT NULL,
        unit TEXT NOT NULL,
        actual REAL,
        status TEXT NOT NULL,
        PRIMARY KEY (run_id, position)
    );
    CREATE TABLE IF NOT EXISTS run_errors (
        run_id TEXT NOT NULL,
        request_id INTEGER NOT NULL,
        reason TEXT NOT NULL,
        count INTEGER NOT NULL,
        first_at REAL NOT NULL,
        last_at REAL NOT NULL,
        PRIMARY KEY (run_id, request_id, reason)
    );";

pub struct History {
    connection: Connection,
    /// Schema version this connection reads and writes.
    version: i64,
    /// Why the upgrade to `SCHEMA_VERSION` failed; runs still load and save.
    migration_issue: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub name: String,
    pub state: String,
    pub elapsed: f64,
    pub completed: u64,
}
impl History {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        connection.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS runs (id TEXT PRIMARY KEY, name TEXT NOT NULL, state TEXT NOT NULL,
                elapsed REAL NOT NULL, completed INTEGER NOT NULL, config TEXT NOT NULL, summary TEXT);
            CREATE TABLE IF NOT EXISTS points (run_id TEXT NOT NULL, sequence INTEGER NOT NULL, data TEXT NOT NULL,
                PRIMARY KEY(run_id, sequence));").map_err(|e| e.to_string())?;
        let (version, migration_issue) = match migrate(&connection) {
            Ok(version) => (version, None),
            Err(e) => (
                user_version(&connection).unwrap_or(0),
                Some(format!(
                    "History database could not be upgraded to schema {SCHEMA_VERSION}: {e}"
                )),
            ),
        };
        Ok(Self {
            connection,
            version,
            migration_issue,
        })
    }
    /// Schema version of the open database (`PRAGMA user_version`).
    pub fn schema_version(&self) -> i64 {
        self.version
    }
    /// Set when the schema upgrade failed. Saved runs still list, open and
    /// save; only the queryable `run_*` tables are not kept up to date.
    pub fn migration_issue(&self) -> Option<&str> {
        self.migration_issue.as_deref()
    }
    /// Writes a consistent copy of the whole database to `target` with
    /// `VACUUM INTO`. An existing file is never replaced.
    pub fn export(&self, target: &Path) -> Result<(), String> {
        if target.exists() {
            return Err(format!(
                "{} already exists; choose a new file name",
                target.display()
            ));
        }
        let path = target
            .to_str()
            .ok_or("The export path must be valid UTF-8")?;
        self.connection
            .execute("VACUUM INTO ?1", [path])
            .map_err(|e| format!("Could not export history to {}: {e}", target.display()))?;
        Ok(())
    }
    pub fn begin(&self, id: &str, plan: &TestPlan) -> Result<(), String> {
        let config = serde_json::to_string(&plan.redacted()).map_err(|e| e.to_string())?;
        self.connection
            .execute(
                "INSERT INTO runs VALUES (?1,?2,'Running',0,0,?3,NULL)",
                params![id, plan.name, config],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn point(&self, id: &str, sequence: u64, p: &Point) -> Result<(), String> {
        let json = serde_json::to_string(p).map_err(|e| e.to_string())?;
        self.connection
            .execute(
                "INSERT OR REPLACE INTO points VALUES (?1,?2,?3)",
                params![id, sequence, json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn finish(&self, s: &Snapshot) -> Result<(), String> {
        let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
        let tx = self
            .connection
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE runs SET state=?2, elapsed=?3, completed=?4, summary=?5 WHERE id=?1",
            params![
                s.run_id,
                format!("{:?}", s.state),
                s.elapsed,
                s.counts.completed,
                json
            ],
        )
        .map_err(|e| e.to_string())?;
        // A newer schema may have changed the derived tables; leave them alone.
        if self.version == SCHEMA_VERSION {
            write_tables(&tx, &s.run_id, s).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn list(&self) -> Result<Vec<HistoryItem>, String> {
        let mut query = self
            .connection
            .prepare("SELECT id,name,state,elapsed,completed FROM runs ORDER BY id DESC LIMIT 200")
            .map_err(|e| e.to_string())?;
        query
            .query_map([], |r| {
                Ok(HistoryItem {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    state: r.get(2)?,
                    elapsed: r.get(3)?,
                    completed: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn read(&self, id: &str) -> Result<(Snapshot, TestPlan), String> {
        let (summary, config): (Option<String>, String) = self
            .connection
            .query_row("SELECT summary,config FROM runs WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|e| e.to_string())?;
        Ok((
            serde_json::from_str(
                &summary
                    .ok_or("Run was interrupted or is still active; final summary unavailable")?,
            )
            .map_err(|e| e.to_string())?,
            serde_json::from_str(&config).map_err(|e| e.to_string())?,
        ))
    }
    pub fn points(&self, id: &str) -> Result<Vec<Point>, String> {
        let mut query = self
            .connection
            .prepare("SELECT data FROM points WHERE run_id=?1 ORDER BY sequence")
            .map_err(|e| e.to_string())?;
        let strings = query
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        strings
            .into_iter()
            .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .collect()
    }
}

fn user_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA user_version", [], |r| r.get(0))
}

/// Brings the schema to `SCHEMA_VERSION` and returns the version in use.
/// Idempotent: the version is re-checked inside an immediate transaction, so
/// two connections opening an old file at once upgrade it once. Runs without
/// derived rows (finished by an older app, or saved before the upgrade) are
/// filled from their summary JSON; unreadable summaries are skipped. With
/// nothing to do this is a single read.
fn migrate(connection: &Connection) -> rusqlite::Result<i64> {
    let version = user_version(connection)?;
    if version > SCHEMA_VERSION {
        return Ok(version);
    }
    if version == SCHEMA_VERSION && missing_runs(connection)?.is_empty() {
        return Ok(version);
    }
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let version = user_version(connection)?;
        if version > SCHEMA_VERSION {
            return Ok(version);
        }
        if version < 1 {
            connection.execute_batch(SCHEMA_V1)?;
        }
        for (id, summary) in missing_runs(connection)? {
            if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&summary) {
                write_tables(connection, &id, &snapshot)?;
            }
        }
        connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(SCHEMA_VERSION)
    })();
    match result {
        Ok(version) => {
            connection.execute_batch("COMMIT")?;
            Ok(version)
        }
        Err(e) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Finished runs (with a summary) that have no `run_metrics` row yet.
fn missing_runs(connection: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut query = connection.prepare(
        "SELECT r.id, r.summary FROM runs r WHERE r.summary IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM run_metrics m WHERE m.run_id = r.id)",
    )?;
    query
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect()
}

/// Latency figures, or `None` for each when nothing was measured.
fn percentiles(l: &crate::metrics::Latency) -> [Option<f64>; 5] {
    if l.samples == 0 {
        return [None; 5];
    }
    [
        Some(l.p50),
        Some(l.p90),
        Some(l.p95),
        Some(l.p99),
        Some(l.mean),
    ]
}
fn error_rate(c: &crate::metrics::Counts) -> Option<f64> {
    (c.completed > 0).then(|| c.error_rate())
}

/// Replaces the derived rows of one run. Only redacted snapshot values go in:
/// names, methods, ids, numbers and the sanitized error reasons.
fn write_tables(connection: &Connection, id: &str, s: &Snapshot) -> rusqlite::Result<()> {
    // Run ids are start times in nanoseconds since the Unix epoch.
    let started_nanos = id.parse::<i64>().ok();
    let window = s.measured_elapsed();
    let [p50, p90, p95, p99, mean] = percentiles(&s.latency);
    connection.execute(
        "INSERT OR REPLACE INTO run_metrics (run_id, name, started_at, state, elapsed,
            measured_secs, started, completed, successful, failed, cancelled, error_rate,
            throughput, latency_samples, p50, p90, p95, p99, mean, bytes, warmup_started,
            warmup_completed, dropped_iterations, errors_overflow, slo_verdict, slo_partial)
         VALUES (?1, ?2, datetime(?3 / 1000000000, 'unixepoch'), ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
        params![
            id,
            s.name,
            started_nanos,
            format!("{:?}", s.state),
            s.elapsed,
            window,
            s.counts.started,
            s.counts.completed,
            s.counts.successful,
            s.counts.failed,
            s.counts.cancelled,
            error_rate(&s.counts),
            (window > 0.0).then(|| s.counts.completed as f64 / window),
            s.latency.samples,
            p50,
            p90,
            p95,
            p99,
            mean,
            s.counts.bytes,
            s.warmup_counts.started,
            s.warmup_counts.completed,
            s.counts.dropped_iterations,
            s.errors_overflow,
            s.slo.as_ref().map(|o| format!("{:?}", o.verdict)),
            s.slo.as_ref().map(|o| o.partial),
        ],
    )?;
    for table in ["run_endpoints", "run_slo", "run_errors"] {
        connection.execute(&format!("DELETE FROM {table} WHERE run_id = ?1"), [id])?;
    }
    let mut insert = connection.prepare(
        "INSERT OR REPLACE INTO run_endpoints VALUES
            (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    )?;
    for e in &s.endpoints {
        let [p50, p90, p95, p99, mean] = percentiles(&e.latency);
        insert.execute(params![
            id,
            e.id,
            e.name,
            e.method,
            e.counts.completed,
            e.counts.successful,
            e.counts.failed,
            error_rate(&e.counts),
            e.latency.samples,
            p50,
            p90,
            p95,
            p99,
            mean,
            e.counts.bytes,
        ])?;
    }
    let mut insert =
        connection.prepare("INSERT INTO run_slo VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;
    for (position, r) in s.slo.iter().flat_map(|o| o.results.iter()).enumerate() {
        insert.execute(params![
            id,
            position as i64,
            format!("{:?}", r.threshold.metric),
            r.threshold.request_id,
            r.threshold.value,
            r.threshold.metric.unit(),
            r.actual,
            format!("{:?}", r.status),
        ])?;
    }
    let mut insert =
        connection.prepare("INSERT OR REPLACE INTO run_errors VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for g in &s.errors {
        insert.execute(params![
            id,
            g.request_id,
            g.reason,
            g.count,
            g.first_at,
            g.last_at
        ])?;
    }
    Ok(())
}

/// Opens `dir` in the system file manager (`open` on macOS, `explorer` on
/// Windows, `xdg-open` elsewhere), creating it first. Blocks until the opener
/// returns, so call it off the render thread.
pub fn open_in_file_manager(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let status = Command::new(program)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(format!(
            "No file manager opener ({program}) is available. The data folder is {}",
            dir.display()
        )),
        Err(e) => Err(format!(
            "Could not run {program}: {e}. The data folder is {}",
            dir.display()
        )),
        // Explorer reports exit code 1 even when it opened the folder.
        Ok(status) if status.success() || cfg!(windows) => Ok(()),
        Ok(status) => Err(format!(
            "{program} could not open the data folder ({status}). The data folder is {}",
            dir.display()
        )),
    }
}

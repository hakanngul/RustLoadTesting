use crate::{
    core::TestPlan,
    metrics::{Point, Snapshot},
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn data_directory() -> PathBuf {
    directories::ProjectDirs::from("dev", "RustLoad", "RustLoad")
        .map(|p| p.data_dir().to_owned())
        .unwrap_or_else(|| PathBuf::from(".rustload"))
}
pub fn default_db() -> PathBuf {
    data_directory().join("history.sqlite3")
}
pub struct History {
    connection: Connection,
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
        Ok(Self { connection })
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
        self.connection
            .execute(
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
        Ok(())
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

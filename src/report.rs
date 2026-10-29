use crate::{
    core::TestPlan,
    metrics::{Point, Snapshot},
};
use serde::Serialize;
use std::path::Path;

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn export(
    path: &Path,
    format: &str,
    snapshot: &Snapshot,
    plan: &TestPlan,
    points: &[Point],
) -> Result<(), String> {
    #[derive(Serialize)]
    struct Report<'a> {
        schema_version: u32,
        latency_definition: &'static str,
        rps_definition: &'static str,
        config: TestPlan,
        summary: &'a Snapshot,
        time_series: &'a [Point],
    }
    let report = Report {
        schema_version: 1,
        latency_definition: "HTTP execution through full body read; only fully read responses; milliseconds; excludes pacing and think time",
        rps_definition: "Card: trailing 5s completions/sec. Time series: interval completion counts; elapsed is interval end.",
        config: plan.redacted(),
        summary: snapshot,
        time_series: points,
    };
    let bytes = match format {
        "JSON" => serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        "CSV" => {
            let mut writer = csv::Writer::from_writer(Vec::new());
            writer
                .write_record([
                    "run_id",
                    "state",
                    "request_id",
                    "request_name",
                    "method",
                    "started",
                    "completed",
                    "success",
                    "failed",
                    "cancelled",
                    "latency_samples",
                    "p50_ms",
                    "p90_ms",
                    "p95_ms",
                    "p99_ms",
                ])
                .map_err(|e| e.to_string())?;
            for e in &snapshot.endpoints {
                // Protect spreadsheet users from formula injection.
                let safe_name = if e.name.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                    format!("'{}", e.name)
                } else {
                    e.name.clone()
                };
                writer
                    .write_record([
                        snapshot.run_id.clone(),
                        format!("{:?}", snapshot.state),
                        e.id.to_string(),
                        safe_name,
                        e.method.clone(),
                        e.counts.started.to_string(),
                        e.counts.completed.to_string(),
                        e.counts.successful.to_string(),
                        e.counts.failed.to_string(),
                        e.counts.cancelled.to_string(),
                        e.latency.samples.to_string(),
                        e.latency.p50.to_string(),
                        e.latency.p90.to_string(),
                        e.latency.p95.to_string(),
                        e.latency.p99.to_string(),
                    ])
                    .map_err(|e| e.to_string())?;
            }
            writer.into_inner().map_err(|e| e.to_string())?
        }
        "HTML" => {
            let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
            format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>RustLoad report</title><style>body{{background:#131A20;color:#F3F6FA;font:15px system-ui;max-width:1100px;margin:40px auto;padding:20px}}h1{{color:#FF873E}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;background:#1B2229;padding:24px;border-radius:10px}}p{{line-height:1.7}}</style><h1>{}</h1><p>State: {:?} · Requests: {} · Errors: {:.2}% · P95: {:.2} ms</p><p>Response latency includes full body read. Network failures and cancellations are counted separately. RPS uses completed requests. Partial and cancelled runs retain their actual state.</p><details open><summary>Complete report and time series</summary><pre>{}</pre></details></html>",
                escape(&snapshot.name),snapshot.state,snapshot.counts.completed,snapshot.counts.error_rate(),snapshot.latency.p95,escape(&json)).into_bytes()
        }
        _ => return Err("Unknown report format".into()),
    };
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

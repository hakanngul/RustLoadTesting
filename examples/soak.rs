//! Reproducible local resource check; defaults to 15 minutes, never a remote target.
use rustload::{core::*, engine, fixture, metrics::RunState, report, storage::History};
use std::{path::Path, sync::atomic::Ordering, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seconds = std::env::args()
        .nth(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(900_u64);
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/rustload-soak".into());
    let output = Path::new(&output);
    std::fs::create_dir_all(output)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;
    let server = rt.block_on(fixture::spawn(0))?;
    let mut plan = fixture::demo_plan(server.address);
    plan.load.users = 100;
    plan.load.duration_secs = seconds;
    plan.load.rps_limit = 200;
    plan.load.profile = Profile::RampUp;
    plan.load.ramp_secs = seconds.min(30);
    plan.load.think_ms = 10;
    let db = output.join("history.sqlite3");
    let mut handle = engine::start(&rt, plan.clone(), Path::new("."), db.clone())?;
    let mut system = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let mut writer = csv::Writer::from_path(output.join("resources.csv"))?;
    writer.write_record([
        "elapsed_s",
        "rss_bytes",
        "cpu_percent",
        "completed",
        "series_len",
        "logs_len",
    ])?;
    let final_snapshot = rt.block_on(async {
        loop {
            let snapshot = handle.snapshots.borrow_and_update().clone();
            system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            if let Some(process) = system.process(pid) {
                writer
                    .write_record([
                        snapshot.elapsed.to_string(),
                        process.memory().to_string(),
                        process.cpu_usage().to_string(),
                        snapshot.counts.completed.to_string(),
                        snapshot.series.len().to_string(),
                        snapshot.logs.len().to_string(),
                    ])
                    .expect("resource sample");
                writer.flush().expect("resource flush");
            }
            assert!(snapshot.series.len() <= 900);
            assert!(snapshot.logs.len() <= 5000);
            if snapshot.state.terminal() {
                break snapshot;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    assert_eq!(final_snapshot.state, RunState::Completed);
    assert_eq!(final_snapshot.counts.failed, 0);
    assert_eq!(final_snapshot.counts.in_flight(), 0);
    assert_eq!(
        final_snapshot.counts.completed,
        server.count.load(Ordering::Relaxed)
    );
    let history = History::open(&db)?;
    let points = history.points(&final_snapshot.run_id)?;
    assert_eq!(
        points.iter().map(|p| p.completed).sum::<u64>(),
        final_snapshot.counts.completed
    );
    report::export(
        &output.join("report.json"),
        "JSON",
        &final_snapshot,
        &plan,
        &points,
    )?;
    println!(
        "PASS: {:.2}s · {} real requests · {} series points · {} logs",
        final_snapshot.elapsed,
        final_snapshot.counts.completed,
        points.len(),
        final_snapshot.logs.len()
    );
    server.stop.cancel();
    Ok(())
}

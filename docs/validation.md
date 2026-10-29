# Validation evidence

## Personal widget layouts update

Current suite: **26 passed** (9 core/report, 9 dashboard-layout, 8 real HTTP engine
integration tests), zero failures. Format, strict all-target Clippy and build pass.
Layout tests cover hidden/restored docks, cross-dock moves, global dashboard order,
preserved workbench assignments, independent named pages, serialization of widths
and hidden state, non-reused IDs, normalization, catalog limits and grid packing.

Native Linux/Xvfb checks loaded legacy preferences, hid latency and the right KPI
group, and dragged logs before throughput. Remaining tiles filled the freed space.
Created and saved a `Performance` dashboard-only page, added catalog widgets,
changed visibility while a local HTTP run was Running, and observed completion:
**584 real requests, zero failures**. The preceding local run completed 580 with
zero failures. Save layout flushed immediately; after restarting, both named
pages, active page, hidden widgets and order were retained. Hiding every bottom
widget removed the dock entirely; Restore default widgets reinstated it.

Screenshots are linked in [widgets.md](widgets.md). Layouts contain no test/request
data. The engine and measurement contract are unchanged. macOS UI execution and
remote CI results remain separate from these local checks.

## Studio 2 update

The alternative Studio 2 workbench was added alongside the original Studio and
Monitor. `cargo fmt --all -- --check`, strict all-target Clippy, build and the
existing 17 unit/integration tests pass after the change.

Native Xvfb checks exercised shared request/header editing across both Studio
layouts, simultaneous body/load editing, target authorization, execution staying
in Studio 2, current-run logs and live cards/sparklines, and Monitor receiving the
same run. A local 30-second scenario completed **584 requests with zero failures**.
Reset then Run/Stop produced a separate **Cancelled** run with **22 completed
requests** recorded in SQLite. Studio 2 selection survived an application restart.

Dark, Light and 900×720 compact screenshots are under `docs/screenshots/studio2-*`.
These are real native Linux screenshots; macOS remains unverified. The engine and
measurement definitions were not changed for this layout.

Executed in the managed Debian 13 x86_64 environment with Rust/Cargo 1.99.1.
No external target received load. These are Linux results, not macOS verification.

## Automated checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: **17 passed**, zero failed/ignored (9 core/report tests,
  8 engine integration tests). Crate binary/unit/doc harnesses have zero additional
  tests; they are not counted as functional validation.
- `cargo build --locked`: passed; native executable launched successfully.
- `bash -n scripts/package-macos.sh`: passed syntax check only.

The tests cover real five-method HTTP transmission, query/headers/JSON/Basic Auth,
Bearer extraction, 100 active VUs, ramp, global pacing, per-user CSV/token isolation,
401/429/500, explicit expected errors, malformed JSON, disconnect, timeout, body
cap, full-body latency, cancellation/draining, counts/histograms, bounded rings,
secret omission, auth reference round-trip, SQLite persistence and all exports.
The stop test uses a 30-second timeout and a deliberately slow server; Stop drains
and cancels within the test's seven-second bound, with no in-flight users left.

## Sustained local execution

`cargo run --locked --example soak -- 900 /tmp/rustload-soak`:

- Elapsed 900.24 seconds; 100 VUs, 30-second ramp, 200 starts/sec ceiling.
- **145,640 started/completed/successful**, zero failed or cancelled.
- Fixture count, engine count, persisted interval totals and histogram samples agree.
- RSS peak **15.84 MiB**, final **15.11 MiB** (initial **8.94 MiB**).
- 1,457 retained logs; 693 stored intervals. This earlier run used the initial
  1–1.5-second polling cadence. Sampling was subsequently aligned to second boundaries.
- This process includes both engine and local fixture; it does not include egui.
  The gradual retention/cache allocation is not a claim of constant total RSS or
  a proof against every possible leak. Ring retention is additionally unit-tested
  beyond the configured 900-point and 5,000-log limits.

The final engine revision was rerun with
`cargo run --locked --example soak -- 60 /tmp/rustload-soak-final`:
**60.14 seconds, 9,729 successful real requests, 61 intervals including final partial
bucket, 98 logs**, with all conservation checks passing.

## Native UI exercised using Xvfb / software OpenGL

- Started Local Demo from Studio, authorized the loopback targets and ran the
  three-step scenario. Representative completed run: 584 requests, no failures.
- Dark/Light in both workspaces; theme retained across application restarts.
- Saved a three-request plan from the native file-path dialog, verified its JSON
  and `{{access_token}}` reference, then reopened it using Cmd/Ctrl+O.
- Opened a completed SQLite run after restarting the application; real metrics,
  interval graphs and endpoint breakdown loaded correctly.
- Resized to 900×720: configuration becomes collapsible, charts become tabbed.
- Cmd/Ctrl+Q displayed the unsaved-plan warning; discard-and-close exited the app.
- GUI logs contained no startup error/panic. Current screenshots show real data,
  without mock values: [Studio Dark](screenshots/studio-dark.png),
  [Studio Light](screenshots/studio-light.png), [Monitor Dark](screenshots/monitor-dark.png),
  [Monitor Light](screenshots/monitor-light.png).

## Remaining release checks

- **macOS native launch, manual acceptance and .app packaging are unrun.** A macOS
  machine is required. Signing/notarization is not configured.
- Windows execution and packaging are unrun. CI definitions have been added;
  remote workflow results must be checked on GitHub separately from these local checks.
- 1,000-VU capacity and long-running renderer RSS are not benchmarked. The local
  100-VU tests are functional checks, not hardware-independent throughput promises.
- Flow/Pause/templates, histogram visualization and cookie sessions remain excluded.
  File picking uses explicit paths; HTML export is an offline structured report.
- M0–M3 functionality is implemented and locally checked; M4 remains open for the
  platform/release checks above. This is a runnable MVP, not a signed release.

# Validation evidence

## Multi-stage load profile update

Load profiles gain **Stages** (up to 20). Constant, RampUp and Stages share one
schedule in the engine; each virtual user computes when it is active. Shared
profile selector and stage editor (table plus draggable preview) in Studio and
Studio 2; comparison warnings describe stage profiles.

- `every_profile_is_one_schedule`: constant users start at 0 s; ramp-up users
  start at ramp × i / users exactly as before; a stage profile up, down and up
  again reactivates a user on the next rise (5 s, idle after 15 s, back at
  25 s); a user above the peak never starts.
- `stage_profiles_validate`: empty, zero-length, over 10,000 users, zero peak and
  21 stages rejected; warmup limits use the stage total; older plans load.
- `stage_profile_ramps_users_up_and_down`: real traffic over 1 s→4, 1 s→4,
  1 s→1. Active users reach 4 and fall to at most 2 near the end; zero failures;
  the run lasts the stage total.
- All earlier engine tests, including ramp-up and 100 users, still pass.
- Native Linux/Xvfb: selecting Stages seeded 6 s→10, 18 s→10, 6 s→0 and locked
  users/duration. Dragging the first point gave 9 s→13 (first attempt ran away
  to 1,728 users because the axes rescaled during the drag; bounds now freeze
  while dragging, and the pick uses the press origin). The run of 9 s→13,
  18 s→10, 6 s→0 completed with zero errors and cancellations; the Active
  Virtual Users chart rose to 13 by 9 s, eased to 10 by 27 s and fell to 0 at 33 s.

Suite: **55 passed** (6 compare, 17 core/report, 9 dashboard-layout, 17 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Warmup update

Load Configuration (Studio and Studio 2, one shared control) gains Warmup: None,
By duration or By requests. Requests started during warmup go to separate
counters; the measured window feeds SLO and comparison throughput; charts draw
warmup completions and the warmup end; HTML reports state what was excluded;
comparison warns when warmup differs. The measurement contract in `CLAUDE.md`
and `docs/decisions.md` is updated.

- `duration_warmup_is_excluded_from_every_result_metric`: 1 s warmup at 40 RPS
  over 3 s. Warmup and measured starts in their expected ranges; started =
  completed + cancelled per phase; histogram samples, status totals, endpoint
  starts and bytes contain only measured requests; charted warmup completions
  equal the warmup counter; measured from 1.0 s; SLO throughput uses the
  measured window.
- `request_count_warmup_excludes_exactly_the_first_starts`: exactly 15.
- `unfinished_warmup_leaves_no_measured_data`: no measured requests, SLO
  Inconclusive.
- With warmup ignored in the engine, all three tests fail; with it they pass.
- `warmup_settings_validate_and_older_plans_have_none` and a comparison test for
  the measured window and the warmup warning.
- Native Linux/Xvfb: Local Demo with a 3 s duration warmup. During warmup Monitor
  showed "Warmup in progress (first 3s) · 41 requests excluded so far" with zero
  measured cards; afterwards "excluded 60 requests · measured from 3.0s", the gray
  warmup line for 0–3 s and the dashed warmup-end marker.

Suite: **52 passed** (6 compare, 15 core/report, 9 dashboard-layout, 16 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## cURL / HAR import update

**Import…** (toolbar, shared by Studio and Studio 2) parses pasted
Copy-as-cURL commands or a HAR file, previews requests and warnings, and adds
them to the plan or replaces its requests. Replacing removes request-scoped SLO
thresholds because IDs restart. Also fixes the comparison hover text, which used
an arrow glyph the bundled font lacks.

- `tests/import.rs` (6 tests): Chrome multi-line bash output (query to Params,
  secret token/Authorization/Cookie, Accept-Encoding and Content-Type dropped,
  JSON body); `$'…'` with `\'` and `\u00e9`; double quotes; repeated `-d`;
  Firefox `-X PUT`; attached short options and `--name=value`; `-G`; `-u` and URL
  credentials (percent-decoded); `--json`; `--data-urlencode`; "Copy all" with
  `;` separators, HEAD skipped, `-k`/`-L`/`-F`/unknown options reported, non-curl
  text ignored; the 500-request limit; imported plans validate and never save
  credentials; HAR order, XHR/fetch filter, pseudo-headers, form params, non-HTTP
  and OPTIONS entries.
- `imported_curl_requests_reach_the_server_as_written`: three imported commands
  run against the fixture `/echo`; assertions confirm the custom header, the JSON
  content type and the raw body arrived; zero failures.
- Native Linux/Xvfb: imported a curl command (Replace) against the fixed-port
  fixture: Params, masked Authorization, removed Content-Type/Accept-Encoding,
  `-k` warning; ran it with 0 % errors. In Studio 2 (Light) imported a HAR file
  (Add): document entry skipped, two requests appended. Rechecked the dialog
  label and path-only request names after the fixes.

Suite: **47 passed** (5 compare, 14 core/report, 9 dashboard-layout, 13 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Run comparison update

Monitor's history cards gain a UTC start time, **Baseline** (pin) and **Compare**.
The comparison view shows the overall verdict, setup warnings, a noise band,
a whole-run table, overlaid throughput/P95 charts and per-request changes. No
SQLite schema change; runs load through `History::read` and `History::points`.

- `tests/compare.rs` (5 tests): direction per metric, the noise boundary
  (+2.0 % unchanged, +2.5 % regressed), Improved/Regressed/Mixed/Unchanged/No
  data verdicts, informational data rate, change from a zero baseline,
  endpoint matching by ID with renamed and one-sided requests, setup and state
  warnings, and UTC labels from run IDs (including a leap day).
- `saved_runs_can_be_compared`: two real fixture runs (2 and 4 users) saved to
  SQLite, read back and compared: one matched endpoint, one users warning,
  throughput present on both sides.
- Native Linux/Xvfb: two 5 s Local Demo runs (10 and 20 users). Pinned the first
  as baseline and compared: verdict Mixed, warning "Virtual users differs: 10 vs
  20", whole-run and per-request tables, dashed baseline overlays. Swap reversed
  the roles and the pin; a 25 % noise band left only P99 regressed. Checked in
  Dark, Light and 900×720.

Suite: **40 passed** (5 compare, 14 core/report, 9 dashboard-layout, 12 engine).
Format, strict all-target Clippy and build pass. Linux only; macOS CI separate.

## SLO thresholds update

Plans gain up to 50 thresholds (P50/P90/P95/P99/mean latency, error rate,
throughput; whole run or one request). Every snapshot carries the evaluation;
the final verdict is computed after the last counts and state, persisted with
the summary, and exported in HTML (section and table), JSON and CSV
(`slo_verdict` column).

- `slo_evaluation_uses_bounds_scopes_and_whole_run_throughput`: equality passes,
  upper/lower bounds, endpoint scope, missing endpoint and empty histogram give
  no data, Failed beats Inconclusive, partial flag on cancelled runs.
- `slo_threshold_validation`, `slo_thresholds_round_trip_and_older_plans_still_load`:
  ranges, NaN/infinity, unknown request, the 50 limit; plans saved before this
  change load with no thresholds; stored configurations keep thresholds.
- `slo_verdict_is_final_persisted_and_exported`: real fixture traffic. `/health`
  gives Passed; `/status/500` gives Failed with a 100 % error rate. The verdict
  and thresholds are read back from SQLite; HTML and CSV contain the verdict.
- Native Linux/Xvfb: added two thresholds in the Studio editor (P95 ≤ 300 ms for
  all requests, P99 ≤ 0.1 ms for Checkout). During the Local Demo run Monitor
  showed "currently failing" with per-threshold pass/fail; after 30 s it showed
  "SLO Failed" (P95 0.94 ms pass, Checkout P99 5.94 ms fail). Studio 2 (Light)
  showed the tree items, the shared editor and the verdict in Live Metrics.
  Checked at 900×720 and after reopening the run from history.

Suite: **34 passed** (14 core/report, 9 dashboard-layout, 11 engine). Format,
strict all-target Clippy and build pass. Linux only; macOS CI separate.

## Compatible history and Byte/s update

Persisted metric structs (`Snapshot`, `Point`, `Counts`, `Latency`, `Endpoint`,
`LogEntry`) now default missing fields. New: response body bytes per endpoint and
interval, and a trailing-5s Data/sec rate (cards, endpoint table, reports).

- `history_written_before_byte_metrics_still_loads` writes v0.1-shaped rows into
  SQLite and reads them through `History`. Without the serde defaults it fails
  with ``missing field `bytes` ``; with them it passes.
- `response_bytes_are_counted_per_endpoint_and_interval`: real fixture traffic;
  endpoint bytes equal completed × known body size (11 B and 16 KiB), endpoint
  sums equal the total, and persisted interval bytes sum to the total.
- Native Linux/Xvfb: Local Demo run in Monitor (Dark), Studio 2 (Light) and a
  900×720 window. Monitor showed eight cards on one wide row and four rows of two
  when narrow. Endpoint data matched body size × completions (Checkout 16 B × 61 =
  976 B). The Studio 2 RPS card shows data/sec on its own line.

Suite: **30 passed** (11 core/report, 9 dashboard-layout, 10 engine). Format,
strict all-target Clippy and build pass. Linux only; macOS CI separate.

## Release automation configuration

Added a tag-triggered release workflow for Linux x64, Windows x64, macOS ARM64,
and macOS x64. It validates the tag against Cargo.toml, tests each target before
packaging, requires all four packages before publication, and creates SHA-256
checksums. Native Windows release builds use the GUI subsystem and static MSVC
runtime; macOS packaging derives the app version from Cargo metadata.

Local checks for this change:

- actionlint 1.7.12: both CI and release workflows pass.
- Bash syntax checks: macOS script and embedded workflow Bash steps pass.
- Executed tag validation, version matching, prerelease detection, checksum
  generation, and rejection of missing/extra packages with temporary fixtures.
- Format, strict all-target Clippy, **27 tests**, and optimized release build pass.
- Ran the workflow's Linux packaging step against the real optimized binary.
  Verified archive contents, executable permissions, binary hash, and local shared
  libraries. The extracted app opened a native Xvfb window and exited cleanly.

These checks do not execute the macOS/Windows build jobs or GitHub publication.
Those require the first release workflow run. Native UI acceptance, Developer ID
signing, Apple notarization, and Windows publisher signing remain separate checks.
See [releases](releases.md) for the tag and download procedure.

## Pacing accuracy and metrics lock update

The global gate now advances from its schedule instead of the wake-up time, so
millisecond timer rounding no longer lowers the achieved rate; at most 10 ms of
lag is caught up, and no burst credit accumulates. Response bodies are parsed at
most once, before the shared metrics lock is taken.

Local fixture, 200 VUs, 5 seconds, release build, achieved starts/sec:
limit 500 → before **320**, after **500**; 1,000 → **469** / **1,000**;
2,000 → **654** / **1,998**; 5,000 → **769** / **4,994**. Debug builds matched.
New test `pacing_reaches_limits_above_timer_resolution` (500 RPS, 2 s, expects
900–1,005 starts) fails on the previous gate (626 starts) and passes now.

Current suite: **27 passed** (9 core/report, 9 dashboard-layout, 9 engine).
Format, strict all-target Clippy and build pass. Linux only; macOS CI separate.

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
  machine is required. Developer ID signing/notarization is not configured.
- Windows execution and packaging are unrun. CI definitions have been added;
  remote workflow results must be checked on GitHub separately from these local checks.
- 1,000-VU capacity and long-running renderer RSS are not benchmarked. The local
  100-VU tests are functional checks, not hardware-independent throughput promises.
- Flow/Pause/templates, histogram visualization and cookie sessions remain excluded.
  File picking uses explicit paths; HTML export is an offline structured report.
- M0–M3 functionality is implemented and locally checked; M4 remains open for the
  platform/release checks above. This is a runnable MVP, not a signed release.

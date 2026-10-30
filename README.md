# RustLoad

[English](README.md) · [Türkçe](README_tr.md)

RustLoad is a desktop app for testing HTTP APIs under load. You can build a
sequence of requests, run it with concurrent virtual users, and inspect response
times, throughput, and failures as the test runs.

The app is written in Rust with egui/eframe. The load engine runs on your machine,
with SQLite history and offline reports. You do not need an account, a separate
backend service, or Docker.

[Get started](#get-started) · [Workspaces](#workspaces) ·
[Custom dashboards](#custom-dashboards) · [Measurements](#how-results-are-measured) ·
[Development](#development)

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/screenshots/monitor-light.png">
  <img src="docs/screenshots/monitor-dark.png" alt="RustLoad Monitor showing a completed local test with latency percentiles, request rates, HTTP status counts, endpoint results, and logs">
</picture>

*Monitor after a local demo run. Screenshots in this README were captured from
the native Linux app using the bundled HTTP fixture.*

## What you can do

| Task | Available in RustLoad |
| --- | --- |
| Build an HTTP scenario | Sequential GET, POST, PUT, PATCH, and DELETE requests; headers, query parameters, raw or JSON bodies, Basic and Bearer auth |
| Pass data between requests | Variables, per-user CSV rows, and JSON Pointer extraction for values such as access tokens |
| Check responses | Expected status codes, response-time limits, and JSON Pointer equality assertions |
| Configure load | Concurrent users, constant or linear ramp-up profiles, total duration, think time, timeouts, and a global request-start limit |
| Follow a run | Request rates, latency percentiles, active users, HTTP status counts, endpoint summaries, and filtered logs |
| Arrange your workspace | Two Studio layouts, named widget pages, drag-to-reorder controls, adjustable widths, and Dark/Light themes |
| Keep results | Local SQLite history and HTML, JSON, or CSV exports |

RustLoad is an MVP. Linux execution has been checked; macOS is the product target
and still needs native acceptance testing. See [platform status](#platform-status)
before planning a release.

## Get started

### Download a release

When a version is published, [GitHub Releases](https://github.com/hakanngul/RustLoadTesting/releases)
contains macOS DMGs for Apple Silicon and Intel, a Windows x64 ZIP, a Linux x64
archive, and SHA-256 checksums. See the [release guide](docs/releases.md) for
installation requirements, signing status, and publishing steps.

### Build from source

Install Rust through [rustup](https://rustup.rs), then clone the repository:

```sh
git clone https://github.com/hakanngul/RustLoadTesting.git
cd RustLoadTesting
```

Run the desktop app:

```sh
cargo run --locked
```

`rust-toolchain.toml` selects Rust 1.99.1. The first build downloads that toolchain
and the dependencies pinned in `Cargo.lock`.

You need a native display and OpenGL to open the app. Linux also needs the
X11/Wayland, xkbcommon, and GL runtime libraries. On macOS, install Xcode Command
Line Tools (`xcode-select --install`) for the linker.

### Try the local demo

The demo gives you a working scenario without sending traffic to an external
service.

1. Open RustLoad and click **Local Demo** on a plan with no unsaved changes.
   The app starts a test server on a temporary `127.0.0.1` port.
2. Review the Login, List products, and Checkout requests. Login extracts
   `/access_token`; the products request uses `{{access_token}}`, and checkout
   uses the `product_id` variable.
3. In Studio's load configuration panel, check **I am authorized to test all
   targets in this plan.**, then click **Run Test**.
4. Follow the results in Monitor. The demo uses 10 virtual users for 30 seconds,
   with a limit of 20 HTTP request starts per second.
5. Let it finish, or click **Stop** to keep a partial result. Use **Export Report**
   to save HTML, JSON, or CSV, or reopen a run from the history sidebar.

A saved demo plan still contains the temporary server port, which changes after
restarting the app. For a server with a fixed port, run this in another terminal:

```sh
cargo run --locked --example fixture
```

The fixture listens on `127.0.0.1:8080` until Ctrl-C. Update the plan's request URLs
to use that address. For your own tests, use targets you have permission to test.

## Workspaces

### Studio

Studio is the main scenario editor. Choose a request in the left sidebar, edit
its parameters in the center, and configure users and timing on the right.
Right-click a request to reorder, duplicate, or delete it.

![Studio in Dark theme, with the request body editor and load configuration visible](docs/screenshots/studio-dark.png)

Both Studio layouts share the same test plan and selected request. Running a plan
locks its configuration until the run ends. Starting from Studio opens Monitor;
starting from Studio 2 keeps the workbench open.

### Studio 2

Studio 2 keeps the request tree, HTTP editor, body, and load configuration visible
together. Live metrics sit on the right, with charts and logs below. You can hide
or move those widgets when you want more room for editing.

![Studio 2 during a local run, showing the request tree, Body and Load Configuration panels, live metrics, and bottom charts](docs/screenshots/studio2-dark.png)

On smaller windows, panels collapse or switch to tabs. See the
[Studio 2 guide](docs/studio2.md) for layout behavior and a
[compact-window screenshot](docs/screenshots/studio2-compact.png).

### Monitor

Monitor shows the current run's request rate, response data rate, P50/P95/P99
latency, error rate, and request count. Charts track throughput, interval latency, and active users.
HTTP status counts, endpoint results, and filterable logs help you inspect failures.
SLO threshold results appear under the run title when the plan has any.
Select a previous run in the sidebar to reopen its saved summary.

With warmup set under Load Configuration (**By duration** or **By requests**),
Monitor shows warmup progress, then how many requests it excluded and when
measurement started. The RPS chart draws warmup traffic as a separate gray line
and marks the end of warmup.

To compare two saved runs, pin one with **Baseline** in the sidebar, then choose
**Compare** on another. The comparison shows both runs' throughput and P95 charts
overlaid, a whole-run table with the change per metric, and per-request changes
matched by request ID. Each metric is marked Improved, Regressed or Unchanged;
the run as a whole is Improved, Regressed, Mixed or Unchanged. Changes within the
noise band (default ±2 %, adjustable and remembered) count as unchanged; this is
a tolerance, not a statistical significance test. Runs with different users,
duration, load profile, RPS cap, think time, timeout or request steps are still
compared, with a warning. **Swap** exchanges baseline and candidate.

Dark and Light themes are available throughout the app. You can also view
[Studio in Light](docs/screenshots/studio-light.png),
[Studio 2 in Light](docs/screenshots/studio2-light.png), and
[Monitor in Light](docs/screenshots/monitor-light.png).

## Custom dashboards

In Studio 2, you can save different layouts for different tasks. Keep only a
throughput chart while editing requests, or create a page dedicated to results.

![A saved Performance dashboard with throughput, HTTP status distribution, active users, and logs](docs/screenshots/widgets-dashboard.png)

To arrange the current page:

1. Click **Edit layout** to show drag handles and widget controls.
2. Use **+ Widget** to add a chart, table, log panel, or live metric group.
3. Drag widgets to change their order. Use their menus to set row widths or move
   them between the bottom and right panels in a workbench layout.
4. Click **×** to hide a widget. You can bring it back through **Layouts…**.
   Empty panels disappear, giving the space back to the editors.
5. Click **Save layout** to save the arrangement on this device.

For a separate page, open **Layouts…** and choose **Create blank page** or
**Duplicate current page**. New blank pages use dashboard-only mode, which hides
the request editor. You can rename pages and switch between them during a run.

<details>
<summary>See widget editing controls</summary>

![Dashboard edit mode with drag handles, widget menus, and hide buttons](docs/screenshots/widgets-edit.png)

</details>

The catalog has seven widgets: throughput, latency, logs, active users, HTTP
status distribution, endpoint breakdown, and live metrics. Each reads the current
run. Multiple instances are allowed; log filtering and follow controls are shared
between log widgets in this version.

Layouts support up to 20 named pages and 16 widgets per page. Names must be unique
and contain 1 to 60 characters. Layouts are saved separately from test plans;
saving a plan JSON does not export your dashboard. The
[widget guide](docs/widgets.md) covers sizing, restoring defaults, and persistence.
External data sources, custom queries, plugins, and free pixel positioning are
outside the current implementation.

## Working with scenarios

Use `{{variable}}` in URL paths and query values, headers, request bodies, or auth.
The URL scheme and host must stay literal. A missing variable fails the attempted
step with a local preparation error. Any failed step skips the remaining requests
in that iteration.

Extract response values with RFC 6901 JSON Pointer, such as `/access_token` or
`/items/0/id`. Extraction uses JSON Pointer, not JSONPath. Assertions can check an
expected status, a response-time ceiling, and JSON Pointer equality.

CSV headers become variables local to each virtual user. A user keeps the same
row throughout the run; row reuse is opt-in, and duplicate headers are rejected.
Relative CSV paths resolve against the saved plan's directory. For an unsaved
plan, they resolve against the process working directory.

### Import from cURL or HAR

**Import…** in the toolbar builds requests from your browser. Paste one or more
commands from DevTools → *Copy as cURL (bash)* (or *Copy all as cURL*), or give
the path of a HAR file saved from DevTools → Network → *Save all as HAR*. Preview
the requests and warnings, then add them to the plan or replace its requests.

- URL query parameters move to Params; credentials in a URL become Basic Auth.
- `-H`, `-X`, `-d`/`--data-raw`/`--data-binary`/`--data-urlencode`, `--json`,
  `-u`, `-G`, `-b`, `-A`, `-e` and `--url` are understood. A JSON body with a JSON
  content type becomes a JSON body.
- Connection headers, `Content-Length` and `Accept-Encoding` are removed:
  responses are read uncompressed so sizes and extraction stay exact.
- `-k`, `-L`, `-F`, `-T`, `@file` data and HEAD/OPTIONS requests are not imported;
  the preview lists what was skipped and why.
- HAR import can keep only XHR/fetch requests. Only requests are read; HAR
  responses are never imported. Files up to 50 MiB, at most 500 requests.
- Authorization, Cookie and other credential values are marked secret: they
  work in the current session but are never saved, like any other secret.

### SLO thresholds

Add pass/fail objectives under **Users & plan settings** (Studio) or the **SLO
Thresholds** tree item (Studio 2), for example `P95 latency ≤ 300 ms` or
`Error rate ≤ 1 %`. A threshold applies to the whole run or to one request.
P50/P90/P95/P99/mean latency and error rate are upper bounds; throughput is a
lower bound. Up to 50 thresholds per plan.

Thresholds never stop a run. While it runs, Monitor and the live metrics panels
show whether each one is currently met. When it ends, the run gets a verdict:
**Passed**, **Failed** (any threshold missed) or **Inconclusive** (a threshold
had no data, such as a request that never completed). Cancelled or failed runs
are marked as partial. The verdict is saved with the run history and included
in HTML, JSON and CSV reports.

Plans use schema version 1. Unknown versions are rejected. Open and Save use a
file-path window; Cmd/Ctrl+S saves, Cmd/Ctrl+O opens, and Cmd/Ctrl+Q requests a safe
close. Closing warns about unsaved edits and stops active work before exiting.

### Credentials and saved data

Literal auth secrets and fields marked secret are omitted from saved plans,
history, and reports. Auth fields containing only a variable reference, such as
`{{access_token}}`, keep that reference. Re-enter omitted secret values when you
reopen a plan.

Request bodies stay in the session unless you enable the body-persistence option.
Use secret variables for credentials instead of embedding them in a saved body.
History configuration snapshots omit request bodies and URL query strings.
Response bodies are never logged.

## How results are measured

RustLoad uses a closed-loop model: each virtual user runs the scenario in order,
then waits for the configured think time before the next iteration. Total test
duration includes ramp-up. The **Stages** profile shapes the user count over up
to 20 stages, each moving linearly from the previous count (0 at the start) to
its target; edit them in the table or drag the points of the preview chart.
When the count drops, users finish their current iteration before stopping.

The **Arrival Rate (open)** profile is the open model: scenario iterations start
at a fixed rate (iterations/s) regardless of how fast the target responds, so a
slowing target does not lower the offered load. Each iteration runs on an idle
virtual user from a pool of at most **Max VUs**. When every VU is busy the
iteration is not started and is counted as a **dropped iteration**; dropped
iterations are not requests, so they do not enter the error rate. The global RPS
cap and think time do not apply in this mode.

| Measurement | Meaning |
| --- | --- |
| Successful request | HTTP 200 to 299 by default, or the configured expected status, with all assertions and extractions passing |
| Completed requests | Successful plus failed requests; cancellations and in-flight requests are counted separately |
| Error rate | Failed requests divided by completed requests; cancellations are excluded |
| HTTP error counts | Separate from success criteria: an expected 401 can pass while still appearing in the HTTP status counts |
| Response latency | Time from HTTP execution through the full response-body read, excluding pacing and think time; network failures and timeouts do not enter response histograms |
| Percentile cards | Whole-run HDR Histogram results |
| Percentile charts | Independent interval histograms; percentiles are never averaged across intervals |
| RPS card | Completions per second over a trailing five-second bucketed window, with proportionally weighted boundary buckets |
| Warmup | Optional, by duration or request count. Requests started during warmup are excluded from every result metric and counted separately; total duration includes warmup; whole-run throughput uses the time after warmup |
| Run comparison | Throughput and data are completed requests and bytes divided by elapsed time over each whole run; latency uses whole-run histograms; data rate is informational and does not affect the verdict |
| SLO verdict | Latency thresholds use whole-run histograms; error rate uses completed requests; throughput is completed requests divided by elapsed time over the whole run, not the five-second RPS card. A target equal to the measured value passes |
| Data / sec card | Response body bytes per second over the same trailing window; only fully read responses count, headers are excluded |
| Global RPS limit | Closed-model profiles only. Scheduled HTTP starts across all users; timer lag catch-up is bounded to 10 ms, and idle time does not bank burst credits |
| Dropped iterations | Arrival Rate only: scheduled iterations not started because all Max VUs were busy. Shown in the status panel and HTML report; a comparison warns when a run dropped any |

P50/P90/P95/P99 use HDR histograms recorded in microseconds and displayed in
milliseconds. Empty samples display a placeholder. **Stop** prevents new requests,
interrupts pacing and think-time waits, and gives in-flight requests up to five
seconds to finish before cancelling them. Final metrics are collected after the
user tasks join. Stopping a run marks it **Cancelled**. A run that reaches its
duration is **Completed**; an internal engine failure produces a **Failed** run.
Natural expiry uses the same grace period.

Version 0.1 does not retry requests, follow redirects, or manage cookies. TLS
verification stays enabled. Remote requests use the inherited system HTTP proxy;
literal loopback targets use a local client.

Closed-loop tests can underrepresent latency when a target is overloaded
(coordinated omission); use the Arrival Rate profile when the offered load must
stay fixed. Achieved throughput also depends on the load generator's
CPU, scheduling, connections, and the target's behavior. CPU and RAM readings
describe the RustLoad process, not the target server. Local Demo and the soak
harness run their fixture in that same process, so those readings include it.

## Storage and limits

Run history lives in `history.sqlite3` under RustLoad's platform local data
directory. SQLite receives aggregates during the run and stores the final
summary. If a write fails, the app reports it; the current results remain
available for export. UI preferences use eframe's platform storage.

| Item | Limit |
| --- | --- |
| Live graph retention | 900 points |
| Logs | 5,000 entries, up to 2 KiB per message |
| Response body | 10 MiB by default, configurable up to 100 MiB; oversized responses fail explicitly |
| Plan or CSV input file | 16 MiB |
| Requests in a plan | 500 definitions |
| Configuration bounds | Up to 10,000 virtual users and 100,000 request starts/sec |

The configuration bounds are input validation limits, not benchmark results.
Measure capacity on your own hardware. HTML exports contain escaped structured
results and time series; they do not include an interactive dashboard.

## Development

Run the checks from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

Tests use local HTTP fixtures. For a 15-minute engine soak test:

```sh
cargo run --locked --example soak -- 900 /tmp/rustload-soak
```

The soak harness records RSS, CPU, and counters, checks count conservation, and
writes an offline JSON report. It exercises the engine and local fixture, not the
renderer. Recorded commands and results are in [validation](docs/validation.md).

The engine runs on a separate Tokio runtime and publishes aggregate snapshots to
the UI. Storage writes run on a dedicated worker. See
[architecture](docs/architecture.md) and [implementation decisions](docs/decisions.md)
before changing execution or measurement behavior.

### Platform status

| Platform | Status |
| --- | --- |
| Linux | Build, automated tests, and native UI checks recorded in the development environment |
| macOS | Product target; native launch, manual acceptance, and app packaging still need verification |
| Windows | Execution and packaging have not been verified |

The [release workflow](.github/workflows/release.yml) tests, builds, and packages
all three operating systems when a version tag is pushed. It publishes only after
all four target builds pass. Check the actual Actions results separately from the
local validation record.

On a Mac, `bash scripts/package-macos.sh` builds an ad-hoc signed
`dist/RustLoad.app`. Developer ID signing, Apple notarization, and Windows
publisher certificates are not configured. Native platform UI acceptance remains
a separate check; see [releases](docs/releases.md).

Capacity at 1,000 users and long-running renderer memory use have not been
benchmarked. Flow, Pause, templates, browser emulation, histogram visualization,
cookie sessions, distributed workers, and benchmark comparisons are outside this
implementation.

## Documentation

- [MVP scope](docs/MVP.md)
- [Architecture](docs/architecture.md)
- [Implementation decisions](docs/decisions.md)
- [Studio 2](docs/studio2.md)
- [Widgets and personal layouts](docs/widgets.md)
- [Test plan](docs/test-plan.md)
- [Validation evidence and remaining release checks](docs/validation.md)
- [Release packages and publishing](docs/releases.md)
- [Original product requirements](docs/original-prd.md)

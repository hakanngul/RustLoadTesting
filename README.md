# RustLoad

Native **Rust + egui/eframe** HTTP load testing desktop. Local-first Studio and
Monitor, real Tokio/reqwest traffic, SQLite history, offline reports, Dark/Light.
No browser frontend, backend service, account or Docker required.

**Studio 2** is an alternative layout based on the split-editor reference:
test tree, request form, simultaneous Body/Load editors, right-hand live cards and
bottom graphs/logs. Both Studio layouts share the same plan and engine.
See [Studio 2](docs/studio2.md) for behavior and screenshots.

## Run

Install Rust with [rustup](https://rustup.rs). From this checkout:

```sh
cargo run --locked
```

`rust-toolchain.toml` selects Rust 1.99.1. The first build downloads the toolchain
and crates. Cargo.lock captures resolved dependency versions. The desktop needs a
native display and OpenGL. On Linux, X11/Wayland, xkbcommon and GL runtime libraries
must be installed. On macOS, install Xcode Command Line Tools (`xcode-select
--install`) for the linker. macOS runtime acceptance remains a separate release
check; see [validation](docs/validation.md).

## First real test: Local Demo

1. Open RustLoad and click **Local Demo** on a clean plan. It starts an HTTP
   fixture on an ephemeral **127.0.0.1** port; no internet target is contacted.
2. Review Login → List products → Checkout. Login extracts `/access_token`;
   products uses `{{access_token}}`; checkout uses the `product_id` variable.
3. Confirm target authorization in Studio's right panel, then **Run Test**.
4. Monitor shows real responses, independent interval percentiles, users and logs.
   The defaults are 10 users, 30 seconds, at most 20 starts/sec.
5. Stop produces a partial Cancelled run. Natural expiry produces Completed.
   Select a previous run to reopen its summary, or **Export Report** to HTML/JSON/CSV.

Save Local Demo plans if needed, but its ephemeral port changes after restarting
the application. For a fixed-port fixture use:

```sh
cargo run --locked --example fixture
```

It listens on 127.0.0.1:8080 until Ctrl-C. Replace target URLs accordingly.

## Studio

- Five HTTP methods; query/header tables; raw or JSON body; Basic/Bearer auth.
- Request context menu: reorder, duplicate, delete. Each request has a stable ID.
- Assertions: expected status, response-time ceiling, JSON Pointer equality.
- Extraction: RFC 6901 JSON Pointer (`/access_token`, `/items/0/id`); not JSONPath.
- `{{variable}}` resolves in URLs (path/query), headers, body and auth. Scheme/host
  cannot be variable. Missing variables fail the attempted step with a local
  preparation error. Later steps in that iteration are skipped on any failure.
- Plan & data: plan variables and CSV. CSV headers become user-local variables;
  a row is assigned once per VU. Reuse is opt-in, duplicate headers are rejected.
  Relative paths resolve against the saved plan's directory.
- Save/Open use an explicit file-path window. Cmd/Ctrl+S saves, Cmd/Ctrl+O opens,
  Cmd/Ctrl+Q requests a safe close. Schema version 1
  is supported; unknown versions are rejected instead of silently migrated.
- Close warns about unsaved changes and stops active work before exiting.

Auth literal secrets and secret-marked fields are **not persisted**. Whole auth
references such as `{{access_token}}` are preserved. Re-enter omitted values after
opening a plan. Bodies are session-only unless “Persist body” is explicitly
enabled; use secret variables instead of embedding credentials. History config
snapshots omit request bodies and URL query strings. Responses are never logged.

## Measurement contract

This is a **closed-loop concurrent-user** generator, not an open arrival-rate
generator. Think time follows each scenario iteration. Total duration includes
linear ramp-up. A global gate spaces HTTP starts and does not accumulate burst
credits. Per-user latency does not include time waiting for that gate.

- Default expected status: 200–299; an explicit expected status overrides it.
- Completed = successful + failed. Cancelled and in-flight are separate.
- HTTP 4xx/5xx counts are independent: an expected 401 can be successful.
- Latency starts at HTTP execution and ends after full body read. Network/timeouts
  do not enter response histograms. P50/P90/P95/P99 use HDR histograms in microseconds.
- Card percentiles are whole-run. Chart percentiles are independently calculated
  intervals, never averaged. Empty samples display “—”.
- RPS is completions/sec over a trailing five-second bucketed window (boundary
  buckets are proportionally weighted). RPS limit caps starts, not completions.
- Stop prevents new work, interrupts pacing/think waits, allows five seconds for
  in-flight work, then cancels it. Final metrics are collected after users join.
- No automatic retries, redirects or cookie management in v0.1. TLS verification
  remains enabled. Inherited system HTTP proxy applies to remote requests;
  literal loopback targets use a local client.

## Storage and limits

History is in the platform local data directory under RustLoad (`history.sqlite3`);
UI preferences use eframe's platform storage. SQLite receives aggregates during
the run. Failed persistence is surfaced; current results remain available for export.

Live graphs retain 900 points, logs 5,000 entries (2 KiB each). HTTP response cap is
10 MiB by default, configurable to 100 MiB. Oversize responses fail explicitly.
CSV and plan input files are capped at 16 MiB; a plan supports 500 request definitions.
The UI validates at most 10,000 users and 100,000 RPS; those are configuration
guards, **not verified capacity promises**. Benchmark on your own hardware.

CPU/RAM displays describe the load-generator process, not the target. Closed-loop
tests can underrepresent latency under overload (coordinated omission). Scheduling,
connections, CPU and target behavior can all limit achieved throughput.
Local Demo and the soak harness host the fixture in that same process, so their
resource measurements also include the fixture server's overhead.

## Development and validation

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
cargo run --locked --example soak -- 900 /tmp/rustload-soak
```

Tests use local fixtures only. The soak example records RSS/CPU and counters,
checks count conservation and writes an offline JSON report. It does not measure
the renderer's memory; use a native UI run for that.

Architecture and scope: [architecture](docs/architecture.md),
[decisions](docs/decisions.md), [MVP](docs/MVP.md), [test plan](docs/test-plan.md).

## Packaging and remaining platform checks

On a Mac, `bash scripts/package-macos.sh` builds an **unsigned** `dist/RustLoad.app`.
The script must be run on macOS and has not been executed in the Linux development
environment. Signing/notarization and distribution are not included. CI defines
Linux/macOS build and test jobs; a workflow file is not evidence a remote job ran.

Flow, Pause, templates, browser emulation, histogram visualization, cookies,
distributed workers and benchmark comparisons are outside this implementation.
HTML reports contain complete escaped structured data, not an interactive dashboard.

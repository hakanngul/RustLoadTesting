# RustLoad v0.1 scope

The supplied PRD plus the agreed measurement and lifecycle decisions define this
implementation. This file describes the concrete delivery, not a claim of testing.

## Studio
Native Rust/egui desktop, Dark and Light, editable sequential requests (five HTTP
methods), headers/query, JSON/raw bodies, Basic/Bearer auth, assertions, RFC 6901
extractors, plan/user variables and per-user CSV rows. New/open/save/save-as use
explicit paths. Request context menus duplicate, reorder and delete. Running plans
are immutable. Authorization acknowledgement is required before Run.

An alternative **Studio 2** tab provides the reference split-editor workbench with
an expandable plan tree, simultaneous Body/Load panels, right-hand live metrics
and bottom throughput/latency/logs. It shares the plan and engine with Studio;
see [Studio 2](studio2.md).

## Engine
Tokio closed-loop users, constant/linear ramp, duration including ramp, fixed
iteration think time, globally spaced HTTP starts, individual request timeout,
body cap and bounded stop. Cookie handling, redirects, retries, scripting and
distributed execution are excluded. A failed step skips the rest of that iteration.

## Monitor
Real aggregate KPIs, P50/P90/P95/P99, rolling RPS and latency/users time series,
status bars, endpoint summaries and filtered virtualized logs. SQLite persists
time series incrementally and final summaries. History and offline HTML/JSON/CSV
export are supported. CPU and RSS are for this local process.

## Explicit boundaries
No Flow, Pause, templates, histogram visualization, comparison deltas, search API,
remote workers or account system. No signing/notarization in this environment.
The bundled egui font is the system-font alternative; no Inter asset is included.
macOS release requires a real macOS build and manual UI acceptance.

## Acceptance evidence
See validation.md for actual commands/results. Source is not proof of successful
execution; screenshots demonstrate the native Linux UI, not macOS support.

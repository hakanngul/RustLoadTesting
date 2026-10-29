# RustLoad architecture

RustLoad is a local desktop application. Studio edits a versioned TestPlan; Run
validates and freezes it. Monitor reads immutable aggregate snapshots. There is
one active run. The HTTP engine never depends on egui or the UI event loop.

Modules: core (model, validation, variables, CSV), engine (Tokio users and pacing),
metrics (histograms, counters, bounded samples), storage (SQLite history), report
(offline HTML/JSON/CSV), theme and ui (eframe).

The desktop owns a separate Tokio runtime. A coordinator joins user tasks and
publishes snapshots via watch, so a slow renderer cannot block HTTP workers.
Metrics are recorded before publishing; dropping a UI snapshot cannot lose counts.
One-second aggregates are emitted to a bounded persistence channel; a dedicated
storage thread writes them to SQLite. Final summaries are also persisted.

Safety: explicit target acknowledgement; HTTP(S) only; no redirects, retries or
cookie jar; bounded body reads; shared pacing; cancellable waits; graceful stop.
Secret values are session-only and are removed from plans and result snapshots.

Validation targets: local fixture scenario, 100 VU, pacing, cancellation, metrics,
serialization, history and exports. Linux results do not establish macOS support.

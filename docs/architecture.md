# RustLoad architecture

RustLoad is a local desktop application. Studio edits a versioned TestPlan; Run
validates and freezes it. Monitor reads immutable aggregate snapshots. There is
one active run. The HTTP engine never depends on egui or the UI event loop.

Modules: core (model, validation, variables, CSV), engine (Tokio users and pacing),
metrics (histograms, counters, bounded samples), storage (SQLite history), report
(offline HTML/JSON/CSV), import (cURL/HAR/OpenAPI parsers; `import/refs.rs`
bundles local `$ref` files), theme and ui (eframe).

Import is pure parsing with no network access: the OpenAPI importer reads only
the document and local files inside its folder, then the dialog builds
requests for the chosen operations and server without re-reading them.

The desktop owns a separate Tokio runtime. A coordinator joins user tasks and
publishes snapshots via watch, so a slow renderer cannot block HTTP workers.
Metrics are recorded before publishing; dropping a UI snapshot cannot lose counts.
One-second aggregates are emitted to a bounded persistence channel; a dedicated
storage thread writes them to SQLite. Final summaries are also persisted.

Personal dashboards are a separate serializable model (`dashboard.rs`), stored in
eframe preferences rather than TestPlan or SQLite run configuration. Stable page
and widget IDs isolate plot state. Studio 2 renders the same current-run widgets
in optional docks or a dashboard-only grid. Layout edits never mutate engine state.

Safety: explicit target acknowledgement; HTTP(S) only; no redirects, retries or
cookie jar; bounded body reads; shared pacing; cancellable waits; graceful stop.
Secret values are session-only and are removed from plans and result snapshots;
a secret that is exactly one `{{variable}}` reference is kept as that reference.

Validation targets: local fixture scenario, 100 VU, pacing, cancellation, metrics,
serialization, history and exports. Linux results do not establish macOS support.

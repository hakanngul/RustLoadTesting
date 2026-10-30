# MVP decisions

- Single package with separate modules; avoid premature workspace overhead.
- English product UI; native egui/eframe only. Studio and Monitor, no Flow/Pause.
- Rust 1.99.1; exact resolved dependencies captured in Cargo.lock.
- Closed loop users; duration includes ramp-up. Linear ramp or immediate constant
  users. Think time is after each scenario iteration. CSV row is fixed per VU;
  reuse is opt-in. Missing variables and columns are errors.
- Default expected status 200–299; an explicit expected status replaces that
  expectation. All assertions must pass. HTTP error counts are orthogonal.
- Started = completed + cancelled + in-flight. Completed = success + failure.
  User cancellations are excluded from error-rate denominators.
- Latency covers HTTP execution including full body read, excluding pacing and
  think time. Only fully read HTTP responses enter latency histograms; network and
  timeout durations are not mixed into response latency percentiles.
- RPS cards use trailing five seconds; percentile cards use whole-run histograms.
  Data/sec uses the same window and counts body bytes of fully read responses
  only (no headers; responses are not decompressed, so this is the wire body).
- SLO thresholds produce a verdict and never stop a run. Latency and error rate
  are upper bounds, throughput a lower bound; equality passes. Values come from
  whole-run histograms and counters, throughput as completed / elapsed. Any
  failed threshold fails the run; otherwise missing data is Inconclusive.
  Non-Completed runs carry a partial flag. Thresholds live in the plan (and its
  stored configuration); the verdict is persisted with the run summary.
- Run comparison is a pure function over two saved summaries and their stored
  configurations. Whole-run throughput/data use completed requests/bytes over
  elapsed time; latency uses whole-run histograms. A relative noise band
  (default ±2 %, a UI preference) marks small changes Unchanged; it is not a
  significance test. Data rate is informational. Endpoints match by request ID.
  Differing setups or non-Completed runs produce warnings, not refusals.
- cURL/HAR import is a pure parser (own bash tokenizer: '…', "…", $'…',
  backslash continuations; no new dependency). It never executes commands,
  reads referenced files or imports HAR responses. Query parameters become
  Params, URL credentials Basic Auth; connection headers, Content-Length and
  Accept-Encoding are dropped (responses are not decompressed). Credential
  headers stay session-only secrets. Unsupported options are listed, not
  guessed.
- OpenAPI import reads JSON only (OpenAPI 3.x, Swagger 2.0) with serde_json, so
  no new dependency; YAML would need one and waits for a user decision. Local
  `$ref`s are followed (depth 16). Operations come in path order (serde_json
  maps are sorted). Values come only from the document's examples/defaults,
  never invented; missing ones are left empty and listed. Path parameters
  become plan variables (names sanitized to `[A-Za-z0-9_]`), so the URL keeps a
  fixed scheme and host. Bearer/OAuth becomes `{{access_token}}`, which the
  secret-safe serializer keeps as a reference while its value stays session-only.
  Existing plan variables of the same name are never overwritten.
- Warmup is None, Duration{secs} or Requests{count}. A request is classified by
  its start: started during warmup means excluded from every result metric
  (counts, histograms, statuses, endpoints, bytes) and counted in separate
  warmup counters, so started = completed + cancelled + in-flight holds per
  phase. Duration includes warmup. The measured window starts at `secs`, or at
  the first non-warmup start for request warmup; SLO and comparison throughput
  divide by that window. An unfinished request warmup leaves no measured data
  (Inconclusive). Charts show warmup completions separately and mark its end.
- Load profiles are one schedule: Constant, RampUp and Stages all become
  piecewise-linear (time, users) points. User `i` runs while the scheduled count
  exceeds `i`, computing its activation time analytically (no polling); RampUp
  keeps its previous start times. Users join or leave only between iterations, so
  ramp-down lets the current iteration finish. Stages derive the peak users and
  total duration; at most 20 stages. The editor offers a table and draggable
  points; axis bounds freeze during a drag.
- Failures are grouped by (request, reason) with the first reason in a fixed
  order: transport failure, unexpected status, latency limit, JSON assertion,
  extraction. One group per failure, so group counts plus the overflow equal the
  measured failed count; warmup failures are not grouped. Transport causes come
  from reqwest/io error kinds (refused; reset/aborted/broken pipe/EOF; connect;
  body read; other), never the error's text, because it can contain the URL.
  JSON pointers are plan text and are clipped to 80 characters. At most 100
  groups; later new reasons only increment an overflow counter.
- The latency distribution widget reads `Snapshot.latency_distribution`: the
  whole-run (measured) histogram in at most 40 log-spaced buckets between its
  minimum and maximum, built from recorded values so counts sum to the sample
  count. Log spacing keeps both the bulk and the tail visible; P50/P95/P99 come
  from the same histogram, never from the bars.
- Studio, Studio 2 and Monitor share one widget/dashboard implementation,
  parameterized by `dashboard::Workspace` (allowed docks, dashboard-only
  support, default page, data source). Each workspace has an independent page
  set: Studio 2 keeps the historical `dashboards` preference key, Monitor and
  Studio add `monitor_dashboards` / `studio_dashboards` with serde defaults equal
  to their default pages, so older preferences load unchanged. Monitor adds a
  `Main` dock (the grid under its fixed run header); Studio offers only Bottom
  because its Load Configuration panel occupies the right. Monitor widgets read
  the snapshot Monitor shows (live or history); Studio and Studio 2 widgets read
  the current run. Two kinds were added to rebuild Monitor: `KpiCards` and
  `RequestsPerSecond` (Total/Success/Failed). Restoring defaults assigns fresh
  widget IDs so stale plot state never attaches to restored widgets.
- Arrival Rate is the open model. A scheduler starts iteration `k` at
  `k / rate` seconds (fixed slots from t=0, so a late wake-up catches up instead
  of lowering the rate) on an idle VU from a pool of `users`. With no idle VU
  the iteration is dropped and counted (`Counts.dropped_iterations`, warmup
  separately); it is not a request, so the request identities and error rate are
  unchanged. Each VU keeps its CSV row and extracted variables across its
  iterations, like closed-model users. The RPS gate and think time are skipped
  because the schedule already paces starts.
- Arrival rate stages (`LoadConfig.rate_stages`, at most 20): empty means the
  constant `arrival_rate`, so older plans are unchanged. Otherwise
  `arrival_rate` is the start rate (may be 0) and each stage moves linearly to
  its `target_rate`; the run length is the stage total. Iteration `k` starts at
  `LoadConfig::arrival_start(k)`, the first `t` where the integrated rate
  reaches `k`, solved analytically per linear segment (a stable quadratic root,
  exact for slope 0; zero-rate segments start nothing). A constant rate still
  gives exactly `k / rate`. Validation: 1–20 stages, each at least 1 s, rates
  0–100,000 and a peak above 0. The engine copies `rate_schedule()` (at most 21
  points) into `Snapshot.target_rate` for the dashed Target it/s chart line.
  User stages and rate stages share one generic stage editor (table and
  draggable preview) in Studio and Studio 2.
- Persisted metric structs default missing fields, so history written by older
  versions stays readable when fields are added.
  Charts use independent one-second histogram buckets, never averaged percentiles.
- A single global scheduled gate spaces starts; no accumulated burst credits.
  Slots advance from the schedule, not the wake-up time, so millisecond timer
  rounding does not lower the rate; at most 10 ms of lag is caught up.
  Redirects and automatic retries are disabled. No cookie support in v0.1.
- Stop forbids new requests immediately; in-flight work gets up to five seconds,
  then cancellation. Natural expiry uses the same grace period. User stop means
  Cancelled, natural expiry means Completed; engine failures mean Failed.
- JSON extraction uses RFC 6901 JSON Pointer, e.g. /access_token. No scripting.
- Response body cap 10 MiB (decoded bytes), log ring 5,000 entries, each message
  at most 2 KiB, response samples disabled by default. Endpoint identity is the
  request definition ID, never a dynamic URL. Chart retention is 900 seconds.
- Auth secrets and explicitly secret variables/header/query values are never
  serialized. Authorization/Cookie and common credential keys are always secret.
  Missing persisted secrets must be re-entered. No response bodies in logs.
- Plan-relative CSV paths resolve against the saved plan directory; unsaved plans
  resolve against the process working directory. Portable reports use redacted
  immutable configuration and escaped offline HTML.
- macOS is the product target; manual macOS execution is a release prerequisite.

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
  guessed. OpenAPI import is a later step.
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
- The latency distribution widget reads `Snapshot.latency_distribution`: the
  whole-run (measured) histogram in at most 40 log-spaced buckets between its
  minimum and maximum, built from recorded values so counts sum to the sample
  count. Log spacing keeps both the bulk and the tail visible; P50/P95/P99 come
  from the same histogram, never from the bars.
- Arrival Rate is the open model. A scheduler starts iteration `k` at
  `k / rate` seconds (fixed slots from t=0, so a late wake-up catches up instead
  of lowering the rate) on an idle VU from a pool of `users`. With no idle VU
  the iteration is dropped and counted (`Counts.dropped_iterations`, warmup
  separately); it is not a request, so the request identities and error rate are
  unchanged. Each VU keeps its CSV row and extracted variables across its
  iterations, like closed-model users. The RPS gate and think time are skipped
  because the schedule already paces starts. A constant rate only for now;
  rate stages are not implemented.
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

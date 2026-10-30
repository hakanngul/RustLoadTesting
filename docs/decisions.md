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
- OpenAPI import reads JSON (serde_json) and YAML (yaml-rust2, see below) for
  OpenAPI 3.x and Swagger 2.0. Local `$ref`s are followed (depth 16). Operations come in document order: paths as
  written, then methods as written within a path item (not a fixed method
  order), because that is the order a reader sees in the file. Parameter values
  come only from the document's examples/defaults; missing ones are left empty
  and listed. Path parameters become plan variables (names sanitized to
  `[A-Za-z0-9_]`), so the URL keeps a fixed scheme and host. Bearer/OAuth
  becomes `{{access_token}}`, which the secret-safe serializer keeps as a
  reference while its value stays session-only. Existing plan variables of the
  same name are never overwritten.
- The OpenAPI document is parsed once (`parse_openapi`) into candidate
  operations with stable keys "METHOD path"; the dialog keeps a key set and
  `OpenApiSpec::build` turns the selection into requests, variables and
  warnings (only for the selected operations) without re-parsing. All
  operations start selected; Import is blocked while the selection would pass
  the 500-request plan limit, and `build` itself stops at 500.
- Document order comes from serde_json's `preserve_order` feature (maps keep
  insertion order; no new crate: `indexmap` and `foldhash` were already in
  Cargo.lock). Audit of every `serde_json::Value`/`Map` use: plans, history,
  reports and preferences serialize Rust structs, so their output is
  unchanged; object equality (JSON assertions) ignores key order; JSON
  pointers do not depend on order. Visible effects: OpenAPI operations, body
  properties, `examples` ("the first example"), the first JSON media type,
  security schemes within a requirement and `api_key_N` numbering now follow
  the document; an extractor that captures a whole JSON object stores it in
  the response's key order instead of sorted order.
- External `$ref`s are bundled before parsing operations (`import/refs.rs`):
  each referenced local file is read once, embedded in the document under
  `x-rustload-files`, and every reference becomes a local `#/…` pointer, so the
  sampler, cycle detection and resolution work unchanged. References resolve
  relative to the file that contains them. Rules: no network ever (URLs are
  listed, other schemes refused); paths are canonicalized and must stay inside
  the document's canonical folder (refuses `..` escapes, absolute paths and
  symlinks pointing outside); 10 MiB per file (checked before and while
  reading); at most 50 files, taken in document order; JSON and YAML files
  follow the same rules, whatever the root document is; files are cached by
  canonical path, so mutual references cannot loop, and schema cycles across
  files stop like local ones. Examples/defaults/enum/const values are data and
  are not rewritten. Messages name the reference, never file contents. A
  reference back to the root document by file name loads it as a separate
  copy (the importer is given the folder, not the root file name). Parsing
  goes through one `parse_document` function for the root and every file.
  `parse_openapi` (bytes only) lists external references instead of reading
  them; `parse_openapi_in`/`load_openapi` take the folder.
- YAML OpenAPI documents (user-approved new dependency): `yaml-rust2` 0.13
  (MIT OR Apache-2.0, compatible with the project's MIT license; pure Rust, no
  `unsafe` in the crate itself; default features off, so no `encoding_rs`).
  It adds `arraydeque` 0.5.1 and `hashlink` 0.12.2 to Cargo.lock (both MIT OR
  Apache-2.0, pure Rust collections with internal `unsafe`). Chosen because
  it is maintained (releases through 2026), widely used and exposes a
  pull-based event parser, so the conversion, alias expansion and limits stay
  in our code. Not chosen: `serde_yaml` (archived) and `serde_yml` (excluded
  by the user); `serde_yaml_ng` (last release May 2024) and `serde_norway`
  (last release December 2024), both serde front ends on `unsafe-libyaml`, a
  machine translation of C libyaml; `saphyr-parser` (same authors, but its
  API was changing weekly and the newest releases were days old).
  Detection is by content, not file name, so the bytes-only entry point
  works: after an optional UTF-8 BOM, `{` or `[` means JSON (if serde_json
  fails, YAML is tried because flow-style YAML starts the same way; the JSON
  error is reported); anything else is YAML, which must be UTF-8.
  Conversion (`import/yaml.rs`) builds `serde_json::Value` in mapping order:
  YAML 1.2 core schema for plain scalars (`yes`/`on` stay text, integers past
  64 bits become floats as in JSON, `.inf`/`.nan` and overflowing floats stay
  text because JSON cannot hold them); quoted and block scalars are text;
  keys are the written text (`200:` → `"200"`), a collection as key is
  refused; `<<` merge keys apply (written keys win, then earlier sources);
  `!!str`/`!` force text, `!!null`/`!!bool`/`!!int`/`!!float` apply when the
  text fits, other tags are ignored; duplicate keys keep the last value like
  serde_json; exactly one document. Safety: the document is parsed twice by
  the pull parser (never its recursive `load`): pass 1 checks syntax, the
  document count and depth (128, as serde_json) and records which anchors
  aliases use; pass 2 builds the value and keeps copies only of those
  anchors. Every anchor copy and alias expansion is charged (48 bytes per
  value plus text bytes) before it is cloned, against 64 MiB per document,
  so alias bombs fail fast with a short message; aliases cannot push nesting
  past 128 either. yaml-rust2's scanner also limits flow nesting. The 50 MiB
  root and 10 MiB/50-file reference limits are unchanged. Errors carry the
  parser's reason (clipped to 80 characters; at most one quoted character)
  and line/column, never content.
- Servers are part of the parsed spec (`OpenApiServer` with variables and
  their `enum` options; Swagger 2.0 becomes one `{scheme}://host/basePath`
  server whose `scheme` variable lists `schemes`). Operations keep their URL
  path; `OpenApiSpec::select_server` derives the base URL from a
  `ServerChoice` or the Base URL override (which wins when not blank) and
  `build` prepends it, so the dialog changes servers without re-reading files.
  A default missing from its `enum` is offered first. Validation is unchanged
  (absolute http(s), host, no credentials, query or unresolved variables);
  loading succeeds without a usable server, and Import stays disabled until a
  base URL is valid.
- JSON bodies: document values first; `allOf` merges members' properties,
  `required` and object examples; `oneOf`/`anyOf` take the first option.
  Otherwise type placeholders (required properties, or all when none is
  required; "" or a fixed value for date-time/date/uuid/email; 0 or `minimum`;
  false; one-item arrays). Depth is bounded (8) and a `$ref` already on the
  current path stops recursion. Placeholders are not business data; the
  preview counts the bodies that use them. Parameters never get placeholders,
  so missing path values stay visible.
- Security requirements are alternatives: the first one RustLoad can fill
  completely wins, then an anonymous `{}` one, else the first one as far as
  possible. apiKey in a header or query becomes that field with `{{api_key}}`
  (`api_key_2`… in the order the schemes are defined) and a secret plan
  variable; apiKey in a cookie is only listed because there is no cookie
  support.
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
  One rule (`is_variable_reference`) applies to auth and secret fields alike: a
  value that is exactly one `{{name}}` reference (name of `[A-Za-z0-9_]`) is
  kept, because it names a variable rather than holding a credential; literal
  values and mixed text (`Bearer {{token}}`) are dropped. The referenced secret
  variable's own value is still never saved. Missing persisted secrets must be
  re-entered. No response bodies in logs.
- Plan-relative CSV paths resolve against the saved plan directory; unsaved plans
  resolve against the process working directory. Portable reports use redacted
  immutable configuration and escaped offline HTML.
- macOS is the product target; manual macOS execution is a release prerequisite.

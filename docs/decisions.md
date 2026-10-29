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
  Charts use independent one-second histogram buckets, never averaged percentiles.
- A single global scheduled gate spaces starts; no accumulated burst credits.
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

# Verification plan

Required: cargo fmt --check; cargo clippy --all-targets -- -D warnings;
cargo test; cargo build. Unit checks cover validation, secret persistence,
variables, CSV assignment, assertion evaluation and histogram algebra.

Integration: localhost fixture endpoints for login/token/protected products and
checkout; 100 users, paced starts, ramp-up, full body timing, 401/429/500, malformed
JSON, timeout, bounded responses, cancellation and graceful draining. Verify real
server counts against engine totals, SQLite history/time series and all exports.

Desktop: create/save/reopen a plan, edit all request fields, run local demo, switch
Studio/Monitor, verify counts and dark/light modes, resize, stop, export and reopen
history. Verify unsaved close protection and secret warnings. Native macOS checks
are required separately; do not infer them from Linux builds.

Resource check: 15-minute local run with rolling graphs/logs; inspect RSS and run
history. No internet load tests. 1,000 users is an architecture target only, not
a verified throughput promise. Closed-loop tests exhibit coordinated omission;
CPU/RAM measures refer to the load generator, not the target.

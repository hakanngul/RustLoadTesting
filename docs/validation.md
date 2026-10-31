# Validation evidence

## Queryable history database, data folder and history export

`history.sqlite3` now has a schema version (`PRAGMA user_version` = 1) and four
derived tables (`run_metrics`, `run_endpoints`, `run_slo`, `run_errors`) next
to the unchanged `runs`/`points` JSON. Version 0 files are upgraded and
backfilled on open; `History::finish` writes the rows in the same transaction.
Test Runs gains **Open data folder** and **Export history…** (`VACUUM INTO`,
background thread, refuses existing files).

- Tests (`tests/history.rs`, real localhost fixture runs with warmup, two
  requests (one failing), a secret header and query value and two SLOs):
  `finished_run_fills_query_tables_and_refinish_replaces_them`,
  `version_zero_database_migrates_backfills_once_and_stays_readable` (exact old
  DDL, plus a NULL and a damaged summary that are skipped; a hand-edited value
  survives the second open), `runs_finished_by_an_older_app_are_filled_on_next_open`,
  `failed_migration_keeps_history_usable_and_reports_it` (foreign `run_metrics`
  table: rollback, user_version stays 0, list/read/finish still work, short
  reason), `export_writes_a_readable_copy_and_refuses_existing_files`. Each run
  checks counts, P95, measured-window throughput, warmup kept apart, endpoint,
  SLO and error rows against the Snapshot, and that no secret, `api_key`,
  `http`, `127.0.0.1` or `?` reaches the new tables.
- Native Linux/Xvfb (debug build, isolated home, Local Demo traffic only): a
  real run's DB downgraded to the exact v0 layout opened with its run listed
  and its summary reopening; Python's `sqlite3` then showed user_version 1 and
  filled `run_metrics`/`run_endpoints`. A second Local Demo run added its rows
  at finish. Export history wrote a copy (user_version 1, both runs, both
  example queries from the user guide return rows); exporting again to the
  same name was refused in red with the dialog kept open. Open data folder:
  the container has no `xdg-open`, so the status line showed "No file manager
  opener (xdg-open) is available. The data folder is …" and the app kept
  running; actually opening a file manager is not verified here. A DB with a
  conflicting `run_metrics` table showed the upgrade notice (found and fixed:
  the first version printed the whole DDL that rusqlite appends to the error),
  listed both runs and was left at user_version 0. Buttons checked at 230 and
  170 px sidebar width, Dark and Light, Monitor and Report.

Suite: **109 passed** (103 + 6). Format, strict all-target Clippy and build
pass. Linux only; macOS `open` and Windows `explorer` paths are untested.

## Screenshot refresh after the header, chart and Report changes

Every documentation screenshot was recaptured from the current build so it shows
the status line in the header, the **k6 Demo** button, series chips next to
chart titles and the **Report** tab. Each image keeps its file name, pixel size
and subject; the captions still match and were not changed.

- Recaptured: `docs/screenshots/` studio-dark/light, studio2-dark/light
  (1520×1040), studio2-compact (900×720), monitor-dark/light,
  widgets-dashboard/edit/focus/layouts (1520×1040); `docs/screenshots/guide/`
  01–03 (1440×900), 04-arrival-rate-stages (285×712 crop),
  05-import-openapi (696×702 crop), 06-layout-manager (666×598 crop).
- New: `report-dark.png` and `report-light.png` (1520×1040) in the README
  Report sections (Light as a link, like the other workspaces) and
  `guide/07-report.png` (1440×900) in the user guides' Report sections.
- Data: native Linux/Xvfb debug build, a fresh isolated home per scenario and
  only **Local Demo** traffic to its 127.0.0.1 server. The guide 02/03/07 and
  Report shots come from one 30 s run with a 5 s warmup and four SLOs (P95 ≤ 50
  ms, error rate ≤ 1 %, throughput ≥ 10 req/s, P99 ≤ 100 ms), all passing. The
  k6 Demo button is only visible; its plan was never run. The OpenAPI import
  used a local file whose server is `http://127.0.0.1:8080`. No secrets appear.
- Checks: old and new sizes compared before overwriting (all equal); PNGs
  re-saved losslessly; every `screenshots/…png` link in README*.md and docs/*.md
  resolves (link checker: 0 problems); `git diff --check` clean. No code
  changes.

## Report view

A **Report** tab next to Monitor shows the run Monitor has open as a fixed,
read-only summary built from the approved mock-up (verdict band, four key
cards, secondary strip, linked timeline with warmup band and SLO line,
failures, HTTP status, slowest request, sortable endpoint table, folded logs).
Monitor's code path and pages are unchanged; Run from Studio still lands on
Monitor. `Preferences.report` (serde default `false`) remembers the choice.

- Test: `old_preferences_keep_studio2_layouts_and_gain_workspace_defaults` also checks
  that older preferences load with Report off and that Report survives a save.
- Native Linux/Xvfb: a live k6 Demo run (RUNNING band, SLOs currently
  passing), the completed run from history (PASSED, 4 SLO chips, P95 SLO line,
  warmup band, aligned axes), a Local Demo run without SLOs (COMPLETED, no SLO
  line), Dark and Light, 1520×1040 and 900×720 (cards 2×2, chips wrap, table
  scrolls inside), endpoint sorting, log drawer, zoom kept per run.
- Found and fixed while checking: framed SLO chips did not wrap and widened
  the page at 900 px; egui_plot's power-of-ten grid left a 0–900 ms axis with
  one label (1-2-5 steps now); a zoom carried over to another run's report.

Suite: **103 passed**. Format, strict all-target Clippy and build pass. Linux only.

## Chart reset, series toggles, widget expand and card fixes

Reported from a real Monitor screen: a panned chart could not be brought back,
legends covered the newest data, and "20.3 KiB/s" wrapped one character per
line in its KPI card.

- Charts (Requests per Second, Throughput, Response Time, Active Users,
  Latency Distribution and the comparison charts) list their series next to
  the title instead of an in-plot legend; clicking a name or dot hides or shows
  it. **Reset zoom** appears after a pan or zoom; double-click still resets.
- The Warmup series stops after its last completions instead of drawing zeros
  over the rest of the run.
- **Expand** (top-right of a tile on hover, not in Edit layout) opens the widget
  in a centred modal at 90 % × 85 % of the window, live; Close, Esc or a click
  outside closes it. Charts and logs fill it; cards and tables keep their
  height. The button lives in its own layer so a widget's scroll area cannot
  take its click, and hides when a window or menu covers the tile.
- KPI and Studio 2 card values shrink to fit (down to 16 px, then ellipsis)
  instead of wrapping.

Native Linux/Xvfb with a real Local Demo run: zoomed Requests per Second to an
empty 70–80 range, **Reset zoom** restored it; Expand on Response Time, KPI
cards and Live Logs; Esc and outside-click close; Studio 2 right dock; Dark and
Light. Suite: 103 passed; format, strict all-target Clippy and build pass.

## Status line moved into the header

The status message and the load-generator CPU/RAM line moved from a bottom
panel to the header, right under **Local Engine · v0.1.0** and the theme
switch. The header grows by about 15 px and the ~50 px bottom strip is gone, so
every workspace gets about 35 px more height. Long messages are cut with an
ellipsis and show in full on hover; the capacity warning joins the CPU/RAM line
in the warning colour. Message tones are unchanged. Because the header is drawn
before the rest of the frame, a message changed later in the frame triggers one
extra repaint.

Native Linux/Xvfb: Studio and Studio 2 in Dark and Light at 1520×1040 and
900×720; the Local Demo message is truncated at 900 px and its tooltip shows
the full text; the amber authorization warning shows in place. Suite: 103
passed; format, strict all-target Clippy and build pass. Linux only.

## k6 Demo preset

**k6 Demo** (toolbar, next to **Local Demo**) loads a ready plan for Grafana
k6's public QuickPizza demo API: 4 requests with status and response-time
assertions, a JSON value check, an extraction, warmup and 4 SLOs. The API token
is random per load and lives in the secret `pizza_token` variable, so it never
reaches saved plans, history or Git. `test.k6.io` was not used: it now answers
every page with a 302 redirect to QuickPizza, which RustLoad does not follow.

- `k6_demo_plan_is_valid_and_keeps_its_token_out_of_saved_plans`: the plan
  validates; the token is not serialized, `{{pizza_token}}` is; a reopened copy
  asks for the token again.
- Real traffic (not in CI): 1 user for 6 s gave 20/20 HTTP 200 with every
  assertion and the extraction passing. Native Linux/Xvfb: clicked **k6 Demo**,
  authorized and ran for 22 s (10 users after ramp-up): 239 measured requests,
  142 excluded by warmup, 0 errors, all four SLOs passing; **Stop** gave
  "Run Cancelled · 257 completed · 0 failed · SLO Passed (partial run)". The
  toolbar wraps cleanly at 900×720.

Suite: **103 passed**. Format, strict all-target Clippy and build pass.

## Validation feedback and Studio 2 layout fixes

Reported from a real session: **Validate plan** seemed to do nothing (the
result was only a grey status-bar line). Fixes, all checked natively on
Linux/Xvfb in Dark and Light at 1520×1040 and 900×720 with a 3-request plan
(warmup, assertions, SLO thresholds) and a real Local Demo run:

- **Validate plan** shows a green "Plan is valid" or red "Plan has a problem"
  card under the button until the plan changes. Validation errors name the
  request (`Request "Pi": Invalid URL`, `Request #2: …` when unnamed) or
  `Plan variables: …`.
- Status bar messages have a tone: validation errors and failed runs red, the
  missing authorization tick and cancelled runs amber, success green. When a
  run ends, "Run started…" is replaced by its state, completed/failed counts
  and SLO verdict.
- Studio 2 request editor: Name, Method/URL and tabs stay in place and only
  the tab content scrolls. Before, scrolling the Assertions tab hid the URL.
  When the editor is under 230 px tall, everything scrolls together as before.
- Studio 2 load panel: the authorization checkbox and origins stay visible
  under the scrolling load settings. Before, it was cut off at 1520×1040.
- SLO threshold rows (Studio and Studio 2): combo boxes, value, unit and remove
  button are on one centre line in fixed columns (pixel-checked). The remove
  buttons are full-size, also in the variables/headers table.
- Progress meters (Virtual Users card, HTTP status distribution) no longer draw
  a dot at 0% or put the percentage on top of the fill.
- The assertion tree uses `≤` like the SLO items.

New test `validation_errors_name_the_request_or_plan_variables`. Suite:
**102 passed** (3 lib, 6 compare, 27 core/report, 16 dashboard, 26 engine,
24 import). Format, strict all-target Clippy and build pass. Linux only.

## Last-plan restore fix

Before: the app started with an empty plan but kept the last plan's path, so
**Save** / Ctrl+S silently overwrote that file with the empty plan. Now the
remembered plan is reopened at start; if it is missing or unreadable the path
is forgotten.

- `saved_plans_load_and_bad_files_are_refused` covers the shared
  `TestPlan::load` (round trip, missing file, wrong schema, invalid JSON,
  over 16 MiB).
- Native Linux/Xvfb: saved the Local Demo plan, quit with Ctrl+Q, restarted:
  the plan reopened with "Plan opened…". Deleted the file and restarted: a new
  plan, and **Save** opened the path dialog (default `rustload-plan.json`)
  instead of writing to the old path.

Suite: **101 passed**. Format, strict all-target Clippy and build pass. Linux only.

## OpenAPI YAML import

OpenAPI documents and referenced files may be YAML (new dependency
`yaml-rust2` 0.13.0; Cargo.lock adds `yaml-rust2`, `arraydeque` 0.5.1 and
`hashlink` 0.12.2). The dialog tab reads "OpenAPI (JSON/YAML)".

- 100 tests pass (3 YAML unit tests in the library, 6 compare, 25 core/report,
  16 dashboard, 26 engine, 24 import); fmt, strict Clippy, build and
  `git diff --check` succeed.
- `openapi_yaml_unquoted_versions_are_accepted`: `swagger: 2.0` and
  `openapi: 3.1` written without quotes (read as numbers) are accepted;
  `openapi: 4.0` is still refused.
- Unit (`import::yaml`): core-schema scalars (hex/octal, floats, 64-bit
  overflow to float; `.inf`, `.nan`, `1e999`, `yes`, `off`, `1_000`, `1:20`
  stay text; quoted numbers stay text); integer/boolean/float/null keys as
  written; `!!str`, `!`, `!!int` and an ignored custom tag; merge keys from a
  list with written keys winning and a quoted `<<` as a plain key; an alias
  used as a key; multiple documents, a collection as key and a bad merge
  refused; depth 128 accepted, 129 refused, also when reached through an
  alias; empty and comment-only input.
- Import: `openapi_yaml_imports_like_its_json_twin` (the petstore in YAML
  with comments, flow/block styles and `200:` keys gives the same operation
  order and the same built requests, variables and warnings as the JSON
  file; BOM + Swagger 2.0 YAML), `openapi_yaml_keys_scalars_and_anchors`
  (aliased parameter in two operations, `<<` merged schema, `0x10`, `yes`,
  `007`, `!!str 200`, `200:`/`true:` response keys),
  `openapi_yaml_alias_bombs_and_deep_nesting_are_refused` (nine levels of ten
  aliases refused with the 64 MiB message; written 129-level flow and block
  nesting refused; 5,000,000 `[` as YAML and as JSON refused, all within
  10 s — the test file finishes in about 0.4 s),
  `openapi_yaml_errors_are_short_and_never_show_content` (unclosed flow,
  bad indentation, second document, unknown alias, truncated JSON: message
  under 160 characters with a line and without the marker; non-UTF-8 input),
  `openapi_yaml_external_refs_load_like_json_ones` (YAML root → YAML path
  item → YAML components → `.yml` schema → JSON file, and the same from a
  JSON root; four files read once; a broken YAML file is a warning with its
  position but not its content). Existing tests changed: the Swagger test now
  expects YAML to import and truncated JSON to be refused with a JSON error;
  the bounded-refs test uses a malformed YAML file instead of a valid one.
- Engine: `imported_yaml_openapi_spec_reaches_the_server` — a YAML spec with
  a `200:` key and a `$ref` to a YAML schema file runs against the localhost
  fixture with zero failures; the echoed body equals the generated
  `{"qty": 2, "item": "book"}`.
- Native Linux/Xvfb, Dark, 1440×900 (fixture on 127.0.0.1:8080): a scratch
  `api.yaml` with `$ref`s to `paths/echo.yml` (anchored query parameter
  reused by alias), `schemas/order.json` and `schemas/item.yaml`. The tab
  read "OpenAPI (JSON/YAML)" and the path label mentioned .json/.yaml/.yml;
  Preview showed `http://127.0.0.1:8080`, "Read 3 referenced file(s)" and
  GET /echo, POST /echo, GET /health in file order. Import (Replace) gave
  three requests; both /echo requests had `q=hello` and the POST body was
  `{"qty": 2, "item": "book"}`. A 10 s run with 10 users sent 200 requests,
  0.00 % errors, all HTTP 200. A malformed YAML file showed "Not a valid JSON
  or YAML document: YAML: while parsing a flow sequence, expected ',' or ']'
  (line 4, column 1)" without its content.
- Not checked: macOS/Windows, Light theme and narrow windows for this change
  (only labels changed), very large YAML files in the dialog (the parser is
  run twice; time for a 50 MiB YAML file was not measured), YAML in UTF-16.

## OpenAPI document order, external refs and server choice

OpenAPI operations now follow the document (serde_json `preserve_order`; no new
crate). `$ref`s to local JSON files inside the document's folder are bundled
before parsing; the dialog offers server, server-variable `enum` and Swagger
scheme dropdowns that re-derive URLs without re-reading files.

- 90 tests pass (6 compare, 25 core/report, 16 dashboard, 25 engine, 18 import);
  fmt, strict Clippy, build and `git diff --check` succeed.
- New import tests: `openapi_operations_follow_document_order` (non-alphabetical
  paths and methods, selection built in document order, body properties keep
  schema order), `openapi_external_refs_resolve_relative_to_each_file`
  (sibling file with pointer, `./paths/…` path item, subfolder file, reference
  inside a referenced file, `../common.json` that stays inside the folder,
  whole-file and `file#` references; each of four files read once; the
  bytes-only entry point lists the references instead),
  `openapi_refs_across_files_stop_at_cycles` (Pet → Owner → [Pet] across two
  files stops at the repetition; a dangling reference in a referenced file is
  named), `openapi_external_refs_stay_local_and_bounded` (`..` escape,
  absolute path, symlink to a file outside (unix), https URL, `file:` scheme,
  missing file, YAML file, a file of exactly 10 MiB read and one byte over
  refused, 50-file limit in document order, pointer to nothing in an external
  and in the root document; no warning contains file contents),
  `openapi_servers_and_variables_can_be_chosen` (servers with descriptions,
  enum/non-enum variables, default missing from its enum offered first,
  re-selection changes built URLs, Base URL override wins, relative server and
  bad URLs rejected and the base cleared, Swagger schemes choice, hostless
  Swagger needs a base URL). The picker test now expects document order.
- `json_assertions_compare_objects_regardless_of_key_order` (core): with
  `preserve_order`, an expected object matches a response with other key
  order, array order still counts, RFC 6901 escapes still resolve. All
  existing engine extraction/assertion tests pass unchanged.
- `imported_multi_file_openapi_spec_runs_with_a_chosen_server` (engine): a
  spec split over four files (path item, shared parameter, schema with a
  nested file reference) with the second server and its port enum chosen runs
  against the localhost fixture with zero failures; the first step's
  extracted `/request/method` reaches the second step as a header.
- Native Linux/Xvfb (fixture on 127.0.0.1:8080): a spec in a scratch folder
  with `paths/echo.json`, `schemas/order.json` and `schemas/item.json`, two
  servers (Production; Local `http://{host}:{port}` with enums) and one https
  `$ref`. Preview listed /echo GET/POST/PUT, /health, /broken, /about in file
  order, "Read 3 referenced file(s)", the URL warning, and "Requests go to
  https://api.invalid/v1". Choosing Local showed host and port dropdowns;
  127.0.0.1 and 8080 changed the target to `http://127.0.0.1:8080`. Typing a
  Base URL disabled the dropdowns and showed "(Base URL field)"; clearing it
  restored the selection with the operation choice kept. Four of six
  operations imported (Replace); a 30 s run with 10 users sent 600 requests,
  0.00 % errors, all HTTP 200. Light theme at 900×720 and Dark at 1440×900
  were checked. Fixed during this check: the server row's labels and combo
  boxes stepped downwards (labels moved into the combo text, row top-aligned),
  and at 900×720 the taller dialog ran past the window bottom (the list and
  warnings now take the height left on screen below them).
- Not checked: macOS/Windows, YAML, very large multi-file specs in the dialog,
  symlink handling on Windows.

## OpenAPI operation picker update

The OpenAPI import parses the document once and lets you choose operations
(text, method and tag filters; Select all / None for the filtered set; 500-request
limit enforced in the dialog). JSON bodies merge `allOf`, take the first
`oneOf`/`anyOf` and otherwise use type placeholders. API keys in a header or
query become `{{api_key}}` references with secret plan variables, and a secret
field whose value is exactly one `{{name}}` reference is now saved as that
reference.

- 83 tests pass (6 compare, 24 core/report, 16 dashboard, 24 engine, 13 import);
  fmt, strict Clippy, build and `git diff --check` succeed.
- New import tests: `openapi_picker_lists_operations_and_builds_the_selection`
  (keys, order, tags, filter matching, selection in document order with
  consecutive IDs, variables and warnings only for the selection),
  `openapi_selection_is_capped_at_the_plan_limit` (501 operations listed, 500
  built), `openapi_bodies_merge_all_of_and_take_the_first_one_of`,
  `openapi_type_skeletons_use_formats_and_stop_at_recursion` (formats, minimum,
  nullable 3.1 types, required-only objects, recursive `$ref`, warning counts),
  `openapi_api_keys_become_secret_variable_references` (header/query, AND/OR
  requirements, anonymous alternative, cookie warning, Swagger 2.0, saved plan
  and history snapshot keep references but never key values). The existing
  OpenAPI test now expects a placeholder for the example-less `age`.
- `secret_fields_keep_whole_variable_references_only` (core): references are
  kept in plan JSON and the redacted history/report plan; literals, `Bearer
  {{token}}`, `{{ a }}`, `{{a}}{{b}}` and `{{api-key}}` are dropped.
- `selected_openapi_operations_with_api_keys_reach_the_server` (engine): two of
  three operations selected by tag run against the localhost fixture with zero
  failures; the fixture echoes the header key and the query key, and the
  `allOf` placeholder body is `{"id": <zero UUID>, "qty": 1}`.
- Native Linux/Xvfb (fresh profile, fixture on 127.0.0.1:8080): a 30-operation
  spec with seven tags, apiKey header security and a cookie apiKey. Preview
  listed all operations checked; None, tag filter "echo" + Select all (5),
  case-insensitive text filter "HEALTH" and a click on the row label selected
  six; Replace imported six requests and added `api_key`. With the key entered,
  a 6 s run sent 120 requests, 0.00 % errors. Save wrote `{{api_key}}` in every
  header and an empty `api_key` value; the literal key was found in neither the
  plan, the history database nor the preferences. Light theme at 900×720: the
  list, filters and warnings fit; the method filter showed the two PATCH
  operations. A 600-operation spec showed "600 operations selected, but a plan
  holds at most 500 requests (6 already in the plan)…" with Import disabled;
  the "bulk" tag (300) re-enabled it. The cURL tab still previews as before.
  Fixed during this check: the filter row's combo boxes were vertically offset,
  and the dialog overflowed the window (the list and warnings now share the
  height left in the window).
- Not checked: macOS/Windows, specs with thousands of operations in the
  dialog, YAML.

## Workspace widgets update

Monitor and Studio use the Studio 2 widget system with their own page sets.
Monitor's results area (below the fixed run header) and Studio's bottom chart
strip are now dashboards whose default pages reproduce the previous layouts. Two
widget kinds were added (KPI Cards, Requests per Second); the catalog has ten.

- 76 tests pass (6 compare, 23 core/report, 16 dashboard, 23 engine, 8 import);
  fmt, strict Clippy, build and `git diff --check` succeed.
- New dashboard tests: `workspace_defaults_reproduce_the_built_in_layouts`
  (Studio 2 default unchanged, Monitor and Studio defaults widget by widget,
  defaults survive normalization), `old_preferences_keep_studio2_layouts_and_gain_workspace_defaults`
  (preferences JSON without the new keys keeps two Studio 2 pages intact and
  gains Monitor/Studio defaults; saved layouts contain no plan/auth fields),
  `workspace_page_sets_are_independent`, `normalization_fits_pages_to_their_workspace`
  (Studio: Bottom only, no dashboard-only; Studio 2: no Main dock; empty set gets
  the workspace default), `new_kinds_round_trip_and_restore_defaults_uses_fresh_ids`
  (and an unknown future kind is removed), `monitor_pages_keep_limits_and_main_area_ordering`.
  The eight-entry catalog assertion became `latency_distribution_widget_is_in_the_catalog`.
- Native Linux/Xvfb: preferences created by the previous build (Studio 2 page
  with Logs hidden and a Latency Distribution added, plus a duplicated "Perf"
  page) opened with the new build on the "Perf" page unchanged; after exit the
  rewritten file's Studio 2 section was byte-identical and gained the Monitor and
  Studio keys. With a fresh profile, Studio (1440×900 Dark) matched the previous
  build's two-chart strip, and a Local Demo run in Monitor matched the previous
  build's Monitor screenshot (KPI row, RPS + Latency, Status + Active users,
  logs at the bottom). In Monitor: added Latency Distribution to the Main area,
  dragged it before Endpoint Breakdown, used Move earlier, hid Requests per
  Second with × and restored it in Layouts…, removed HTTP Status Distribution,
  Save layout; after a restart the layout was intact and opening the history run
  rendered it with that run's data. A history summary with `latency_distribution`
  stripped showed "No latency distribution recorded for this run". Light theme
  checked; at 900×720 the KPI cards wrap to two columns and the toolbar's layout
  controls wrap to their own row (fixed during this check: the layout selector
  was clipped). Studio: + Widget lists the catalog directly (Bottom only), the
  dock selector is disabled with an explanation, compact width switches the
  strip to tabs as before; added Latency Distribution, moved it up, removed
  Latency, saved. Studio 2 in the same profile kept its default page.
- Not checked: macOS/Windows, the Right dock in Monitor with many widgets, and
  long runs with many Monitor widgets.

## Arrival-rate stages update

The Arrival Rate (open) profile can ramp its rate: **Ramp the rate** seeds three
stages from the current rate and duration (up, hold, down to 0) with a start
rate of 0; the rate field becomes **Start rate** and the duration is derived.
User stages and rate stages share one generic table and draggable preview.
Throughput charts (Monitor and the Studio 2 widget) draw a dashed **Target it/s**
line from `Snapshot.target_rate` up to the elapsed time.

- `arrival_starts_follow_the_integrated_rate`: a constant rate gives exactly
  `k / rate` and `None` at the end; a 0 to 50 it/s ramp over 10 s gives
  `sqrt(2kT/R)` for all 250 starts; a start-rate ramp, a hold / fall / zero-rate
  pause / rise schedule (no start inside the pause), monotonic times and a start
  count matching the integrated area for mixed slopes.
- `rate_stages_validate_label_and_load_old_plans`: 20 stages accepted, 21
  rejected; zero duration, negative, NaN, infinite, over-100,000 and all-zero
  rates rejected; a zero constant rate is still rejected; duration and warmup
  limits use the stage total; label, peak rate and elevated warning; plans
  without `rate_stages` load as a constant rate.
- `arrival_summary_names_constant_and_staged_rates`: status/report wording and
  history without a schedule.
- `arrival_rate_stages_ramp_the_start_rate` (localhost fixture): 0 to 60 it/s
  over 3 s starts 85–90 iterations (the integral is 90), drops none, the first
  interval runs below 25/s and a later one above 35/s; `target_rate` is the
  schedule. `arrival_rate_stages_still_drop_iterations_without_idle_vus`:
  `/body-delay` with 1 VU drops at least 40 and started + dropped is 85–90.
  Both were run 6 more times in a row without a failure.
- Native Linux/Xvfb, Studio at 1440×900 (Dark): ticking Ramp the rate seeded
  6 s / 18 s / 6 s stages; table edits to 40 it/s and dragging the first point
  (to 11 s, 24.3 it/s) updated the derived duration (35 s). A real run against
  the fixture completed with 833 requests, matching the schedule's integral
  (832.35), 0 errors and 0 dropped; the Monitor's dashed Target it/s line
  tracked the measured rate and the status line read "Arrival rate 0 to 40/s
  (stages)". Light theme checked on the finished run. Studio 2 (Light 1440×900
  and Dark 900×720): same controls, a preview drag, toggling off restored a
  40 it/s constant rate and on re-seeded stages; a run from Studio 2 stayed in
  Studio 2 with the target line in the Throughput widget. Studio at 900×720
  (Dark) showed the full rate table and preview without clipping; the user
  Stages editor, now the same shared helper, still seeded 7/25/7 s stages and a
  drag kept whole users.

Suite: **70 passed** (6 compare, 23 core/report, 10 dashboard-layout, 23 engine,
8 import). Format, strict all-target Clippy and build pass. Linux only.

## OpenAPI import update

**Import…** gains an **OpenAPI (JSON)** tab for OpenAPI 3.x and Swagger 2.0
documents, with an optional base URL override.

- `openapi3_operations_become_requests`: server variables, path-level `$ref`
  parameters, path variable with example (`petId=42`) and without (warned,
  sanitized `owner_name`), required query default, optional query skipped,
  required header example, cookie parameter and HEAD operation skipped with
  warnings, `$ref` request body built from property examples and `enum`,
  non-JSON body left empty, global bearer security and an operation that clears
  it with `security: []`. The plan fails validation until `access_token` has a
  value, then passes.
- `swagger2_and_base_url_rules`: host/basePath/schemes, body parameter example,
  Basic security, base URL override; relative server without override, base
  URLs with credentials/query/non-HTTP scheme, YAML and non-OpenAPI JSON are
  rejected with explanations.
- `imported_openapi_requests_reach_the_server`: an imported document runs
  against the localhost fixture with zero failures; the path variable, required
  query, header and JSON body arrive as written.
- Native Linux/Xvfb: imported a three-operation document with base URL
  `http://127.0.0.1:8080` (Replace). The preview listed 3 requests and the bearer
  warning; the plan gained `endpoint=echo` and secret `access_token`. Run Test
  first stopped with "Re-enter secret value: access_token"; after entering a
  value a 3 s run completed with 60 requests and 0 errors.

Suite: **65 passed** (6 compare, 20 core/report, 10 dashboard-layout, 21 engine,
8 import). Format, strict all-target Clippy and build pass. Linux only.

## Failure reasons update

Measured failures are grouped by request and a sanitized reason; logs use the
same reason. The status panel (Monitor and the Studio 2 status widget) lists the
five largest groups; HTML and JSON reports include them all.

- `failures_are_grouped_by_sanitized_reason` (real localhost traffic): `/status/500`
  gives "HTTP 500, expected 2xx"; an expected 201 gives "HTTP 200, expected 201";
  a JSON mismatch, an empty extraction, a timeout, a body over the cap, a server
  that closes without answering ("Request failed before a response") and a port
  with no listener ("Connection refused") each yield one group. Group counts
  equal the failed count, and no reason contains a host or URL. The refused
  run's ERROR log line ends with the same reason.
- `error_groups_are_bounded_and_ordered`: same reason on two requests gives two
  groups, most frequent first with first/last times; at 100 groups existing ones
  keep counting and new reasons go to the overflow counter.
- `offline_reports_escape_html_and_csv_formulas` now also checks the escaped
  Failure reasons table.
- Native Linux/Xvfb: a 4 s run against `/status/500` showed "80 · HTTP Request ·
  HTTP 500, expected 2xx" in Dark and Light, and log lines read "Request #1 ·
  HTTP 500, expected 2xx · 1.2 ms" (the first attempt printed "HTTP 500" twice
  and truncated the request column; both fixed).

Suite: **62 passed** (6 compare, 20 core/report, 10 dashboard-layout, 20 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Latency distribution widget update

Studio 2 gains an eighth widget, **Latency Distribution**: the whole-run
histogram as at most 40 log-spaced bars with P50/P95/P99 marked.

- `latency_distribution_covers_every_sample_in_log_buckets`: empty histogram has
  no bars; 1 ms to 2 s spreads over 40 contiguous, equal-ratio buckets whose
  counts sum to the sample count; 12 responses near 5 ms land in one bar; a
  single recorded value gives one bar.
- `latency_distribution_widget_is_an_eighth_catalog_entry`: catalog of 8, the
  widget can be added and survives a serialization round trip.
- `hundred_users_real_scenario_history_and_exports` now also checks that the
  bars of a real run hold exactly the whole-run sample count.
- Native Linux/Xvfb: added the widget from + Widget > Add to Bottom, ran the
  Local Demo (80 RPS cap). Dark and Light showed the bars around 0.8–1.1 ms with
  a sparse tail to ~6 ms and the P50/P95/P99 lines; hovering a bar showed
  "0.88 – 0.93 ms / 104 responses" (the y range keeps 30% headroom so the label
  is not clipped). At 900×720 the widget appears as a tab.

Suite: **60 passed** (6 compare, 19 core/report, 10 dashboard-layout, 19 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Arrival-rate (open model) update

Load profiles gain **Arrival Rate (open)**: iterations start at a fixed rate on an
idle VU from a pool of at most Max VUs; with every VU busy the iteration is
dropped and counted (`dropped_iterations`, warmup separately). The RPS cap and
think time are disabled in this mode. Shared controls in Studio and Studio 2;
the status panel, HTML report and comparison warnings show dropped iterations.

- `arrival_rate_starts_iterations_on_schedule`: 50/s for 2 s with 10 VUs, an RPS
  cap of 1 and 5 s think time (both ignored) starts 95–100 iterations, drops
  none, has no failures and matches the fixture's served count.
- `arrival_rate_drops_iterations_when_every_vu_is_busy`: `/body-delay` (~100 ms)
  at 50/s with 2 VUs drops at least 40; started + dropped equals the offered
  95–100; active users never exceed 2; error rate stays 0.
- `arrival_rate_profile_validates_and_labels`: rates outside 0.01–100,000,
  NaN and infinity rejected; profile label; elevated-load warning uses the
  arrival rate instead of the unused RPS cap; older plans load the default rate.
- All earlier engine tests (closed model, stages, warmup, pacing, stop) pass
  after the iteration body moved into a shared function.
- Native Linux/Xvfb: Studio (Dark) showed Max VUs and Arrival rate with the RPS
  cap and think time disabled. A real run of `/body-delay` at 50/s, 3 Max VUs,
  10 s completed with 251 requests, 0 errors and 249 dropped iterations
  (251 + 249 = 500 = 10 s × 50/s); Active Virtual Users stayed at 3. Studio 2
  (Light) showed the same shared controls.

Suite: **58 passed** (6 compare, 18 core/report, 9 dashboard-layout, 19 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Multi-stage load profile update

Load profiles gain **Stages** (up to 20). Constant, RampUp and Stages share one
schedule in the engine; each virtual user computes when it is active. Shared
profile selector and stage editor (table plus draggable preview) in Studio and
Studio 2; comparison warnings describe stage profiles.

- `every_profile_is_one_schedule`: constant users start at 0 s; ramp-up users
  start at ramp × i / users exactly as before; a stage profile up, down and up
  again reactivates a user on the next rise (5 s, idle after 15 s, back at
  25 s); a user above the peak never starts.
- `stage_profiles_validate`: empty, zero-length, over 10,000 users, zero peak and
  21 stages rejected; warmup limits use the stage total; older plans load.
- `stage_profile_ramps_users_up_and_down`: real traffic over 1 s→4, 1 s→4,
  1 s→1. Active users reach 4 and fall to at most 2 near the end; zero failures;
  the run lasts the stage total.
- All earlier engine tests, including ramp-up and 100 users, still pass.
- Native Linux/Xvfb: selecting Stages seeded 6 s→10, 18 s→10, 6 s→0 and locked
  users/duration. Dragging the first point gave 9 s→13 (first attempt ran away
  to 1,728 users because the axes rescaled during the drag; bounds now freeze
  while dragging, and the pick uses the press origin). The run of 9 s→13,
  18 s→10, 6 s→0 completed with zero errors and cancellations; the Active
  Virtual Users chart rose to 13 by 9 s, eased to 10 by 27 s and fell to 0 at 33 s.

Suite: **55 passed** (6 compare, 17 core/report, 9 dashboard-layout, 17 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Warmup update

Load Configuration (Studio and Studio 2, one shared control) gains Warmup: None,
By duration or By requests. Requests started during warmup go to separate
counters; the measured window feeds SLO and comparison throughput; charts draw
warmup completions and the warmup end; HTML reports state what was excluded;
comparison warns when warmup differs. The measurement contract in `CLAUDE.md`
and `docs/decisions.md` is updated.

- `duration_warmup_is_excluded_from_every_result_metric`: 1 s warmup at 40 RPS
  over 3 s. Warmup and measured starts in their expected ranges; started =
  completed + cancelled per phase; histogram samples, status totals, endpoint
  starts and bytes contain only measured requests; charted warmup completions
  equal the warmup counter; measured from 1.0 s; SLO throughput uses the
  measured window.
- `request_count_warmup_excludes_exactly_the_first_starts`: exactly 15.
- `unfinished_warmup_leaves_no_measured_data`: no measured requests, SLO
  Inconclusive.
- With warmup ignored in the engine, all three tests fail; with it they pass.
- `warmup_settings_validate_and_older_plans_have_none` and a comparison test for
  the measured window and the warmup warning.
- Native Linux/Xvfb: Local Demo with a 3 s duration warmup. During warmup Monitor
  showed "Warmup in progress (first 3s) · 41 requests excluded so far" with zero
  measured cards; afterwards "excluded 60 requests · measured from 3.0s", the gray
  warmup line for 0–3 s and the dashed warmup-end marker.

Suite: **52 passed** (6 compare, 15 core/report, 9 dashboard-layout, 16 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## cURL / HAR import update

**Import…** (toolbar, shared by Studio and Studio 2) parses pasted
Copy-as-cURL commands or a HAR file, previews requests and warnings, and adds
them to the plan or replaces its requests. Replacing removes request-scoped SLO
thresholds because IDs restart. Also fixes the comparison hover text, which used
an arrow glyph the bundled font lacks.

- `tests/import.rs` (6 tests): Chrome multi-line bash output (query to Params,
  secret token/Authorization/Cookie, Accept-Encoding and Content-Type dropped,
  JSON body); `$'…'` with `\'` and `\u00e9`; double quotes; repeated `-d`;
  Firefox `-X PUT`; attached short options and `--name=value`; `-G`; `-u` and URL
  credentials (percent-decoded); `--json`; `--data-urlencode`; "Copy all" with
  `;` separators, HEAD skipped, `-k`/`-L`/`-F`/unknown options reported, non-curl
  text ignored; the 500-request limit; imported plans validate and never save
  credentials; HAR order, XHR/fetch filter, pseudo-headers, form params, non-HTTP
  and OPTIONS entries.
- `imported_curl_requests_reach_the_server_as_written`: three imported commands
  run against the fixture `/echo`; assertions confirm the custom header, the JSON
  content type and the raw body arrived; zero failures.
- Native Linux/Xvfb: imported a curl command (Replace) against the fixed-port
  fixture: Params, masked Authorization, removed Content-Type/Accept-Encoding,
  `-k` warning; ran it with 0 % errors. In Studio 2 (Light) imported a HAR file
  (Add): document entry skipped, two requests appended. Rechecked the dialog
  label and path-only request names after the fixes.

Suite: **47 passed** (5 compare, 14 core/report, 9 dashboard-layout, 13 engine,
6 import). Format, strict all-target Clippy and build pass. Linux only.

## Run comparison update

Monitor's history cards gain a UTC start time, **Baseline** (pin) and **Compare**.
The comparison view shows the overall verdict, setup warnings, a noise band,
a whole-run table, overlaid throughput/P95 charts and per-request changes. No
SQLite schema change; runs load through `History::read` and `History::points`.

- `tests/compare.rs` (5 tests): direction per metric, the noise boundary
  (+2.0 % unchanged, +2.5 % regressed), Improved/Regressed/Mixed/Unchanged/No
  data verdicts, informational data rate, change from a zero baseline,
  endpoint matching by ID with renamed and one-sided requests, setup and state
  warnings, and UTC labels from run IDs (including a leap day).
- `saved_runs_can_be_compared`: two real fixture runs (2 and 4 users) saved to
  SQLite, read back and compared: one matched endpoint, one users warning,
  throughput present on both sides.
- Native Linux/Xvfb: two 5 s Local Demo runs (10 and 20 users). Pinned the first
  as baseline and compared: verdict Mixed, warning "Virtual users differs: 10 vs
  20", whole-run and per-request tables, dashed baseline overlays. Swap reversed
  the roles and the pin; a 25 % noise band left only P99 regressed. Checked in
  Dark, Light and 900×720.

Suite: **40 passed** (5 compare, 14 core/report, 9 dashboard-layout, 12 engine).
Format, strict all-target Clippy and build pass. Linux only; macOS CI separate.

## SLO thresholds update

Plans gain up to 50 thresholds (P50/P90/P95/P99/mean latency, error rate,
throughput; whole run or one request). Every snapshot carries the evaluation;
the final verdict is computed after the last counts and state, persisted with
the summary, and exported in HTML (section and table), JSON and CSV
(`slo_verdict` column).

- `slo_evaluation_uses_bounds_scopes_and_whole_run_throughput`: equality passes,
  upper/lower bounds, endpoint scope, missing endpoint and empty histogram give
  no data, Failed beats Inconclusive, partial flag on cancelled runs.
- `slo_threshold_validation`, `slo_thresholds_round_trip_and_older_plans_still_load`:
  ranges, NaN/infinity, unknown request, the 50 limit; plans saved before this
  change load with no thresholds; stored configurations keep thresholds.
- `slo_verdict_is_final_persisted_and_exported`: real fixture traffic. `/health`
  gives Passed; `/status/500` gives Failed with a 100 % error rate. The verdict
  and thresholds are read back from SQLite; HTML and CSV contain the verdict.
- Native Linux/Xvfb: added two thresholds in the Studio editor (P95 ≤ 300 ms for
  all requests, P99 ≤ 0.1 ms for Checkout). During the Local Demo run Monitor
  showed "currently failing" with per-threshold pass/fail; after 30 s it showed
  "SLO Failed" (P95 0.94 ms pass, Checkout P99 5.94 ms fail). Studio 2 (Light)
  showed the tree items, the shared editor and the verdict in Live Metrics.
  Checked at 900×720 and after reopening the run from history.

Suite: **34 passed** (14 core/report, 9 dashboard-layout, 11 engine). Format,
strict all-target Clippy and build pass. Linux only; macOS CI separate.

## Compatible history and Byte/s update

Persisted metric structs (`Snapshot`, `Point`, `Counts`, `Latency`, `Endpoint`,
`LogEntry`) now default missing fields. New: response body bytes per endpoint and
interval, and a trailing-5s Data/sec rate (cards, endpoint table, reports).

- `history_written_before_byte_metrics_still_loads` writes v0.1-shaped rows into
  SQLite and reads them through `History`. Without the serde defaults it fails
  with ``missing field `bytes` ``; with them it passes.
- `response_bytes_are_counted_per_endpoint_and_interval`: real fixture traffic;
  endpoint bytes equal completed × known body size (11 B and 16 KiB), endpoint
  sums equal the total, and persisted interval bytes sum to the total.
- Native Linux/Xvfb: Local Demo run in Monitor (Dark), Studio 2 (Light) and a
  900×720 window. Monitor showed eight cards on one wide row and four rows of two
  when narrow. Endpoint data matched body size × completions (Checkout 16 B × 61 =
  976 B). The Studio 2 RPS card shows data/sec on its own line.

Suite: **30 passed** (11 core/report, 9 dashboard-layout, 10 engine). Format,
strict all-target Clippy and build pass. Linux only; macOS CI separate.

## Release automation configuration

Added a tag-triggered release workflow for Linux x64, Windows x64, macOS ARM64,
and macOS x64. It validates the tag against Cargo.toml, tests each target before
packaging, requires all four packages before publication, and creates SHA-256
checksums. Native Windows release builds use the GUI subsystem and static MSVC
runtime; macOS packaging derives the app version from Cargo metadata.

Local checks for this change:

- actionlint 1.7.12: both CI and release workflows pass.
- Bash syntax checks: macOS script and embedded workflow Bash steps pass.
- Executed tag validation, version matching, prerelease detection, checksum
  generation, and rejection of missing/extra packages with temporary fixtures.
- Format, strict all-target Clippy, **27 tests**, and optimized release build pass.
- Ran the workflow's Linux packaging step against the real optimized binary.
  Verified archive contents, executable permissions, binary hash, and local shared
  libraries. The extracted app opened a native Xvfb window and exited cleanly.

These checks do not execute the macOS/Windows build jobs or GitHub publication.
Those require the first release workflow run. Native UI acceptance, Developer ID
signing, Apple notarization, and Windows publisher signing remain separate checks.
See [releases](releases.md) for the tag and download procedure.

## Pacing accuracy and metrics lock update

The global gate now advances from its schedule instead of the wake-up time, so
millisecond timer rounding no longer lowers the achieved rate; at most 10 ms of
lag is caught up, and no burst credit accumulates. Response bodies are parsed at
most once, before the shared metrics lock is taken.

Local fixture, 200 VUs, 5 seconds, release build, achieved starts/sec:
limit 500 → before **320**, after **500**; 1,000 → **469** / **1,000**;
2,000 → **654** / **1,998**; 5,000 → **769** / **4,994**. Debug builds matched.
New test `pacing_reaches_limits_above_timer_resolution` (500 RPS, 2 s, expects
900–1,005 starts) fails on the previous gate (626 starts) and passes now.

Current suite: **27 passed** (9 core/report, 9 dashboard-layout, 9 engine).
Format, strict all-target Clippy and build pass. Linux only; macOS CI separate.

## Personal widget layouts update

Current suite: **26 passed** (9 core/report, 9 dashboard-layout, 8 real HTTP engine
integration tests), zero failures. Format, strict all-target Clippy and build pass.
Layout tests cover hidden/restored docks, cross-dock moves, global dashboard order,
preserved workbench assignments, independent named pages, serialization of widths
and hidden state, non-reused IDs, normalization, catalog limits and grid packing.

Native Linux/Xvfb checks loaded legacy preferences, hid latency and the right KPI
group, and dragged logs before throughput. Remaining tiles filled the freed space.
Created and saved a `Performance` dashboard-only page, added catalog widgets,
changed visibility while a local HTTP run was Running, and observed completion:
**584 real requests, zero failures**. The preceding local run completed 580 with
zero failures. Save layout flushed immediately; after restarting, both named
pages, active page, hidden widgets and order were retained. Hiding every bottom
widget removed the dock entirely; Restore default widgets reinstated it.

Screenshots are linked in [widgets.md](widgets.md). Layouts contain no test/request
data. The engine and measurement contract are unchanged. macOS UI execution and
remote CI results remain separate from these local checks.

## Studio 2 update

The alternative Studio 2 workbench was added alongside the original Studio and
Monitor. `cargo fmt --all -- --check`, strict all-target Clippy, build and the
existing 17 unit/integration tests pass after the change.

Native Xvfb checks exercised shared request/header editing across both Studio
layouts, simultaneous body/load editing, target authorization, execution staying
in Studio 2, current-run logs and live cards/sparklines, and Monitor receiving the
same run. A local 30-second scenario completed **584 requests with zero failures**.
Reset then Run/Stop produced a separate **Cancelled** run with **22 completed
requests** recorded in SQLite. Studio 2 selection survived an application restart.

Dark, Light and 900×720 compact screenshots are under `docs/screenshots/studio2-*`.
These are real native Linux screenshots; macOS remains unverified. The engine and
measurement definitions were not changed for this layout.

Executed in the managed Debian 13 x86_64 environment with Rust/Cargo 1.99.1.
No external target received load. These are Linux results, not macOS verification.

## Automated checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: **17 passed**, zero failed/ignored (9 core/report tests,
  8 engine integration tests). Crate binary/unit/doc harnesses have zero additional
  tests; they are not counted as functional validation.
- `cargo build --locked`: passed; native executable launched successfully.
- `bash -n scripts/package-macos.sh`: passed syntax check only.

The tests cover real five-method HTTP transmission, query/headers/JSON/Basic Auth,
Bearer extraction, 100 active VUs, ramp, global pacing, per-user CSV/token isolation,
401/429/500, explicit expected errors, malformed JSON, disconnect, timeout, body
cap, full-body latency, cancellation/draining, counts/histograms, bounded rings,
secret omission, auth reference round-trip, SQLite persistence and all exports.
The stop test uses a 30-second timeout and a deliberately slow server; Stop drains
and cancels within the test's seven-second bound, with no in-flight users left.

## Sustained local execution

`cargo run --locked --example soak -- 900 /tmp/rustload-soak`:

- Elapsed 900.24 seconds; 100 VUs, 30-second ramp, 200 starts/sec ceiling.
- **145,640 started/completed/successful**, zero failed or cancelled.
- Fixture count, engine count, persisted interval totals and histogram samples agree.
- RSS peak **15.84 MiB**, final **15.11 MiB** (initial **8.94 MiB**).
- 1,457 retained logs; 693 stored intervals. This earlier run used the initial
  1–1.5-second polling cadence. Sampling was subsequently aligned to second boundaries.
- This process includes both engine and local fixture; it does not include egui.
  The gradual retention/cache allocation is not a claim of constant total RSS or
  a proof against every possible leak. Ring retention is additionally unit-tested
  beyond the configured 900-point and 5,000-log limits.

The final engine revision was rerun with
`cargo run --locked --example soak -- 60 /tmp/rustload-soak-final`:
**60.14 seconds, 9,729 successful real requests, 61 intervals including final partial
bucket, 98 logs**, with all conservation checks passing.

## Native UI exercised using Xvfb / software OpenGL

- Started Local Demo from Studio, authorized the loopback targets and ran the
  three-step scenario. Representative completed run: 584 requests, no failures.
- Dark/Light in both workspaces; theme retained across application restarts.
- Saved a three-request plan from the native file-path dialog, verified its JSON
  and `{{access_token}}` reference, then reopened it using Cmd/Ctrl+O.
- Opened a completed SQLite run after restarting the application; real metrics,
  interval graphs and endpoint breakdown loaded correctly.
- Resized to 900×720: configuration becomes collapsible, charts become tabbed.
- Cmd/Ctrl+Q displayed the unsaved-plan warning; discard-and-close exited the app.
- GUI logs contained no startup error/panic. Current screenshots show real data,
  without mock values: [Studio Dark](screenshots/studio-dark.png),
  [Studio Light](screenshots/studio-light.png), [Monitor Dark](screenshots/monitor-dark.png),
  [Monitor Light](screenshots/monitor-light.png).

## Remaining release checks

- **macOS native launch, manual acceptance and .app packaging are unrun.** A macOS
  machine is required. Developer ID signing/notarization is not configured.
- Windows execution and packaging are unrun. CI definitions have been added;
  remote workflow results must be checked on GitHub separately from these local checks.
- 1,000-VU capacity and long-running renderer RSS are not benchmarked. The local
  100-VU tests are functional checks, not hardware-independent throughput promises.
- Flow/Pause/templates and cookie sessions remain excluded.
  File picking uses explicit paths; HTML export is an offline structured report.
- M0–M3 functionality is implemented and locally checked; M4 remains open for the
  platform/release checks above. This is a runnable MVP, not a signed release.

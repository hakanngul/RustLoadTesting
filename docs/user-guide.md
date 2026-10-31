# RustLoad user guide

[English](user-guide.md) · [Türkçe](user-guide_tr.md)

This guide is for testers and developers who have RustLoad open and want to get
work done: build a scenario, run it, read the results and keep or compare them.
Labels in **bold** are the exact texts you see in the app.

Contents

1. [What RustLoad is](#1-what-rustload-is)
2. [Install and launch](#2-install-and-launch)
3. [Tutorial: your first test with Local Demo](#3-tutorial-your-first-test-with-local-demo)
4. [Building scenarios](#4-building-scenarios)
5. [Load models and settings](#5-load-models-and-settings)
6. [Running, stopping and reading Monitor](#6-running-stopping-and-reading-monitor)
7. [SLO thresholds](#7-slo-thresholds)
8. [History, comparison and reports](#8-history-comparison-and-reports)
9. [Importing from cURL, HAR and OpenAPI](#9-importing-from-curl-har-and-openapi)
10. [Custom dashboards](#10-custom-dashboards)
11. [Theme, preferences and shortcuts](#11-theme-preferences-and-shortcuts)
12. [Troubleshooting and FAQ](#12-troubleshooting-and-faq)
13. [Glossary of metrics](#13-glossary-of-metrics)

## 1. What RustLoad is

RustLoad is a desktop app that sends real HTTP traffic from your machine to an
API and measures how it responds under load. A test is a **plan**: an ordered
list of HTTP requests (the scenario) plus a load configuration. Virtual users
(VUs) run the scenario repeatedly while RustLoad records latency, throughput,
status codes and failures.

Use it to check an API before a release, compare two builds or configurations,
or find the load at which latency or errors start to grow. Everything stays
local: there is no account, cloud service or separate backend. History is kept
in a local SQLite file and reports are offline files.

It is a version 0.1 tool with deliberate limits: requests run in a fixed
sequence (no branching or loops), and there are no retries, redirects or
cookies. See [Limits](#limits-you-should-know) for the full list.

> **Only test what you are allowed to test.** Load tests can degrade or take
> down a service. Point RustLoad only at systems you own or have written
> permission to test. Before every run you must tick **I am authorized to test
> all targets in this plan.** (Studio) or **I am authorized to test all
> configured targets.** (Studio 2). The box clears itself whenever you change
> the plan, so you confirm the targets you are about to hit. The origins
> (scheme, host and port) of all requests are listed under the checkbox.

## 2. Install and launch

### Download a release

When a version is published, the
[GitHub Releases](https://github.com/hakanngul/RustLoadTesting/releases) page
offers macOS DMGs (Apple Silicon and Intel), a Windows x64 ZIP, a Linux x64
archive and SHA-256 checksums. The builds are not signed with a Developer ID or
notarized, so macOS and Windows may warn on first launch. See the
[release guide](releases.md) for details.

### Build from source

1. Install Rust with [rustup](https://rustup.rs).
2. Clone and run:

   ```sh
   git clone https://github.com/hakanngul/RustLoadTesting.git
   cd RustLoadTesting
   cargo run --locked
   ```

   The first build downloads the pinned toolchain from `rust-toolchain.toml`
   and the dependencies from `Cargo.lock`.

You need a desktop session with OpenGL. On Linux, install the X11/Wayland,
xkbcommon and GL runtime libraries. On macOS, install the Xcode Command Line
Tools (`xcode-select --install`).

### Platform status

| Platform | Status |
| --- | --- |
| Linux | Build, automated tests and native UI checks have been done |
| macOS | Product target; native acceptance testing is still pending |
| Windows | Running and packaging have not been verified |

### A first look around

The top bar has three workspace buttons: **Studio**, **Studio 2** and
**Monitor**. All three work on the same plan and the same run; switching never
starts or stops anything. The app remembers the last workspace you used.

Below them is the toolbar: **+ New**, **Open**, **Save**, **Save as…**,
**Import…**, **Local Demo**, **▶ Run Test**, **■ Stop**, **Reset view**,
**Export Report**, then the layout controls (**Layout: …**, **+ Widget**,
**Edit layout**, **Layouts…**, **Save layout**). **Unsaved changes** appears
when the plan has edits you have not saved. The status line at the top right,
under **Local Engine**, shows the last message (a long one is cut short; hover
to read all of it) and the CPU and RAM used by RustLoad itself.

![Studio with the Local Demo plan loaded and the Login request's Extractors tab open](screenshots/guide/01-studio-demo.png)

- **Studio** is the main editor. Left: **Test Plan** with
  **⚙ Users & plan settings** and the **HTTP Requests** list. Center: the
  request editor. Right: **Load Configuration** and the authorization checkbox.
  Charts sit at the bottom.
- **Studio 2** is an alternative workbench with a collapsible plan tree, a
  compact request editor, the body and load settings side by side, live metrics
  on the right and charts and logs at the bottom. Running from here keeps you
  in Studio 2.
- **Monitor** shows results: the **Test Runs** list on the left, and the run
  that is open (live or from history) on the right.

## 3. Tutorial: your first test with Local Demo

Local Demo starts a small test server inside RustLoad on a random
`127.0.0.1` port and loads a three-step scenario against it. No traffic leaves
your machine.

1. Start with a plan that has no unsaved changes (save or discard first;
   **Local Demo** is disabled while **Unsaved changes** is shown).
2. Click **Local Demo**. The plan **Local API · Checkout scenario** appears
   with three requests:
   - **Login** (POST `/login`) extracts `/access_token` from the JSON response
     into the variable `access_token`.
   - **List products** (GET `/products`) sends `Authorization: Bearer
     {{access_token}}` through its **Auth** tab.
   - **Checkout** (POST `/checkout`) posts `{"product_id":"{{product_id}}"}`
     and expects status `201` (see its **Assertions** tab). `product_id` is a
     plan variable under **⚙ Users & plan settings**.
3. Look at **Load Configuration**: 10 virtual users, 30 s, **Constant Users**,
   **Global RPS cap** 20, **Think / iteration** 100 ms.
4. Optional: open **⚙ Users & plan settings**, scroll to **SLO thresholds**
   and click **+ Add threshold** (default: P95 latency ≤ 300 ms). Set
   **Warmup** to **By duration** to exclude the first seconds.
5. Tick **I am authorized to test all targets in this plan.** The listed
   origin is the local `http://127.0.0.1:<port>`.
6. Click **▶ Run Test**. RustLoad switches to Monitor and the charts start to
   fill.
7. Wait 30 s for the run to finish (**Completed**), or click **■ Stop** to end
   it early (**Cancelled**, with partial results kept).
8. Click **Export Report**, choose **HTML**, **JSON** or **CSV**, edit the
   **File path** if you like and click **Continue**.

![Monitor after the demo run: SLO verdict, warmup line, KPI cards, charts, status codes and logs](screenshots/guide/02-monitor-results.png)

The run now appears in **Test Runs** and stays there after you restart the app.

The demo server lives only as long as the app. If you save the demo plan and
open it after a restart, its port is gone and requests fail with
"Connection refused". For a server on a fixed port, run
`cargo run --locked --example fixture` in a terminal (it listens on
`127.0.0.1:8080` until Ctrl-C) and point the request URLs there.

### k6 Demo (internet)

**k6 Demo** loads a ready plan for Grafana k6's public QuickPizza demo API at
`https://quickpizza.grafana.com`, which is published for load-test practice:
Home, Quotes, Doughs and Recommend pizza (POST with a JSON body), each with
expected status 200 and a response-time limit; Doughs also checks
`/doughs/0/name` is `"Thin"` and Recommend pizza extracts `/pizza/name`. Load:
Ramp-up Users, 10 users, 10 s ramp-up, 70 s total with a 10 s warmup, RPS cap 20,
1 s think time. SLOs: P95 ≤ 800 ms, P99 ≤ 1500 ms, error rate ≤ 1 %, throughput
≥ 10 req/s. The demo API token is generated at random in the secret variable
`pizza_token` (QuickPizza accepts any 16-character token), so a saved copy asks
for it again: enter `token ` followed by any 16 letters or digits, or click
**k6 Demo** again. It is a shared public service: keep the load modest and tick
the authorization box only if you accept its terms.

## 4. Building scenarios

### Requests

- **+ Add request** (Studio sidebar or Studio 2 tree) adds a GET request at the
  end. A plan holds 1 to 500 requests.
- Right-click a request for **Duplicate**, **Move up**, **Move down** and
  **Delete**.
- Every iteration runs the requests from top to bottom. If a step fails, the
  rest of that iteration is skipped and the virtual user starts the next
  iteration (after think time, in the closed models).

For each request set **Name**, **Method** (GET, POST, PUT, PATCH, DELETE) and
**URL**. URLs must be absolute `http://` or `https://`. The editor tabs are:

| Tab | What it does |
| --- | --- |
| **Params** | Query parameters as **Name** / **Value** / **Secret** rows |
| **Headers** | Request headers, same table |
| **Body** | Request body text. **JSON body** sends `Content-Type: application/json`. In Studio 2 the body has its own panel with a **JSON** / **Raw text** selector |
| **Auth** | **None**, **Basic** (**Username**, **Password (session-only)**) or **Bearer** (**Token or {{variable}} (session-only)**) |
| **Assertions** | Expected status, maximum response time and a JSON value check |
| **Extractors** | Copy values from the JSON response into variables |
| **Advanced** | **Override request timeout** for this request |

Credentials in the URL (`https://user:pass@host`) are rejected; use **Auth**.
Query parameters whose names look sensitive (for example `token` or
`api_key`) must be moved to **Params** and marked secret.

### Variables and `{{var}}` references

Write `{{name}}` in a URL path or query value, a header, the body or auth
fields. Values come from three places:

1. **Plan variables** under **⚙ Users & plan settings** (Studio) or
   **CSV Data / Variables** (Studio 2 tree).
2. **CSV columns** (see below). They override plan variables with the same name.
3. **Extractors**, which update the variable for that virtual user only.

Each virtual user gets its own copy of the variables, so values never leak
between users. The URL scheme and host must stay literal: `https://{{host}}/x`
is rejected so a run cannot be redirected to another server by data. A
variable that has no value when a request is prepared fails that step with a
local preparation error ("Missing variable: …").

### Extraction and chaining

On the **Extractors** tab, click **+ Add extractor** and fill in:

- **Variable**: the name to store, for example `access_token`.
- **/json/pointer**: an RFC 6901 JSON Pointer such as `/access_token` or
  `/items/0/id`. This is JSON Pointer, not JSONPath.

A string is stored as is; numbers, booleans and objects are stored as their
JSON text. If the pointer finds nothing, the request counts as failed
("Extraction found nothing at …") and the iteration stops there. Use the
variable in later requests, for example **Auth** → **Bearer** →
`{{access_token}}`.

### Assertions and expected status

By default a request succeeds when the status is 200–299, the body is read
completely and every assertion and extraction passes. On the **Assertions**
tab you can:

- tick **Exact expected status (otherwise 200–299)** and enter one code (for
  example `201` or, for a negative test, `401`);
- tick **Maximum response time** and enter a limit in ms;
- fill **JSON Pointer (optional, e.g. /created)** and **Expected JSON value
  (e.g. true, 123, or "text")**. The expected value must be valid JSON, so
  text needs quotes.

A request that returns an expected `401` counts as successful, but the 401 still
shows up in the HTTP status counts.

### CSV data per virtual user

1. Put a CSV file with a header row next to your saved plan.
2. In **⚙ Users & plan settings** → **CSV data source**, enter the path in
   **Path (relative to saved plan directory)**. For an unsaved plan the path is
   relative to the folder RustLoad was started from.
3. Header names become variables (`{{email}}`, `{{password}}`, …).

Virtual user 1 gets the first data row, user 2 the second, and so on; a user
keeps its row for the whole run. If there are fewer rows than users the plan is
rejected unless you tick **Reuse rows when there are more users than data
rows**, which wraps around. Headers must be unique and non-empty, and the file
may be at most 16 MiB.

### Secrets: what is and isn't saved

RustLoad never writes credential values to plan files, history, logs or reports.

- Tick **Secret** on a param, header or plan variable to treat it as a
  credential. Names containing `authorization`, `cookie`, `password`, `passwd`,
  `token`, `api_key` or `secret` are always secret (the checkbox is locked)
  and their values are masked.
- **Basic** passwords and **Bearer** tokens are always secret.
- A secret whose value is exactly one variable reference, such as
  `{{access_token}}`, is saved as that reference. Literal values and mixed text
  such as `Bearer {{token}}` are dropped when saving.
- Request bodies stay in the session unless you tick **Persist body in plan
  (ensure it contains no credentials)** (Studio) or **Save body in plan (no
  literal secrets)** (Studio 2). Put credentials in secret variables and
  reference them from the body.
- History snapshots leave out request bodies and URL query strings. Response
  bodies are never logged or stored.

After you **Open** a plan, re-enter the omitted secret values and session-only
bodies before running. Validation tells you which one is missing, for example
"Re-enter secret value: api_key" or "Re-enter Bearer token or variable
reference".

### Saving and opening plans

- **Save** writes to the current file; if there is none it asks for a path
  (default `rustload-plan.json`). **Save as…** always asks. Saving to an
  existing path replaces the file. **+ New** and **Local Demo** clear the
  current path, so the next **Save** asks for one.
- **Open** asks for a file path. Plans up to 16 MiB with schema version 1 can be
  opened.
- **+ New** starts an empty plan. If there are unsaved changes, the
  **Unsaved plan** dialog offers **Keep editing** or **Discard changes**.
- **Validate plan** (bottom of **⚙ Users & plan settings**) checks the plan and
  the CSV file without sending traffic. The result appears under the button as
  a green **Plan is valid** or a red **Plan has a problem** card (the message
  names the request, for example `Request "Pi": Invalid URL`) and stays until
  you change the plan.

File dialogs are plain path fields, not system file pickers. Relative paths are
resolved from the folder the app was started in.

## 5. Load models and settings

Load settings live in **Load Configuration** (Studio right panel, Studio 2
lower right). They are locked while a run is active.

### Profiles

Choose one under **Profile** (Studio) or **Load profile** (Studio 2):

| Profile | How load is generated |
| --- | --- |
| **Constant Users** | All **Virtual users** start at once and loop the scenario for **Total duration** |
| **Ramp-up Users** | Users start evenly over **Ramp-up** seconds (Studio 2: **Ramp-up time**), then hold. Ramp-up is part of the total duration |
| **Stages** | The user count follows up to 20 stages; each moves linearly from the previous count (0 at the start) to its **Target users** over its **Duration** |
| **Arrival Rate (open)** | Iterations start at a fixed rate (it/s), regardless of how fast the target answers, using a pool of at most **Max VUs** |

The first three are **closed-loop** models: each user waits for its own
response, then for **Think / iteration**, before it starts again. When the
target slows down, the offered load drops with it.

**Stages** shows a table and a preview chart. Edit **Duration** and
**Target users** in the table, use **+ Add stage** or **×**, or drag the points
on the chart (left/right changes the duration, up/down the value). **Virtual
users** and **Total duration** are derived from the stages and greyed out. When
the count goes down, users finish their current iteration and then idle.

### Arrival Rate (open model)

1. Choose **Arrival Rate (open)**. **Virtual users** becomes **Max VUs**.
2. Set **Arrival rate** in iterations per second (0.01–100,000 it/s). One
   iteration is one pass through all requests, so requests/s can be a
   multiple of it/s.
3. Set **Max VUs** high enough to cover rate × iteration time with headroom.

If every VU is busy when an iteration is due, it is not started and is counted
as a **dropped iteration**. Dropped iterations are not requests: they do not
change the error rate, but they mean the offered load was lower than planned.
The status panel shows them in a warning color; raise **Max VUs** when they
appear.

To change the rate over time, tick **Ramp the rate** (row **Rate stages**). The
rate field becomes **Start rate**, and a stage table with **Target rate** and a
draggable preview appears. RustLoad seeds three stages (ramp up, hold, ramp
down). Each stage moves linearly from the previous rate to its target; the total
duration is the sum of the stages. A stage with rate 0 pauses new starts. At
least one stage needs a rate above 0. Charts draw the planned rate as a dashed
**Target it/s** line. Untick the box to return to a constant rate.

![Arrival Rate profile with Ramp the rate: Start rate, rate stage table and draggable preview](screenshots/guide/04-arrival-rate-stages.png)

**Global RPS cap** and **Think / iteration** do not apply in this mode and are
disabled.

### Other settings

| Setting | Meaning |
| --- | --- |
| **Total duration** | Whole run length in seconds, including ramp-up and warmup (1–86,400 s) |
| **Global RPS cap** | Closed models only: the most HTTP request starts per second across all users (1–100,000). Starts are spread evenly; idle time does not build up burst credit |
| **Think / iteration** | Closed models only: pause after each scenario iteration (0–300,000 ms) |
| **Request timeout** | Time limit per request (1–300,000 ms). A single request can override it on its **Advanced** tab |
| **Body cap** | Largest response body read, in bytes (default 10 MiB, up to 100 MiB). A larger response fails the request |
| **Warmup** | **None**, **By duration** (first N seconds) or **By requests** (first N requests) |

In Studio 2, **Think / iteration**, **Request timeout** and **Body cap** are
inside the collapsible **Timeout, think time & body limit** section.

**Warmup** excludes the requests that *start* during warmup from every result
metric (counts, percentiles, status codes, endpoints, bytes). They are counted
separately and shown as a gray **Warmup** line on the request-rate charts, with
a **Warmup end** marker. Warmup is part of the total duration and must be
shorter than it. If warmup never finishes (for example a request-count warmup
on a short run), the run has no measured data.

**Elevated load: verify target and local capacity.** appears above 100 users or
an RPS cap (or peak arrival rate) above 100. It is a reminder, not a block.

### Which model should I use?

| Goal | Use |
| --- | --- |
| Smoke test or quick check | **Constant Users** with a few users and a short duration |
| Find out what N concurrent users experience | **Constant Users** or **Ramp-up Users** |
| Load that rises, holds and falls (soak, step or spike shape) | **Stages** |
| A fixed request rate the target must sustain, even when it slows down | **Arrival Rate (open)**, with stages for a rate ramp |
| Protect a fragile target from bursts | Any closed model with a **Global RPS cap** |

Closed-loop tests can make latency look better than it is when the target is
overloaded, because slow responses also slow down new requests (coordinated
omission). If you need the offered load to stay fixed, use Arrival Rate.

## 6. Running, stopping and reading Monitor

### Starting and stopping

1. Fix any problem the status line reports. **▶ Run Test** validates the plan
   first and shows "Validation: …" if something is wrong. Status line messages
   that need attention are coloured: red for errors, amber for warnings (such
   as a missing authorization tick), green for success. When a run ends the
   status line shows its state, completed and failed counts and SLO verdict.
2. Tick the authorization checkbox and click **▶ Run Test**. From Studio you go
   to Monitor; from Studio 2 you stay in Studio 2.
3. While the run is active, plan and load editing is locked ("The running
   configuration is locked. Stop the test to edit.").
4. **■ Stop** stops new requests at once, interrupts think-time and pacing
   waits, and gives in-flight requests up to 5 s before cancelling them. The
   run is saved as **Cancelled**. A run that reaches its duration is
   **Completed**; an internal engine error gives **Failed**.

Only one run can be active at a time. Changing workspaces or layouts during a
run does not affect it. **Reset view** clears the result shown (in Studio 2 it
also clears the current-run widgets); history is not touched.

### Monitor layout

The header shows the run name, its state and the elapsed time, then a note
on how values are measured: "RPS and data/sec: last 5s · Percentiles: entire
run, fully read responses · Latency in ms". Below that, if they apply:

- a warning line, for example when the history writer fails;
- the warmup line ("Warmup in progress …", then "Warmup (first 3s) excluded 60
  requests · measured from 3.0s");
- the SLO line (see [SLO thresholds](#7-slo-thresholds)).

Everything below is a widget page you can rearrange (see
[Custom dashboards](#10-custom-dashboards)). The default page contains:

**KPI cards**

| Card | Meaning |
| --- | --- |
| **Virtual Users** | Active users / target users |
| **Requests / sec** | Completed requests per second over the last 5 s |
| **Data / sec** | Response body bytes per second over the last 5 s (headers excluded) |
| **P50 / P95 / P99 Latency** | Percentiles of all measured, fully read responses in the run so far |
| **Error Rate** | Failed ÷ completed requests, in % |
| **Request Count** | Completed requests (measured window only) |

**Charts** (x axis: elapsed seconds; drag to pan, scroll to zoom)

Each chart lists its series next to the title; click a name or its dot to hide
or show that series. After you pan or zoom, **Reset zoom** appears in the
title row and returns to the whole run (double-clicking the chart does the
same).

- **Requests per Second**: **Total**, **Success** and **Failed** completions
  per interval, plus **Warmup** and **Target it/s** when relevant.
- **Response Time (ms) · interval percentiles**: P50, P95 and P99 computed
  separately for each interval. These are not averaged into the whole-run
  values; a spike here can be invisible on the cards and vice versa.
- **Active Virtual Users**: users running over time.

**HTTP Status Distribution** lists each status code with its share. Below it
are counters: **Network**, **Timeout**, **Body limit**, **Local preparation**,
**Unexpected status**, **Assertion/extraction**, **Cancelled** and
**In-flight**. For Arrival Rate runs it adds the rate and **Dropped
iterations**. Under **Failure reasons · measured, most frequent first** you
see the five most common failures per request, such as "HTTP 500, expected
2xx", "Timeout", "Connection refused", "Slower than the 500 ms limit" or
"JSON at /ok did not match". Hover for first and last occurrence. The full list
(up to 100 groups) is in the HTML and JSON reports. Reasons are built only from
status codes, plan values and fixed labels; response content and raw network
error text (which can contain URLs) are never shown.

**Endpoint Breakdown · slowest P95 first** has one row per request definition
with **Completed**, **Errors**, **Avg RPS**, **Data** and **P50**–**P99**.
Rows are keyed by the request, not by the expanded URL.

**Live Logs** shows every failed request and roughly one in every hundred
successful ones, with level, request number, status and latency. Filter by
**All levels** / **INFO** / **ERROR** or with **Filter logs…**; untick
**Follow** to stop auto-scrolling. At most 5,000 entries are kept, and logs
never contain request bodies or credential values.

The **Latency Distribution** widget (not on the default page; add it with
**+ Widget**) draws the whole-run histogram as up to 40 log-scaled bars with
P50/P95/P99 markers. Hover a bar to see its range and count.

On narrow windows the right dock folds into a **Dashboard widgets** section and
the bottom dock shows its widgets as tabs.

### Studio 2 while running

![Studio 2 during a run: plan tree, compact request editor, load panel, Live Metrics and bottom charts](screenshots/guide/03-studio2-run.png)

The **Live Metrics** widget on the right shows **Virtual Users**,
**Requests / sec** (last 5 s, plus data rate), **P95 Latency** and
**Error Rate** (whole run) and **Average Response Time**, each with a small
trend line from the real interval data, followed by the completed and failed
counts. **Summary Report** in the tree opens the current run in Monitor.

## 7. SLO thresholds

Thresholds turn a run into a pass/fail verdict.

1. Open **⚙ Users & plan settings** (Studio) or click an item under
   **SLO Thresholds** in the Studio 2 tree (**Add SLO thresholds** when there
   are none).
2. Click **+ Add threshold**. Choose a **Metric** (**P50 latency**,
   **P90 latency**, **P95 latency**, **P99 latency**, **Mean latency**,
   **Error rate**, **Throughput**), a **Scope** (**All requests** or a single
   request) and a **Target**.
3. Latency (ms) and error rate (%) are upper limits (≤); throughput (req/s) is
   a lower limit (≥). A value exactly equal to the target passes.

A plan can have up to 50 thresholds. They never stop a run. During the run the
line reads "SLO currently passing" or "currently failing" with each result; at
the end it shows the verdict:

- **Passed**: every threshold met.
- **Failed**: at least one missed.
- **Inconclusive**: a threshold had no data (for example a request that never
  completed).

Latency thresholds use the whole-run histograms, error rate uses completed
requests, and throughput is completed requests divided by the measured time
(not the 5-second card). Cancelled or failed runs are marked as partial. The
verdict is saved in history and included in all report formats.

When you import with **Replace current requests**, thresholds scoped to single
requests are removed, because the new requests reuse the old IDs.

## 8. History, comparison and reports

### History

Every finished run (completed, cancelled or failed) is stored in the local
history and listed in Monitor under **Test Runs**, newest first, with its name,
start time (UTC), state, duration and request count.

- **Current run** returns to the run of this session.
- **Open summary** loads a saved run into Monitor. All widgets then show that
  run, and **Export Report** exports it.
- **↻** reloads the list.

There is no delete button in the app; see
[Where is my data stored?](#where-is-my-data-stored).

### Comparing two runs

1. Click **Baseline** on the run you want to compare against. "Baseline pinned ·
   choose Compare on another run" appears.
2. Click **Compare** on another run (the candidate).

The **Run comparison** view shows:

- an overall verdict: **Improved**, **Regressed**, **Mixed**, **Unchanged** or
  **No data**;
- **Swap** to exchange baseline and candidate, and **Close comparison**;
- **Noise band ±** (default 2 %): changes inside it count as unchanged. This is
  a tolerance, not a statistical significance test. The value is remembered;
- **Not a like-for-like comparison:** warnings when users, duration, profile,
  RPS cap, think time, timeout, warmup or request steps differ, when a run did
  not complete normally, or when a run dropped arrival-rate iterations. The
  comparison is still shown;
- a **Whole run** table (**Metric**, **Baseline**, **Candidate**, **Change**,
  **Result**) for throughput, P50/P90/P95/P99, mean latency, error rate and data
  rate. Data rate is informational (**Changed**) and does not affect the
  verdict;
- overlaid **Throughput (requests per second)** and **P95 latency (ms) ·
  interval** charts;
- **Per request · change from baseline (hover for values)**, matched by
  request ID, with requests that exist in only one run listed below.

Throughput and data in a comparison are averages over the whole run
(completed ÷ elapsed), not the 5-second card value.

### Exporting reports

**Export Report** is enabled when the run shown has finished. Choose the format,
check the **File path** and click **Continue**. An existing file at that path is
replaced.

| Format | Contents |
| --- | --- |
| **HTML** | Offline, self-contained summary: configuration, metrics, SLO results, failure reasons, warmup and arrival-rate info, time series. Not interactive |
| **JSON** | Full summary, redacted configuration and the per-interval time series, with metric definitions |
| **CSV** | One row per request: counts, P50–P99, bytes and the SLO summary |

Exports use the redacted plan (no secrets, no bodies). Text is HTML-escaped,
and CSV cells that could be read as spreadsheet formulas are neutralized. If
part of the time series is missing, the report says so; the summary counters
remain authoritative.

## 9. Importing from cURL, HAR and OpenAPI

Click **Import…** to open **Import requests**. Choose a source, click
**Preview**, review the requests and warnings, choose **Add to current
requests** or **Replace current requests**, then click **Import**. Import never
contacts the network; it only reads what you paste or the files you name.

After import, check the new requests, fill in any secret variables and tick the
authorization box again.

### Copy as cURL

In your browser's DevTools, right-click a request → *Copy as cURL (bash)* (or
*Copy all as cURL*) and paste one or more commands.

- Query strings move to **Params**; `user:pass@` in the URL becomes Basic Auth.
- `-H`, `-X`, `-d`/`--data-raw`/`--data-binary`/`--data-urlencode`, `--json`,
  `-u`, `-G`, `-b`, `-A`, `-e` and `--url` are understood. A JSON body with a
  JSON content type becomes a JSON body.
- Connection headers, `Content-Length` and `Accept-Encoding` are removed so
  responses are read uncompressed and sizes stay exact.
- `-k` and `-L` are ignored with a note (TLS is always verified, redirects are
  never followed). `-F`, `-T` and `@file` data are skipped, and HEAD/OPTIONS
  requests are not imported. The preview lists everything skipped.

### HAR file

Save a HAR from DevTools → Network → *Save all as HAR* and enter its path.
**Only XHR/fetch requests (skip pages, scripts, styles, images)** is on by
default. Only requests are read, never responses. Files may be up to 50 MiB and
a plan holds at most 500 requests.

In both cURL and HAR imports, Authorization, Cookie and similar headers are
marked secret: they work in this session but are not saved.

### OpenAPI (JSON/YAML)

![Import requests with an OpenAPI file: server choice, operation filters and selection, warnings](screenshots/guide/05-import-openapi.png)

1. Choose **OpenAPI (JSON/YAML)** and enter the path of an OpenAPI 3.x or
   Swagger 2.0 file (`.json`, `.yaml`, `.yml`).
2. Optionally enter a **Base URL**, for example a local or staging address. It
   replaces the server from the document.
3. Click **Preview**. You get:
   - a **Server:** dropdown when the document lists several servers, plus one
     dropdown per server variable with an `enum` (a Swagger 2.0 file with
     several schemes gets a scheme dropdown). **Requests go to** shows the
     resulting base URL and where it comes from;
   - the operations (GET/POST/PUT/PATCH/DELETE) in document order, each with
     method, path, name and tags. All start selected;
   - a text filter (**Filter by path, name or operationId**), **All methods**
     and **All tags** filters, and **Select all (N)** / **None**, which act on
     the operations currently shown;
   - "N of M operations selected", and an error if the selection would take
     the plan past 500 requests.
4. Click **Import**.

How the document is translated:

- Path parameters become `{{variables}}` added to the plan with their example
  values. Required query and header parameters use `example`, `examples`,
  `default` or `enum` values.
- JSON bodies use the document's examples. Where there are none, type
  placeholders are filled in (`""`, `0` or the `minimum`, `false`, one-item
  arrays, and fixed values for date, date-time, uuid and email formats). The
  preview counts such bodies; replace the placeholders with real test data.
- Bearer and OAuth security becomes `{{access_token}}`; API keys in a header or
  query parameter become `{{api_key}}` (`{{api_key_2}}`, … for more schemes).
  These are added as secret plan variables for you to fill in. Basic Auth needs
  its credentials. Cookie API keys, other schemes and non-JSON bodies are
  listed in the preview.
- `$ref`s to other JSON or YAML files in the document's folder (and its
  subfolders) are followed. Files outside the folder, absolute paths and
  `http(s)` references are not read; unresolved references are listed by name.
  Up to 50 files of 10 MiB each are read.
- YAML follows the YAML 1.2 core schema: `yes`/`no`/`on`/`off` stay text, and
  an unquoted `openapi: 3.1` or `swagger: 2.0` is accepted. Errors show line
  and column, never file contents.

## 10. Custom dashboards

Studio, Studio 2 and Monitor each have their own set of widget pages. Changing
one workspace never changes the others, and layouts are separate from plans:
saving a plan does not include them.

| Workspace | Fixed parts | Widget areas (docks) |
| --- | --- | --- |
| Studio | Plan list, request editor, Load Configuration | Bottom |
| Studio 2 | Plan tree and editors (unless the page is dashboard-only) | Bottom, Right |
| Monitor | Run list, run header, warning, warmup and SLO lines | Main area, Bottom, Right |

### Arrange widgets

1. Click **Edit layout**. Each widget shows a `::` drag handle, its name, a
   **…** menu and **×**.
2. Drag a `::` handle onto another widget to put it in front of that widget.
   Without dragging, use **…** → **Move earlier** / **Move later**.
3. In **…**, choose **Small**, **Medium** or **Full row** for the width, or
   **Move to Bottom** / **Move to Right** / **Move to Main area** to change the
   dock. (Widgets in the Right dock share the panel width.)
4. **×** hides a widget. An empty dock disappears and its space goes back to
   the editors.
5. Click **Save layout** to save at once. Layouts are also saved when you close
   the app.

Drag the edge of the bottom or right panel to resize it.

**Expand** appears in a widget's top-right corner while the pointer is over it
(outside **Edit layout**). It opens the widget large in the middle of the
window, still live; **Close**, Esc or a click outside returns. The layout is
not changed.

### Add widgets and manage pages

**+ Widget** adds a widget to the current page (in Studio 2 and Monitor via
**Add to …** submenus per dock). **Layouts…** opens **My Studio layouts &
widgets** (or Studio 2 / Monitor):

![Layout manager for Studio with the widget list and the widget catalog](screenshots/guide/06-layout-manager.png)

- The page selector, **Save layout** and **Restore default widgets** (puts the
  workspace's default widgets back on this page).
- **Page name** with **Create blank page**, **Duplicate current page** and
  **Rename current page**; **Delete page** (one page must remain).
- One row per widget: a checkbox to show or hide it (this is how hidden widgets
  come back), its dock, its width, **Up**, **Down** and **Remove** (deletes the
  widget's configuration from this page).
- **Widget catalog** with a **+** button for each widget type.
- Studio 2 only: **Dashboard only (hide request editor)** with **Dashboard row
  height**. New blank pages in Studio 2 start in this mode; use **Configure test
  in Studio** to edit and authorize the plan from such a page. Monitor has
  **Main area row height**.

Switch pages with the **Layout: …** selector in the toolbar, also during a run.

### The ten widgets

| Widget | Shows |
| --- | --- |
| **Throughput** | Completed requests per second over time |
| **Requests per Second** | Total, success and failed rates (Monitor's chart) |
| **Latency** | Interval P50/P95/P99 |
| **Recent Events / Logs** | The filterable log |
| **Active Users** | Virtual users over time |
| **HTTP Status Distribution** | Status codes, error counters and failure reasons |
| **Endpoint Breakdown** | Per-request table |
| **Live Metrics** | The Studio 2 metric cards |
| **KPI Cards** | Monitor's eight headline cards |
| **Latency Distribution** | Whole-run latency histogram |

In Studio and Studio 2 widgets show the current run; in Monitor they show the
run that Monitor has open, live or from history. You can add the same widget
more than once. Log filters and **Follow** are shared between all log widgets.

Limits per workspace: 20 pages, 16 widgets per page, page names of 1–60
characters that must be unique. External data sources, custom queries, plugins
and free positioning are not available.

## 11. Theme, preferences and shortcuts

- **☀ Light** / **☾ Dark** (top right) switches the theme.
- Remembered between sessions: theme, last workspace, the last plan path,
  dashboard pages and the comparison noise band. These preferences contain no
  plan, request or credential data.
- On start the app reopens the last plan you saved or opened (re-enter its
  secrets and session-only bodies, as after **Open**). If that file is gone or
  can't be read, the app starts with a new plan and forgets the path, so
  **Save** asks where to save instead of overwriting anything.

| Shortcut | Action |
| --- | --- |
| Ctrl+S (⌘S on macOS) | Save the plan (asks for a path the first time) |
| Ctrl+O (⌘O) | Open a plan (asks first if there are unsaved changes) |
| Ctrl+Q (⌘Q) | Close the app safely |

Ctrl+S and Ctrl+O do nothing while a run is active. When you close the app with
unsaved changes or an active run, **Close RustLoad?** offers **Keep working**,
**Save plan…** and **Discard plan & close** or **Stop & close**. An active run is
stopped and its partial results are saved.

## 12. Troubleshooting and FAQ

### Common messages

| Message | What to do |
| --- | --- |
| Confirm you are authorized to test every configured target. | Tick the authorization checkbox. It clears after every plan change |
| Re-enter secret value: *name* | A secret param, header or variable is empty, usually after opening a saved plan. Enter the value, or use a `{{variable}}` reference |
| Re-enter Basic Auth password / Re-enter Bearer token or variable reference | Fill in the **Auth** tab |
| Target scheme and host cannot contain variables | Write the scheme and host literally; use variables only in the path and query |
| Credentials in URLs are not allowed; use Auth | Remove `user:pass@` from the URL and use **Auth** |
| Move sensitive URL parameters to Params and mark them secret | Move e.g. `?token=` from the URL to **Params** |
| Only HTTP and HTTPS targets are supported / Invalid absolute URL | Use a full `http://` or `https://` URL |
| CSV needs one row per user, or enable Reuse rows | Add rows or tick **Reuse rows when there are more users than data rows** |
| CSV file cannot be opened | Check the path; it is relative to the saved plan's folder |
| Ramp-up must be between 1s and the total duration | Shorten **Ramp-up** or lengthen **Total duration** |
| Warmup duration must be at least 1s and shorter than the total duration | Adjust **Warmup** |
| Rate stages need a peak rate above 0 iterations/s | Give at least one rate stage a target above 0 |
| JSON assertion expected value must be valid JSON | Quote text: `"ok"`, not `ok` |
| Extractor requires a variable name and RFC 6901 JSON pointer | Fill **Variable**; pointers start with `/` |
| An SLO threshold refers to a request that no longer exists | Remove or re-scope that threshold |
| Unsupported schema version; no automatic migration available | The plan file comes from an incompatible version |

### Many dropped iterations

All **Max VUs** were busy when iterations were due: the target is slower than
rate × VUs allows. Raise **Max VUs**, lower the rate, or treat it as a finding
(the target cannot sustain that rate). Dropped iterations do not count as
errors, but the offered load was lower than planned and comparisons warn about
it.

### High error rate

Read **Failure reasons** in the status panel first:

- **HTTP 4xx/5xx, expected 2xx**: the target rejected the request. Check auth,
  missing variables and test data. For an intended status, set **Exact expected
  status**.
- **Timeout**: the target is slower than **Request timeout**.
- **Connection refused** / **Connection reset or closed** / **Connection failed
  (DNS, TLS or TCP)**: the server is down, the port is wrong or the server drops
  connections under load. After a restart, a saved Local Demo plan always ends
  here because its port changed.
- **ResponseTooLarge** (**Body limit** counter): raise **Body cap**.
- **Invalid template, URL or header (check variables)** (**Local preparation**):
  a variable is missing, for example an extraction failed earlier.
- **Extraction found nothing at …** / **JSON at … did not match**: the response
  is not what the scenario expects.

Also check the status line: "Local capacity warning: results may be
generator-limited" means RustLoad itself is short on CPU or memory, so the
numbers describe your machine as much as the target.

### History writer problems

If saving to SQLite fails, Monitor shows "History writer unavailable; export
results before closing" or "History persistence failed; export current results
before closing". The run continues and its results stay in memory: use
**Export Report** before closing. Check free disk space and permissions of the
data folder.

### Where is my data stored?

- **Run history**: `history.sqlite3` in RustLoad's data folder. On Linux that is
  `~/.local/share/rustload/` (or `$XDG_DATA_HOME/rustload/`); other systems use
  their usual per-user application data folder.
- **Preferences and layouts**: eframe's `app.ron`, on Linux in the same folder.
- **Plans and reports**: wherever you save them.

To clear history, close RustLoad and delete or move `history.sqlite3`.

### Limits you should know

- No automatic retries, no redirect following, no cookie jar or cookie
  sessions. TLS certificates are always verified.
- Remote targets use the system HTTP proxy; literal loopback addresses do not.
- Scenarios are sequential only: no flows, pauses between steps, loops,
  conditions or templates. No distributed workers or browser emulation.
- Charts keep 900 points; logs keep 5,000 entries of up to 2 KiB.
- Up to 10,000 users and 100,000 request starts/s are accepted as input; these
  are validation limits, not measured capacity. Measure capacity on your own
  hardware.
- CPU and RAM in the status line are RustLoad's own. Local Demo runs its server
  in the same process, so its usage is included.

## 13. Glossary of metrics

| Term | Meaning |
| --- | --- |
| **Started** | Requests sent: completed + cancelled + in-flight |
| **Completed** | Requests that ended with a result: successful + failed |
| **Successful** | Expected status (200–299 by default), body fully read, all assertions and extractions passed |
| **Failed** | Completed but not successful: unexpected status, assertion or extraction failure, timeout, network error, body cap, preparation error |
| **Cancelled** | Ended by Stop or the end of the run before finishing; not part of the error rate |
| **In-flight** | Started and not finished yet |
| **Error rate** | Failed ÷ completed, in % |
| **Requests / sec (card)** | Completions per second over the last 5 s |
| **Throughput (chart)** | Completions per second per interval over the run |
| **Throughput (SLO, comparison)** | Completed requests ÷ measured time over the whole run |
| **Latency** | From starting the HTTP request until the response body is fully read. Excludes pacing and think time. Only fully read responses count; timeouts and network errors do not |
| **P50 / P90 / P95 / P99** | 50/90/95/99 % of responses were at least this fast. Cards and SLOs use the whole-run histogram; charts use separate per-interval histograms |
| **Mean / Average Response Time** | Arithmetic mean latency of measured responses |
| **Data / sec** | Response body bytes per second over the last 5 s; headers excluded |
| **Virtual user (VU)** | A simulated client that runs the scenario in order with its own variables |
| **Iteration** | One pass of a VU through all requests |
| **Dropped iterations** | Arrival Rate only: iterations not started because all Max VUs were busy |
| **Warmup** | Initial period whose requests are excluded from results and counted separately |
| **Think time** | Pause after each iteration in closed models |
| **Global RPS cap** | Closed models: limit on request starts per second across all users |
| **SLO verdict** | Passed, Failed or Inconclusive, from the plan's thresholds |

For more detail on design and measurement, see the [README](../README.md),
[architecture](architecture.md), [Studio 2](studio2.md) and
[widgets](widgets.md).

# Personal layouts and widget dashboards

Studio, Studio 2 and Monitor all use the same widget system. In each of them the
layout selector, **+ Widget**, **Edit layout**, **Layouts…** and **Save layout**
controls are in the toolbar. These settings belong to the user interface, not the
TestPlan.

## Workspaces

Each workspace owns an independent set of pages; editing Monitor never changes
Studio 2 or Studio. The default page of each workspace reproduces its built-in
layout, so nothing changes until you customize it.

| Workspace | Fixed part | Docks | Default page |
| --- | --- | --- | --- |
| Studio | Test plan list, request editor, Load Configuration (right) | Bottom | Requests per Second + Latency |
| Studio 2 | Request tree and editors (unless Dashboard only) | Bottom, Right | Throughput + Latency + Logs below, Live Metrics right |
| Monitor | Run list (left), run title/state, issue, warmup line, SLO badges | Main area, Bottom, Right | KPI cards; Requests per Second + Latency; Status + Active users; Endpoints; Logs at the bottom |

Studio is restricted to the Bottom dock because a Right dock would collide with
its fixed Load Configuration panel. Dashboard-only pages exist only in Studio 2;
blank pages in Studio and Monitor stay beside their fixed parts. Monitor's Main
area is a twelve-column grid below the run header; its rows take the height of
their tallest widget up to **Main area row height**, and KPI cards size to their
card rows. While no run is open, Monitor shows its empty state and hides the
Main area unless **Edit layout** is on.

## Hide, restore and arrange

1. Turn on **Edit layout**. Each widget gets a `::` drag handle, a menu and `×`.
2. `×` hides the widget. Restore it with its checkbox in **Layouts…**. **Remove**
   in the manager permanently removes that widget's configuration from this page.
3. Drag a handle onto another widget to insert before it. Menu/manager arrows are
   keyboard-friendly alternatives for moving earlier/later.
4. In a workbench layout, move widgets between the workspace's docks (menu
   **Move to …** or the dock selector in **Layouts…**). Hidden widgets
   retain their position and width. If a dock has no visible widgets it disappears,
   returning the space to the request editors.
5. Small, Medium and Full row set grid spans. Partly-filled rows expand to use the
   entire width; hiding one of three small widgets leaves two equal-width widgets.
   Workbench right widgets share the resizable sidebar width; its span control is
   disabled. Drag the bottom panel border to change its height.

**Restore default widgets** returns the current page to its workspace's default
widgets (see the table above) with fresh widget IDs. It does not change the test
plan.

## Your own pages

In **Layouts…**, enter a name and choose **Create blank page** or **Duplicate
current page**. Names are unique. Rename and delete controls are available; at
least one page must remain. A page may intentionally have no widgets.

**Dashboard only (hide request editor)** creates a full analytics page. New blank
pages use this mode. Add charts, tables and logs from the catalog; use the same
drag handles and width controls. Dashboard row height is configurable. On a narrow
window the grid uses fewer columns without changing the stored widths.

Disable Dashboard only to use the split Studio 2 editors with optional Bottom and
Right docks. Changing mode preserves widget identity and the workbench dock
assignments. Use **Configure test in Studio** to edit and authorize targets when
viewing a pure dashboard. Switching pages, hiding widgets and editing layouts do
not modify or stop the active run.

## Catalog and data

Ten native widgets are available in every workspace: throughput, requests per
second (Total/Success/Failed, Monitor's chart), P50/P95/P99 latency, live logs,
active users, HTTP status distribution, endpoint breakdown, the live KPI group,
KPI cards (Monitor's eight cards: users, RPS, data/sec, P50/P95/P99, error rate,
request count) and the latency distribution. The distribution comes from the whole-run histogram as
`Snapshot.latency_distribution`: at most 40 log-spaced buckets between the fastest
and slowest recorded response, so every widget instance reads the same bounded
copy. Layouts saved by an older version that lacks a widget open it as an
unavailable widget, which normalization removes.
Multiple instances are allowed. Each graph has an independent plot UI identity
(workspace + page + widget). In Studio and Studio 2 they read the current run's
real aggregate data; in Monitor they read the run Monitor has open, the current
run or a history run. A history run saved before the latency distribution
existed shows "No latency distribution recorded for this run". Log filtering/follow controls
are shared across log widgets in this first implementation.

Limits per workspace: 20 named layouts, 16 widgets per layout, 1–60-character
names. This is a
built-in widget catalog; external data sources, user-defined queries, arbitrary
plugins and individual pixel positioning are outside this version.

## Saving

**Save layout** immediately flushes the UI preferences on this device. Normal
eframe autosave and exit also retain them. Pages, active page, visibility, order,
spans and sizes persist separately from request/auth data. Saving a test plan JSON
does not export personal layouts. No HTTP body, token or test history is added to
the layout model.

Studio 2 pages keep their original `dashboards` preference key; Monitor and
Studio pages are stored in `monitor_dashboards` and `studio_dashboards`.
Preferences saved before these keys existed open with Studio 2 pages unchanged
and the Monitor/Studio default pages. Old preferences without any layouts receive
the standard Studio 2 layout. Normalization also moves a widget whose dock the
workspace does not offer to its first dock. Restoration
normalizes missing/duplicate IDs, unknown widget kinds and out-of-range sizes.
Stable IDs are not reused after deleting a widget/page, preventing stale plot
state from attaching to an unrelated new widget.

Screenshots: [custom dashboard](screenshots/widgets-dashboard.png),
[edit mode](screenshots/widgets-edit.png), [manager](screenshots/widgets-layouts.png),
[all analytics hidden](screenshots/widgets-focus.png).

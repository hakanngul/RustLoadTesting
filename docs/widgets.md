# Personal layouts and widget dashboards

Open **Studio 2**. The layout selector, **+ Widget**, **Edit layout**, **Layouts…**
and **Save layout** controls are in the toolbar. Studio and Monitor keep their
existing workflows; these settings belong to the user interface, not the TestPlan.

## Hide, restore and arrange

1. Turn on **Edit layout**. Each widget gets a `::` drag handle, a menu and `×`.
2. `×` hides the widget. Restore it with its checkbox in **Layouts…**. **Remove**
   in the manager permanently removes that widget's configuration from this page.
3. Drag a handle onto another widget to insert before it. Menu/manager arrows are
   keyboard-friendly alternatives for moving earlier/later.
4. In a workbench layout, move widgets between Bottom and Right. Hidden widgets
   retain their position and width. If a dock has no visible widgets it disappears,
   returning the space to the request editors.
5. Small, Medium and Full row set grid spans. Partly-filled rows expand to use the
   entire width; hiding one of three small widgets leaves two equal-width widgets.
   Workbench right widgets share the resizable sidebar width; its span control is
   disabled. Drag the bottom panel border to change its height.

**Restore default widgets** returns the current page to throughput + latency +
logs below and live KPI cards on the right. It does not change the test plan.

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

Seven native widgets are available: throughput, P50/P95/P99 latency, live logs,
active users, HTTP status distribution, endpoint breakdown and the live KPI group.
Multiple instances are allowed. Each graph has an independent plot UI identity.
They all read the current run's real aggregate data. Log filtering/follow controls
are shared across log widgets in this first implementation.

Limits: 20 named layouts, 16 widgets per layout, 1–60-character names. This is a
built-in widget catalog; external data sources, user-defined queries, arbitrary
plugins and individual pixel positioning are outside this version.

## Saving

**Save layout** immediately flushes the UI preferences on this device. Normal
eframe autosave and exit also retain them. Pages, active page, visibility, order,
spans and sizes persist separately from request/auth data. Saving a test plan JSON
does not export personal layouts. No HTTP body, token or test history is added to
the layout model.

Old preferences without layouts receive the standard Studio 2 layout. Restoration
normalizes missing/duplicate IDs, unknown widget kinds and out-of-range sizes.
Stable IDs are not reused after deleting a widget/page, preventing stale plot
state from attaching to an unrelated new widget.

Screenshots: [custom dashboard](screenshots/widgets-dashboard.png),
[edit mode](screenshots/widgets-edit.png), [manager](screenshots/widgets-layouts.png),
[all analytics hidden](screenshots/widgets-focus.png).

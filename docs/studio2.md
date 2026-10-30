# Studio 2 — alternative workbench

Select **Studio 2** in the top navigation. The original Studio and Monitor remain
available. The chosen workspace, like the theme, is retained across restarts.

The layout below is the default. **Edit layout / + Widget / Layouts…** now allow
personal widget visibility, order, sizes and named dashboard-only pages. All
analytics can be hidden to reclaim editor space. Studio and Monitor have their own,
independent widget pages built on the same system. See [personal layouts](widgets.md).

Studio 2 uses the same TestPlan, selected request, variables, auth, load settings,
engine handle and current-run measurements as Studio. It does not create a second
plan or a separate load engine. Editing in one layout is reflected in the other.

## Reference layout

- Left: expandable plan/users/requests/assertions/CSV tree. Request context menus
  reorder, duplicate and delete; assertion items open the selected request's
  assertions; CSV/variable items open the shared plan settings.
- Center, above: compact Name and Method/URL form, Params/Headers/Auth/Assertions/
  Extractors/Advanced tabs. The horizontal divider resizes this editor. Name, URL
  and tabs stay in place and only the tab content scrolls; when the editor is
  too short for that, the whole editor scrolls.
- Center, below: Body and Load Configuration together, with target authorization.
  The authorization box stays visible below the load settings, which scroll.
  Think time, timeout and response cap are under the **Timeout, think time & body limit** section.
- Right: active/target VUs, five-second completion RPS, whole-run P95, error rate,
  average response time and real interval sparklines. No fabricated delta badges.
  When the plan has SLO thresholds, the verdict and per-threshold results appear
  above the cards; the tree lists thresholds and opens the shared editor.
- Bottom: throughput, latency and filtered bounded logs together. Drag its upper
  edge to resize. All of these read current-run data, even if a historical result
  was previously selected in Monitor.

Run from Studio 2 keeps this layout open; Run from Studio retains the original
automatic Monitor transition. Editing is locked during execution, Stop stays
available, and moving between layouts does not start or stop another run.

On smaller windows the right-dock widgets (Live Metrics by default) collapse under
**Dashboard widgets**, Body/Load become tabs and the bottom charts/logs become tabs. Long tree labels truncate rather than expanding
the sidebar. Themes and metrics retain the same semantic tokens.

The tree represents the existing sequential scenario model. Visual folders do not
introduce nested execution groups, a Flow editor, extra CSV sources or cookie
support. Those require separate engine/model features.

Screenshots: [Dark](screenshots/studio2-dark.png),
[Light](screenshots/studio2-light.png), [Compact](screenshots/studio2-compact.png).

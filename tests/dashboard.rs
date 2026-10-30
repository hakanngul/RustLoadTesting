use rustload::dashboard::*;
use rustload::ui::Preferences;

#[test]
fn hiding_every_widget_frees_both_docks_and_is_restorable() {
    let mut page = DashboardPage::default();
    let saved = page.widgets.clone();
    for w in saved {
        page.hide(w.id);
    }
    assert!(page.visible(Dock::Bottom).is_empty());
    assert!(page.visible(Dock::Right).is_empty());
    assert_eq!(page.widgets.len(), 4);
    page.widgets[2].visible = true;
    assert_eq!(page.visible(Dock::Bottom)[0].kind, WidgetKind::Logs);
}

#[test]
fn cross_dock_moves_preserve_identity_visibility_and_span() {
    let mut page = DashboardPage::default();
    page.widgets[0].span = Span::Medium;
    page.place(1, Dock::Right, Some(4));
    let right = page.visible(Dock::Right);
    assert_eq!(right.iter().map(|w| w.id).collect::<Vec<_>>(), [1, 4]);
    assert_eq!(right[0].span, Span::Medium);
    assert_eq!(
        page.visible(Dock::Bottom)
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    page.place(4, Dock::Bottom, Some(2));
    assert_eq!(page.visible(Dock::Bottom)[0].id, 4);
    let before = page.clone();
    page.place(4, Dock::Bottom, Some(4));
    page.place(4, Dock::Right, Some(999));
    page.place(999, Dock::Bottom, None);
    assert_eq!(page, before);
}

#[test]
fn reordering_is_confined_to_its_dock_and_hidden_settings_survive() {
    let mut page = DashboardPage::default();
    page.hide(2);
    page.shift(1, true);
    assert_eq!(
        page.widgets.iter().map(|w| w.id).collect::<Vec<_>>(),
        [2, 1, 3, 4]
    );
    assert!(!page.widgets[0].visible);
    page.shift(4, false);
    assert_eq!(page.widgets[3].id, 4);
    page.shift(1, false);
    assert_eq!(page.widgets[0].id, 1);
}

#[test]
fn named_pages_round_trip_sizes_order_and_hidden_widgets() {
    let mut all = Dashboards::default();
    all.active_mut().bottom_height = 340.0;
    all.active_mut().right_width = 420.0;
    all.active_mut().hide(3);
    all.active_mut().place(2, Dock::Bottom, Some(1));
    all.create("Performance", true).unwrap();
    all.active_mut()
        .add(WidgetKind::ActiveUsers, Dock::Bottom)
        .unwrap();
    assert_eq!(all.pages[0].widgets.len(), 4);
    assert_eq!(all.active().widgets.len(), 5);
    let serialized = serde_json::to_string(&all).unwrap();
    let mut restored: Dashboards = serde_json::from_str(&serialized).unwrap();
    restored.normalize();
    assert_eq!(all, restored);
    assert_eq!(restored.active().bottom_height, 340.0);
    assert!(
        !restored
            .active()
            .widgets
            .iter()
            .find(|w| w.kind == WidgetKind::Logs)
            .unwrap()
            .visible
    );
    restored.create("Debug", false).unwrap();
    assert!(restored.active().widgets.is_empty());
    assert!(restored.create(" debug ", false).is_err());
    assert!(restored.create("", false).is_err());
    restored.rename_active("Debugging").unwrap();
    assert!(restored.rename_active("Performance").is_err());
}

#[test]
fn deleted_ids_are_not_reused_for_ui_state() {
    let mut all = Dashboards::default();
    all.create("Temporary", false).unwrap();
    let deleted_page_id = all.active_id;
    all.delete_active().unwrap();
    all.create("Replacement", false).unwrap();
    assert!(all.active_id > deleted_page_id);
    let page = all.active_mut();
    let removed = page.add(WidgetKind::Logs, Dock::Bottom).unwrap();
    page.remove(removed);
    let replacement = page.add(WidgetKind::Latency, Dock::Bottom).unwrap();
    assert!(replacement > removed);
}

#[test]
fn invalid_saved_configuration_is_repaired_without_panics() {
    let mut all = Dashboards {
        active_id: 999,
        ..Dashboards::default()
    };
    all.pages.push(all.pages[0].clone());
    all.pages[0].bottom_height = f32::NAN;
    all.pages[0].right_width = 10000.0;
    all.pages[0].widgets[0].kind = WidgetKind::Unknown;
    all.pages[0].widgets[2].id = all.pages[0].widgets[1].id;
    all.normalize();
    assert_eq!(all.active_id, 1);
    assert_ne!(all.pages[0].id, all.pages[1].id);
    assert_eq!(all.active().bottom_height, 255.0);
    assert_eq!(all.active().right_width, 520.0);
    let ids: std::collections::BTreeSet<_> = all.active().widgets.iter().map(|w| w.id).collect();
    assert_eq!(ids.len(), all.active().widgets.len());
    all.pages.clear();
    all.normalize();
    assert_eq!(all, Dashboards::default());
}

#[test]
fn widget_catalog_bounds_and_unknown_widgets_are_handled() {
    let mut page = DashboardPage::default();
    assert!(page.add(WidgetKind::Unknown, Dock::Bottom).is_err());
    while page.widgets.len() < MAX_WIDGETS {
        page.add(WidgetKind::Logs, Dock::Bottom).unwrap();
    }
    let before = page.clone();
    assert!(page.add(WidgetKind::Logs, Dock::Bottom).is_err());
    assert_eq!(page, before);
    let kind: WidgetKind = serde_json::from_str("\"FutureWidget\"").unwrap();
    assert_eq!(kind, WidgetKind::Unknown);
}

#[test]
fn row_packing_supports_resizing_and_hiding_without_blank_slots() {
    let mut page = DashboardPage::default();
    assert_eq!(rows(&page.visible(Dock::Bottom))[0].len(), 3);
    page.hide(2);
    assert_eq!(rows(&page.visible(Dock::Bottom))[0].len(), 2);
    page.widgets[0].span = Span::Full;
    let packed = rows(&page.visible(Dock::Bottom));
    assert_eq!(packed.len(), 2);
    assert!(
        packed
            .iter()
            .all(|r| r.iter().map(|w| w.span.units()).sum::<u32>() <= 12)
    );
    assert!(rows(&[]).is_empty());
}

#[test]
fn canvas_reorders_all_widgets_without_losing_workbench_docks() {
    let mut page = DashboardPage {
        dashboard_only: true,
        ..DashboardPage::default()
    };
    page.place(4, Dock::Bottom, Some(1));
    assert_eq!(
        page.widgets.iter().map(|w| w.id).collect::<Vec<_>>(),
        [4, 1, 2, 3]
    );
    assert_eq!(page.widgets[0].dock, Dock::Right);
    page.shift(4, true);
    assert_eq!(
        page.widgets.iter().map(|w| w.id).collect::<Vec<_>>(),
        [1, 4, 2, 3]
    );
    page.dashboard_only = false;
    assert_eq!(page.visible(Dock::Right)[0].id, 4);
    assert_eq!(
        page.visible(Dock::Bottom)
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn latency_distribution_widget_is_in_the_catalog() {
    assert!(WidgetKind::ALL.contains(&WidgetKind::LatencyHistogram));
    let mut page = DashboardPage::default();
    let id = page.add(WidgetKind::LatencyHistogram, Dock::Right).unwrap();
    let json = serde_json::to_string(&page).unwrap();
    let restored: DashboardPage = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored.widgets.iter().find(|w| w.id == id).unwrap().kind,
        WidgetKind::LatencyHistogram
    );
}

fn layout(page: &DashboardPage) -> Vec<(WidgetKind, Dock, Span, bool)> {
    page.widgets
        .iter()
        .map(|w| (w.kind, w.dock, w.span, w.visible))
        .collect()
}

#[test]
fn workspace_defaults_reproduce_the_built_in_layouts() {
    use {Dock::*, Span::*, WidgetKind::*};
    assert_eq!(Workspace::Studio2.default_page(), DashboardPage::default());
    assert_eq!(
        Workspace::Studio2.default_dashboards(),
        Dashboards::default()
    );
    let monitor = Workspace::Monitor.default_page();
    assert!(!monitor.dashboard_only);
    assert_eq!(
        layout(&monitor),
        [
            (KpiCards, Main, Full, true),
            (RequestsPerSecond, Main, Medium, true),
            (Latency, Main, Medium, true),
            (StatusCodes, Main, Medium, true),
            (ActiveUsers, Main, Medium, true),
            (Endpoints, Main, Full, true),
            (Logs, Bottom, Full, true),
        ]
    );
    let studio = Workspace::Studio.default_page();
    assert_eq!(
        layout(&studio),
        [
            (RequestsPerSecond, Bottom, Medium, true),
            (Latency, Bottom, Medium, true),
        ]
    );
    for ws in Workspace::ALL {
        let mut normalized = ws.default_dashboards();
        normalized.normalize_for(ws);
        assert_eq!(normalized, ws.default_dashboards(), "{ws:?}");
        assert!(
            ws.default_page()
                .widgets
                .iter()
                .all(|w| ws.docks().contains(&w.dock))
        );
    }
}

#[test]
fn old_preferences_keep_studio2_layouts_and_gain_workspace_defaults() {
    // Shape written before per-workspace layouts: only the Studio 2 set exists.
    let old = r#"{"light":true,"plan_path":"plan.json","workspace":"Studio2",
        "dashboards":{"active_id":2,"next_page_id":3,"pages":[
          {"id":1,"name":"Studio 2","widgets":[
            {"id":1,"kind":"Throughput","dock":"Bottom","visible":true,"span":"Small"},
            {"id":3,"kind":"Logs","dock":"Bottom","visible":false,"span":"Small"},
            {"id":5,"kind":"LatencyHistogram","dock":"Right","visible":true,"span":"Medium"}],
           "bottom_height":300.0,"right_width":400.0,"next_widget_id":6,
           "dashboard_only":false,"card_height":260.0},
          {"id":2,"name":"Perf","widgets":[],"bottom_height":255.0,"right_width":315.0,
           "next_widget_id":5,"dashboard_only":true,"card_height":320.0}]},
        "compare_noise_pct":null}"#;
    let mut prefs: Preferences = serde_json::from_str(old).unwrap();
    let studio2 = prefs.dashboards.clone();
    prefs.normalize();
    assert_eq!(prefs.dashboards, studio2);
    assert_eq!(prefs.dashboards.active().name, "Perf");
    assert!(prefs.dashboards.active().dashboard_only);
    assert_eq!(
        prefs.monitor_dashboards,
        Workspace::Monitor.default_dashboards()
    );
    assert_eq!(
        prefs.studio_dashboards,
        Workspace::Studio.default_dashboards()
    );
    assert_eq!(prefs.workspace, Workspace::Studio2);
    // A fresh install gets every workspace's default set.
    let fresh = Preferences::default();
    for ws in Workspace::ALL {
        assert_eq!(fresh.dashboards_for(ws), &ws.default_dashboards());
    }
    // Saved layouts never carry plan, auth or request data.
    let saved = serde_json::to_string(&prefs).unwrap();
    for field in ["url", "header", "auth", "token", "body"] {
        assert!(!saved.to_ascii_lowercase().contains(field), "{field}");
    }
}

#[test]
fn workspace_page_sets_are_independent() {
    let mut prefs = Preferences::default();
    let studio2 = prefs.dashboards.clone();
    let studio = prefs.studio_dashboards.clone();
    let monitor = prefs.dashboards_for_mut(Workspace::Monitor);
    monitor
        .active_mut()
        .add(WidgetKind::LatencyHistogram, Dock::Main)
        .unwrap();
    monitor.active_mut().hide(7);
    monitor
        .create_for(Workspace::Monitor, "Errors", false)
        .unwrap();
    assert_eq!(prefs.dashboards, studio2);
    assert_eq!(prefs.studio_dashboards, studio);
    let restored: Preferences =
        serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
    assert_eq!(restored, prefs);
    let first = &restored.monitor_dashboards.pages[0];
    assert_eq!(
        first.widgets.last().unwrap().kind,
        WidgetKind::LatencyHistogram
    );
    assert!(!first.widgets.iter().find(|w| w.id == 7).unwrap().visible);
    // Blank Monitor/Studio pages stay workbench pages beside the fixed areas.
    let blank = restored.monitor_dashboards.active();
    assert_eq!(blank.name, "Errors");
    assert!(blank.widgets.is_empty() && !blank.dashboard_only);
}

#[test]
fn normalization_fits_pages_to_their_workspace() {
    let mut studio = Workspace::Studio2.default_dashboards();
    studio.active_mut().dashboard_only = true;
    studio
        .active_mut()
        .add(WidgetKind::KpiCards, Dock::Main)
        .unwrap();
    let ids: Vec<_> = studio.active().widgets.iter().map(|w| w.id).collect();
    let mut as_studio2 = studio.clone();
    as_studio2.normalize_for(Workspace::Studio2);
    assert!(as_studio2.active().dashboard_only);
    assert!(
        as_studio2
            .active()
            .widgets
            .iter()
            .all(|w| w.dock != Dock::Main)
    );
    studio.normalize_for(Workspace::Studio);
    // Studio keeps Load Configuration on the right: Bottom only, no dashboard-only.
    assert!(!studio.active().dashboard_only);
    assert!(
        studio
            .active()
            .widgets
            .iter()
            .all(|w| w.dock == Dock::Bottom)
    );
    assert_eq!(
        studio
            .active()
            .widgets
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>(),
        ids
    );
    let mut empty = Dashboards {
        pages: vec![],
        ..Dashboards::default()
    };
    empty.normalize_for(Workspace::Monitor);
    assert_eq!(empty, Workspace::Monitor.default_dashboards());
}

#[test]
fn new_kinds_round_trip_and_restore_defaults_uses_fresh_ids() {
    let mut page = Workspace::Monitor.default_page();
    page.remove(1);
    let json = serde_json::to_string(&page).unwrap();
    assert!(json.contains("\"RequestsPerSecond\"") && !json.contains("\"KpiCards\""));
    let mut restored: DashboardPage = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, page);
    restored.reset_widgets(Workspace::Monitor.default_page().widgets);
    assert_eq!(
        layout(&restored),
        layout(&Workspace::Monitor.default_page())
    );
    assert!(
        restored.widgets.iter().all(|w| w.id >= 8),
        "removed IDs are not reused"
    );
    let ids: std::collections::BTreeSet<_> = restored.widgets.iter().map(|w| w.id).collect();
    assert_eq!(ids.len(), restored.widgets.len());
    // An older build that lacks a kind normalizes it away.
    let mut future: DashboardPage =
        serde_json::from_str(&json.replacen("RequestsPerSecond", "SomeFutureWidget", 1)).unwrap();
    future.normalize();
    assert!(future.widgets.iter().all(|w| w.kind != WidgetKind::Unknown));
    assert_eq!(future.widgets.len(), page.widgets.len() - 1);
}

#[test]
fn monitor_pages_keep_limits_and_main_area_ordering() {
    let mut all = Workspace::Monitor.default_dashboards();
    for i in 1..MAX_PAGES {
        all.create_for(Workspace::Monitor, &format!("Page {i}"), true)
            .unwrap();
    }
    assert!(
        all.create_for(Workspace::Monitor, "One more", false)
            .is_err()
    );
    let page = all.active_mut();
    // Reordering stays within the dock; Logs in Bottom is unaffected.
    page.shift(6, false);
    assert_eq!(
        page.visible(Dock::Main)
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 6, 5]
    );
    page.place(7, Dock::Main, Some(1));
    assert_eq!(page.visible(Dock::Main)[0].kind, WidgetKind::Logs);
    assert!(page.visible(Dock::Bottom).is_empty());
    while page.widgets.len() < MAX_WIDGETS {
        page.add(WidgetKind::KpiCards, Dock::Right).unwrap();
    }
    assert!(page.add(WidgetKind::Logs, Dock::Main).is_err());
}

use rustload::dashboard::*;

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
fn latency_distribution_widget_is_an_eighth_catalog_entry() {
    assert_eq!(WidgetKind::ALL.len(), 8);
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

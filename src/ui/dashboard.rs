//! Widget catalog, named user layouts and editing UI shared by every workspace.
//! Each workspace (Studio, Studio 2, Monitor) owns its own page set; all of them
//! use this single implementation.
use super::*;

#[derive(Clone, Copy)]
struct DragWidget {
    workspace: Workspace,
    page: u64,
    widget: u64,
}
#[derive(Clone, Copy)]
enum Edit {
    Hide(u64),
    Remove(u64),
    Place(u64, Dock, Option<u64>),
    Shift(u64, bool),
    Span(u64, Span),
}

impl RustLoadApp {
    /// Studio 2's dashboard-only page: the whole central area is the grid.
    pub(super) fn dashboard_canvas(&mut self, ctx: &egui::Context, p: Palette) {
        let ws = Workspace::Studio2;
        let page = self.prefs.dashboards_for(ws).active().clone();
        let mut edits = vec![];
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(&page.name);
                ui.colored_label(p.success,format!("{:?} · {:.0}s",self.live.state,self.live.elapsed));
                if ui.button("Configure test in Studio").clicked() { self.workspace = Workspace::Studio; self.prefs.workspace = Workspace::Studio; }
            });
            let widgets:Vec<_> = page.widgets.iter().filter(|w|w.visible).cloned().collect();
            if widgets.is_empty() {
                ui.add_space(24.0);
                ui.heading("Build your own dashboard");
                ui.label("Use + Widget or Layouts to add charts, metrics and logs. Configure and authorize test targets in Studio.");
                if ui.button("Open widget catalog").clicked() { self.layout_manager_open = true; }
            }
            egui::ScrollArea::vertical().id_salt(("dashboard_canvas",ws,page.id)).show(ui, |ui| {
                self.dashboard_grid(ui, p, ws, &page, widgets, &mut edits);
            });
        });
        self.apply_dashboard_edits(ws, edits);
    }

    /// The Main-area grid of a workbench page (Monitor), drawn inside `ui`.
    pub(super) fn dashboard_main(&mut self, ui: &mut egui::Ui, p: Palette, ws: Workspace) {
        let page = self.prefs.dashboards_for(ws).active().clone();
        let widgets = page.visible(Dock::Main);
        if widgets.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "No widgets in the main area. Use + Widget or Layouts… to add charts, metrics and logs.",
                )
                .color(p.muted),
            );
            return;
        }
        let mut edits = vec![];
        self.dashboard_grid(ui, p, ws, &page, widgets, &mut edits);
        self.apply_dashboard_edits(ws, edits);
    }

    /// Twelve-column rows of tiles. Narrow windows widen spans for display only.
    fn dashboard_grid(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        ws: Workspace,
        page: &DashboardPage,
        mut widgets: Vec<Widget>,
        edits: &mut Vec<Edit>,
    ) {
        // Keep cards readable on smaller windows without changing the saved widths.
        if ui.available_width() < 600.0 {
            for widget in &mut widgets {
                widget.span = Span::Full;
            }
        } else if ui.available_width() < 1000.0 {
            for widget in &mut widgets {
                if widget.span == Span::Small {
                    widget.span = Span::Medium;
                }
            }
        }
        for row in crate::dashboard::rows(&widgets) {
            let units = row.iter().map(|w| w.span.units()).sum::<u32>() as f32;
            let width = ui.available_width() - 8.0 * row.len().saturating_sub(1) as f32;
            let height = row
                .iter()
                .map(|w| {
                    self.tile_height(
                        w.kind,
                        width * w.span.units() as f32 / units,
                        page.card_height,
                    )
                })
                .fold(0.0, f32::max);
            ui.horizontal_top(|ui| {
                for widget in row {
                    ui.allocate_ui_with_layout(
                        egui::vec2(width * widget.span.units() as f32 / units, height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.dashboard_tile(ui, p, ws, page, &widget, height, edits),
                    );
                }
            });
            ui.add_space(8.0);
        }
    }

    /// Tile height in a grid row or the right dock. KPI cards size to their
    /// wrapped card rows instead of the page's row height.
    fn tile_height(&self, kind: WidgetKind, width: f32, default: f32) -> f32 {
        let edit = if self.layout_edit { 40.0 } else { 0.0 };
        match kind {
            WidgetKind::KpiCards => {
                let rows = KPI_CARDS.div_ceil(kpi_columns(width - 8.0)) as f32;
                rows * 92.0 + 10.0 + edit
            }
            _ => default,
        }
    }

    pub(super) fn dashboard_toolbar(&mut self, ui: &mut egui::Ui, _p: Palette, ws: Workspace) {
        // Keep the layout controls together; a combo box does not wrap on its own.
        if ui.available_size_before_wrap().x < 540.0 {
            ui.end_row();
        } else {
            ui.separator();
        }
        let all = self.prefs.dashboards_for_mut(ws);
        egui::ComboBox::from_id_salt(("dashboard_page", ws))
            .width(130.0)
            .selected_text(format!("Layout: {}", all.active().name))
            .show_ui(ui, |ui| {
                for page in &all.pages {
                    ui.selectable_value(&mut all.active_id, page.id, &page.name);
                }
            });
        let docks = ws.docks();
        ui.menu_button("+ Widget", |ui| {
            let dashboard_only = self.prefs.dashboards_for(ws).active().dashboard_only;
            if dashboard_only || docks.len() == 1 {
                for kind in WidgetKind::ALL {
                    if ui.button(kind.label()).clicked() {
                        if let Err(e) = self
                            .prefs
                            .dashboards_for_mut(ws)
                            .active_mut()
                            .add(kind, docks[0])
                        {
                            self.message = e;
                        }
                        ui.close();
                    }
                }
            } else {
                for &dock in docks {
                    ui.menu_button(format!("Add to {}", dock.label()), |ui| {
                        for kind in WidgetKind::ALL {
                            if ui.button(kind.label()).clicked() {
                                if let Err(e) = self
                                    .prefs
                                    .dashboards_for_mut(ws)
                                    .active_mut()
                                    .add(kind, dock)
                                {
                                    self.message = e;
                                }
                                ui.close();
                            }
                        }
                    });
                }
            }
            ui.separator();
            if ui.button("Show hidden widgets / manage layouts…").clicked() {
                self.layout_manager_open = true;
                ui.close();
            }
        });
        ui.toggle_value(&mut self.layout_edit, "Edit layout");
        if ui.button("Layouts…").clicked() {
            self.layout_manager_open = true;
        }
        if ui.button("Save layout").clicked() {
            self.layout_save_requested = true;
        }
    }

    pub(super) fn dashboard_bottom(&mut self, ctx: &egui::Context, p: Palette, ws: Workspace) {
        let page = self.prefs.dashboards_for(ws).active().clone();
        let widgets = page.visible(Dock::Bottom);
        if widgets.is_empty() {
            return;
        } // Give the entire area back to the editors.
        let mut edits = vec![];
        let result = egui::TopBottomPanel::bottom(egui::Id::new(("dashboard_bottom", ws, page.id)))
            .resizable(true)
            .default_height(page.bottom_height)
            .height_range(170.0..=450.0)
            .show(ctx, |ui| {
                let height = ui.available_height();
                ui.set_min_height(height);
                // Studio's strip sits between fixed side panels, so it keeps its
                // original window-width rule; full-width docks use their own width.
                let narrow = if ws == Workspace::Studio {
                    ctx.content_rect().width() < 1150.0
                } else {
                    ui.available_width() < 1200.0
                };
                if narrow && widgets.len() > 1 {
                    let mut selected = self
                        .bottom_tabs
                        .get(&ws)
                        .copied()
                        .filter(|id| widgets.iter().any(|w| w.id == *id))
                        .unwrap_or(widgets[0].id);
                    let tabs = ui.horizontal_wrapped(|ui| {
                        for widget in &widgets {
                            ui.selectable_value(&mut selected, widget.id, widget.kind.label());
                        }
                    });
                    let tabs_height = tabs.response.rect.height() + ui.spacing().item_spacing.y;
                    self.bottom_tabs.insert(ws, selected);
                    let widget = widgets
                        .iter()
                        .find(|w| w.id == selected)
                        .expect("selected visible widget");
                    self.dashboard_tile(
                        ui,
                        p,
                        ws,
                        &page,
                        widget,
                        (height - tabs_height).max(70.0),
                        &mut edits,
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt(("dashboard_bottom_scroll", ws, page.id))
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for row in crate::dashboard::rows(&widgets) {
                                let total = row.iter().map(|w| w.span.units()).sum::<u32>() as f32;
                                let available =
                                    ui.available_width() - 8.0 * row.len().saturating_sub(1) as f32;
                                ui.horizontal_top(|ui| {
                                    for widget in row {
                                        let width = available * widget.span.units() as f32 / total;
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(width, height),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                self.dashboard_tile(
                                                    ui, p, ws, &page, &widget, height, &mut edits,
                                                );
                                            },
                                        );
                                    }
                                });
                            }
                        });
                }
            });
        self.prefs.dashboards_for_mut(ws).active_mut().bottom_height =
            result.response.rect.height().clamp(170.0, 450.0);
        self.apply_dashboard_edits(ws, edits);
    }

    pub(super) fn dashboard_right(
        &mut self,
        ctx: &egui::Context,
        p: Palette,
        ws: Workspace,
        compact: bool,
    ) {
        if compact {
            return;
        }
        let page = self.prefs.dashboards_for(ws).active().clone();
        let widgets = page.visible(Dock::Right);
        if widgets.is_empty() {
            return;
        }
        let mut edits = vec![];
        let response = egui::SidePanel::right(egui::Id::new(("dashboard_right", ws, page.id)))
            .resizable(true)
            .default_width(page.right_width)
            .width_range(275.0..=520.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("dashboard_right_scroll", ws, page.id))
                    .show(ui, |ui| {
                        for widget in &widgets {
                            let height = if widget.kind == WidgetKind::LiveMetrics {
                                585.0
                            } else {
                                self.tile_height(widget.kind, ui.available_width(), 260.0)
                            };
                            self.dashboard_tile(ui, p, ws, &page, widget, height, &mut edits);
                            ui.add_space(8.0);
                        }
                    });
            });
        self.prefs.dashboards_for_mut(ws).active_mut().right_width =
            response.response.rect.width().clamp(275.0, 520.0);
        self.apply_dashboard_edits(ws, edits);
    }

    pub(super) fn dashboard_compact_right(&mut self, ui: &mut egui::Ui, p: Palette, ws: Workspace) {
        let page = self.prefs.dashboards_for(ws).active().clone();
        let widgets = page.visible(Dock::Right);
        if widgets.is_empty() {
            return;
        }
        let mut edits = vec![];
        egui::CollapsingHeader::new("Dashboard widgets")
            .id_salt(("compact_widgets", ws, page.id))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("compact_widgets_scroll", ws, page.id))
                    .max_height((ui.available_height() * 0.45).clamp(80.0, 220.0))
                    .show(ui, |ui| {
                        for widget in &widgets {
                            let height = self.tile_height(widget.kind, ui.available_width(), 280.0);
                            self.dashboard_tile(ui, p, ws, &page, widget, height, &mut edits);
                        }
                    });
            });
        self.apply_dashboard_edits(ws, edits);
    }

    #[allow(clippy::too_many_arguments)]
    fn dashboard_tile(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        ws: Workspace,
        page: &DashboardPage,
        widget: &Widget,
        height: f32,
        edits: &mut Vec<Edit>,
    ) {
        let editing = self.layout_edit;
        ui.push_id(("dashboard_tile", ws, page.id, widget.id), |ui| {
            let mut body = |ui: &mut egui::Ui| {
                let edit_height = if self.layout_edit {
                    ui.horizontal(|ui| {
                        ui.dnd_drag_source(
                            egui::Id::new(("widget_drag", ws, page.id, widget.id)),
                            DragWidget {
                                workspace: ws,
                                page: page.id,
                                widget: widget.id,
                            },
                            |ui| {
                                ui.label("::")
                                    .on_hover_text("Drag onto another widget to move before it");
                            },
                        );
                        ui.label(RichText::new(widget.kind.label()).strong().size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("×")
                                .on_hover_text("Hide widget (restore in Layouts)")
                                .clicked()
                            {
                                edits.push(Edit::Hide(widget.id));
                            }
                            ui.menu_button("…", |ui| {
                                if !page.dashboard_only {
                                    for &dock in ws.docks().iter().filter(|d| **d != widget.dock) {
                                        if ui.button(format!("Move to {}", dock.label())).clicked()
                                        {
                                            edits.push(Edit::Place(widget.id, dock, None));
                                            ui.close();
                                        }
                                    }
                                }
                                ui.separator();
                                if page.dashboard_only || widget.dock != Dock::Right {
                                    for span in [Span::Small, Span::Medium, Span::Full] {
                                        if ui
                                            .selectable_label(widget.span == span, span.label())
                                            .clicked()
                                        {
                                            edits.push(Edit::Span(widget.id, span));
                                            ui.close();
                                        }
                                    }
                                }
                                ui.separator();
                                if ui.button("Move earlier").clicked() {
                                    edits.push(Edit::Shift(widget.id, false));
                                    ui.close();
                                }
                                if ui.button("Move later").clicked() {
                                    edits.push(Edit::Shift(widget.id, true));
                                    ui.close();
                                }
                            });
                        });
                    });
                    32.0
                } else {
                    0.0
                };
                egui::ScrollArea::both()
                    .id_salt("widget_viewport")
                    .max_height((height - edit_height - 8.0).max(40.0))
                    // Main-area rows (Monitor) take their tallest content, as the
                    // fixed Monitor layout did; docks and Studio 2 tiles fill.
                    .auto_shrink([false, widget.dock == Dock::Main && !page.dashboard_only])
                    .show(ui, |ui| {
                        // Charts and logs fill the viewport whatever their
                        // header wraps to.
                        let body = (height - edit_height - 12.0).max(120.0);
                        self.widget_body(ui, p, ws, widget.kind, body);
                    });
            };
            let tile = ui.max_rect();
            if editing {
                let (_, dropped) = ui.dnd_drop_zone::<DragWidget, _>(
                    egui::Frame::new().inner_margin(3).corner_radius(8),
                    &mut body,
                );
                if let Some(payload) = dropped
                    && payload.workspace == ws
                    && payload.page == page.id
                    && payload.widget != widget.id
                {
                    edits.push(Edit::Place(payload.widget, widget.dock, Some(widget.id)));
                }
            } else {
                body(ui);
                // Shown while the pointer is over the tile, in its own layer so
                // the widget's scroll area or plot cannot take the click. Modal
                // windows and menus sit above that layer. It sits on the tile's
                // top border, across the frame margin, so it never covers the
                // widget's own controls (Follow, Reset zoom, series toggles).
                let area = egui::Id::new(("expand_widget", ws, page.id, widget.id));
                let button = ui.ctx().memory(|m| m.area_rect(area));
                let over_tile = ui.ctx().pointer_hover_pos().is_some_and(|pos| {
                    ((tile.contains(pos) && ui.clip_rect().contains(pos))
                        || button.is_some_and(|b| b.contains(pos)))
                        // Not while a window, menu or the button covers it.
                        && ui.ctx().layer_id_at(pos).is_some_and(|layer| {
                            layer == ui.layer_id() || layer.id == area
                        })
                });
                if over_tile {
                    // Kept inside the visible part when the tile is scrolled.
                    let top = tile.top().max(ui.clip_rect().top() + 8.0);
                    let shown = egui::Area::new(area)
                        .order(egui::Order::Middle)
                        .pivot(egui::Align2::RIGHT_CENTER)
                        .fixed_pos(egui::pos2(tile.right() - 14.0, top))
                        .show(ui.ctx(), |ui| {
                            let expand =
                                egui::Button::new(RichText::new("Expand").size(11.0).color(p.text))
                                    .small()
                                    .fill(p.elevated)
                                    .stroke((1.0, p.border))
                                    .corner_radius(6)
                                    .wrap_mode(egui::TextWrapMode::Extend);
                            ui.add(expand)
                                .on_hover_text("Show this widget large; Esc or Close returns")
                                .clicked()
                        });
                    if shown.inner {
                        self.expanded = Some((ws, widget.kind));
                    }
                }
            }
        });
    }

    /// Content of one widget. Monitor shows the run it has open (live or
    /// history); Studio and Studio 2 always show the current run.
    fn widget_body(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        ws: Workspace,
        kind: WidgetKind,
        height: f32,
    ) {
        let s = self.widget_source(ws);
        match kind {
            WidgetKind::Throughput => chart(ui, s, p, Chart::Throughput, height),
            WidgetKind::RequestsPerSecond => chart(ui, s, p, Chart::Rps, height),
            WidgetKind::Latency => chart(ui, s, p, Chart::Latency, height),
            WidgetKind::ActiveUsers => chart(ui, s, p, Chart::Users, height),
            WidgetKind::Logs => self.logs_for(ui, p, ws != Workspace::Monitor, height),
            WidgetKind::StatusCodes => status_panel(ui, s, p),
            WidgetKind::Endpoints => endpoint_panel(ui, s, p),
            WidgetKind::LiveMetrics => self.studio2_metrics(ui, p, ws),
            WidgetKind::KpiCards => kpi_cards(ui, s, p),
            WidgetKind::LatencyHistogram => latency_histogram(ui, s, p, height),
            WidgetKind::Unknown => {
                ui.label("Unavailable widget");
            }
        }
    }

    /// The expanded widget in a large centred window; Esc, a click outside
    /// or Close returns to the layout. The layout itself is not changed.
    pub(super) fn expanded_widget(&mut self, ctx: &egui::Context, p: Palette) {
        let Some((ws, kind)) = self.expanded else {
            return;
        };
        let screen = ctx.content_rect();
        let size = egui::vec2(screen.width() * 0.9, screen.height() * 0.85);
        let mut close = false;
        // Charts and logs fill the window; cards and tables keep their height.
        let fill = matches!(
            kind,
            WidgetKind::Throughput
                | WidgetKind::RequestsPerSecond
                | WidgetKind::Latency
                | WidgetKind::ActiveUsers
                | WidgetKind::LatencyHistogram
                | WidgetKind::Logs
        );
        let response = egui::Modal::new(egui::Id::new("expanded_widget")).show(ctx, |ui| {
            ui.set_width(size.x);
            if fill {
                ui.set_height(size.y);
            }
            ui.horizontal(|ui| {
                ui.strong(kind.label());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui.button("Close").on_hover_text("Close (Esc)").clicked();
                });
            });
            ui.separator();
            egui::ScrollArea::both()
                .id_salt("expanded_widget_scroll")
                .max_height(size.y)
                .auto_shrink([false, !fill])
                .show(ui, |ui| {
                    ui.push_id(("expanded", ws), |ui| {
                        // Charts and logs take the window below its title row.
                        let height = (size.y - 48.0).max(160.0);
                        self.widget_body(ui, p, ws, kind, height);
                    });
                });
        });
        if close || response.should_close() {
            self.expanded = None;
        }
    }

    pub(super) fn widget_source(&self, ws: Workspace) -> &Snapshot {
        if ws == Workspace::Monitor {
            &self.snapshot
        } else {
            &self.live
        }
    }

    fn apply_dashboard_edits(&mut self, ws: Workspace, edits: Vec<Edit>) {
        let page = self.prefs.dashboards_for_mut(ws).active_mut();
        for edit in edits {
            match edit {
                Edit::Hide(id) => page.hide(id),
                Edit::Remove(id) => page.remove(id),
                Edit::Place(id, dock, before) => page.place(id, dock, before),
                Edit::Shift(id, forward) => page.shift(id, forward),
                Edit::Span(id, span) => {
                    if let Some(w) = page.widgets.iter_mut().find(|w| w.id == id) {
                        w.span = span;
                    }
                }
            }
        }
    }

    pub(super) fn dashboard_manager(&mut self, ctx: &egui::Context, p: Palette) {
        if !self.layout_manager_open {
            return;
        }
        let ws = self.workspace;
        let docks = ws.docks();
        let mut open = true;
        egui::Window::new(format!("My {} layouts & widgets", ws.label())).id(egui::Id::new("dashboard_manager")).open(&mut open).default_width(650.0).default_height(480.0).resizable(true).show(ctx, |ui| {
            ui.label(format!("Create your own {} pages from the widget catalog. Each workspace keeps its own pages; layouts are separate from test plans.", ws.label()));
            if ws == Workspace::Studio {
                ui.label(RichText::new("Studio keeps Load Configuration on the right, so its widgets dock at the bottom.").small().color(p.muted));
            }
            ui.horizontal_wrapped(|ui| {
                let all = self.prefs.dashboards_for_mut(ws);
                egui::ComboBox::from_id_salt(("manage_dashboard_page", ws)).selected_text(&all.active().name).show_ui(ui, |ui| {
                    for page in &all.pages { ui.selectable_value(&mut all.active_id,page.id,&page.name); }
                });
                if ui.button("Save layout").clicked() { self.layout_save_requested = true; }
                if ui.button("Restore default widgets").clicked() {
                    self.prefs.dashboards_for_mut(ws).active_mut().reset_widgets(ws.default_page().widgets);
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_layout_name).hint_text("Page name").desired_width(190.0));
                let all = self.prefs.dashboards_for_mut(ws);
                let blank = ui.button("Create blank page").clicked();
                let duplicate = ui.button("Duplicate current page").clicked();
                let rename = ui.button("Rename current page").clicked();
                let result = if blank || duplicate {
                    Some(all.create_for(ws, &self.new_layout_name, duplicate))
                } else if rename {
                    Some(all.rename_active(&self.new_layout_name))
                } else {
                    None
                };
                match result { Some(Ok(())) => self.new_layout_name.clear(), Some(Err(e)) => self.message = e, None => {} }
                let all = self.prefs.dashboards_for_mut(ws);
                if ui.add_enabled(all.pages.len() > 1,egui::Button::new("Delete page")).clicked()
                    && let Err(e) = all.delete_active() { self.message = e; }
            });
            if ws.allows_dashboard_only() {
                ui.horizontal_wrapped(|ui| {
                    let page = self.prefs.dashboards_for_mut(ws).active_mut();
                    ui.checkbox(&mut page.dashboard_only, "Dashboard only (hide request editor)");
                    ui.label("Dashboard row height");
                    ui.add_enabled(page.dashboard_only, egui::DragValue::new(&mut page.card_height).range(180.0..=500.0).suffix(" px"));
                });
            } else if docks.contains(&Dock::Main) {
                ui.horizontal_wrapped(|ui| {
                    let page = self.prefs.dashboards_for_mut(ws).active_mut();
                    ui.label("Main area row height");
                    ui.add(egui::DragValue::new(&mut page.card_height).range(180.0..=500.0).suffix(" px"));
                });
            }
            ui.label(RichText::new("Edit mode: drag :: handles or use arrows. × hides a widget; Remove deletes its configuration.").small().color(p.muted));
            let page = self.prefs.dashboards_for(ws).active().clone(); let mut edits = vec![];
            egui::ScrollArea::vertical().id_salt(("manage_dashboard_widgets", ws)).max_height(290.0).show(ui, |ui| {
                if page.widgets.is_empty() { ui.label("Empty page. Add widgets from the catalog below."); }
                for widget in &page.widgets {
                    ui.push_id(widget.id, |ui| {
                        theme::panel(p).show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let mut visible = widget.visible;
                                if ui.checkbox(&mut visible,widget.kind.label()).changed()
                                    && let Some(w) = self.prefs.dashboards_for_mut(ws).active_mut().widgets.iter_mut().find(|w|w.id == widget.id) { w.visible = visible; }
                                let mut dock = widget.dock;
                                ui.add_enabled_ui(!page.dashboard_only && docks.len() > 1, |ui| {
                                    egui::ComboBox::from_id_salt("dock").width(85.0).selected_text(dock.label()).show_ui(ui, |ui| {
                                        for &option in docks { ui.selectable_value(&mut dock,option,option.label()); }
                                    });
                                });
                                if dock != widget.dock { edits.push(Edit::Place(widget.id,dock,None)); }
                                let mut span = widget.span;
                                ui.add_enabled_ui(page.dashboard_only || widget.dock != Dock::Right, |ui| {
                                    egui::ComboBox::from_id_salt("span").width(85.0).selected_text(span.label()).show_ui(ui, |ui| {
                                        for option in [Span::Small,Span::Medium,Span::Full] { ui.selectable_value(&mut span,option,option.label()); }
                                    });
                                });
                                if span != widget.span { edits.push(Edit::Span(widget.id,span)); }
                                if ui.small_button("Up").clicked() { edits.push(Edit::Shift(widget.id,false)); }
                                if ui.small_button("Down").clicked() { edits.push(Edit::Shift(widget.id,true)); }
                                if ui.small_button("Remove").clicked() { edits.push(Edit::Remove(widget.id)); }
                            });
                        });
                        ui.add_space(4.0);
                    });
                }
            });
            self.apply_dashboard_edits(ws, edits);
            ui.separator(); ui.strong("Widget catalog");
            ui.horizontal_wrapped(|ui| {
                for kind in WidgetKind::ALL {
                    if ui.button(format!("+ {}",kind.label())).clicked()
                        && let Err(e) = self.prefs.dashboards_for_mut(ws).active_mut().add(kind,docks[0]) { self.message = e; }
                }
            });
            ui.label(RichText::new(if ws == Workspace::Monitor {
                "Widgets show the run open in Monitor: the current run or a history run. Pages and sizes are saved on this device; Save layout writes immediately."
            } else {
                "All widgets show the current test's real data. Pages and sizes are saved on this device; Save layout writes immediately."
            }).small().color(p.muted));
        });
        self.layout_manager_open = open;
    }
}

const KPI_CARDS: usize = 8;
/// Card columns for the KPI group at `width`: one row on wide screens.
pub(super) fn kpi_columns(width: f32) -> usize {
    if width > 1100.0 {
        KPI_CARDS
    } else if width > 750.0 {
        4
    } else {
        2
    }
}
/// The Monitor KPI card row for `s`.
pub(super) fn kpi_cards(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    let cards = [
        (
            "Virtual Users",
            format!("{} / {}", s.active_users, s.target_users),
            p.success,
        ),
        ("Requests / sec", format!("{:.1}", s.rps), p.accent),
        (
            "Data / sec",
            format!("{}/s", format_bytes(s.bytes_rate)),
            p.accent,
        ),
        (
            "P50 Latency",
            latency_label(&s.latency, s.latency.p50),
            p.p50,
        ),
        (
            "P95 Latency",
            latency_label(&s.latency, s.latency.p95),
            p.p95,
        ),
        (
            "P99 Latency",
            latency_label(&s.latency, s.latency.p99),
            p.p99,
        ),
        (
            "Error Rate",
            format!("{:.2}%", s.counts.error_rate()),
            p.error,
        ),
        ("Request Count", s.counts.completed.to_string(), p.text),
    ];
    let columns = kpi_columns(ui.available_width());
    for (index, row) in cards.chunks(columns).enumerate() {
        if index > 0 {
            ui.add_space(8.0);
        }
        ui.columns(columns, |cols| {
            for (col, (label, value, color)) in cols.iter_mut().zip(row) {
                metric(col, p, label, value, *color);
            }
        });
    }
}

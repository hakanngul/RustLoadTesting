//! Widget catalog, named user layouts and editing UI for Studio 2.
use super::*;

#[derive(Clone, Copy)]
struct DragWidget {
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
    pub(super) fn dashboard_canvas(&mut self, ctx: &egui::Context, p: Palette) {
        let page = self.prefs.dashboards.active().clone();
        let mut edits = vec![];
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(&page.name);
                ui.colored_label(p.success,format!("{:?} · {:.0}s",self.live.state,self.live.elapsed));
                if ui.button("Configure test in Studio").clicked() { self.workspace = Workspace::Studio; self.prefs.workspace = Workspace::Studio; }
            });
            let mut widgets:Vec<_> = page.widgets.iter().filter(|w|w.visible).cloned().collect();
            if widgets.is_empty() {
                ui.add_space(24.0);
                ui.heading("Build your own dashboard");
                ui.label("Use + Widget or Layouts to add charts, metrics and logs. Configure and authorize test targets in Studio.");
                if ui.button("Open widget catalog").clicked() { self.layout_manager_open = true; }
            }
            // Keep cards readable on smaller windows without changing the saved widths.
            if ui.available_width() < 600.0 { for widget in &mut widgets { widget.span = Span::Full; } }
            else if ui.available_width() < 1000.0 { for widget in &mut widgets { if widget.span == Span::Small { widget.span = Span::Medium; } } }
            egui::ScrollArea::vertical().id_salt(("dashboard_canvas",page.id)).show(ui, |ui| {
                for row in crate::dashboard::rows(&widgets) {
                    let units = row.iter().map(|w|w.span.units()).sum::<u32>() as f32;
                    let width = ui.available_width() - 8.0 * row.len().saturating_sub(1) as f32;
                    ui.horizontal_top(|ui| {
                        for widget in row {
                            ui.allocate_ui_with_layout(egui::vec2(width * widget.span.units() as f32 / units,page.card_height),egui::Layout::top_down(egui::Align::Min), |ui| {
                                self.dashboard_tile(ui,p,&page,&widget,page.card_height,&mut edits);
                            });
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        });
        self.apply_dashboard_edits(edits);
    }

    pub(super) fn dashboard_toolbar(&mut self, ui: &mut egui::Ui, _p: Palette) {
        ui.separator();
        egui::ComboBox::from_id_salt("dashboard_page")
            .width(130.0)
            .selected_text(format!("Layout: {}", self.prefs.dashboards.active().name))
            .show_ui(ui, |ui| {
                for page in &self.prefs.dashboards.pages {
                    ui.selectable_value(&mut self.prefs.dashboards.active_id, page.id, &page.name);
                }
            });
        ui.menu_button("+ Widget", |ui| {
            if self.prefs.dashboards.active().dashboard_only {
                for kind in WidgetKind::ALL {
                    if ui.button(kind.label()).clicked() {
                        if let Err(e) = self.prefs.dashboards.active_mut().add(kind, Dock::Bottom) {
                            self.message = e;
                        }
                        ui.close();
                    }
                }
            } else {
                for dock in [Dock::Bottom, Dock::Right] {
                    ui.menu_button(format!("Add to {}", dock.label()), |ui| {
                        for kind in WidgetKind::ALL {
                            if ui.button(kind.label()).clicked() {
                                if let Err(e) = self.prefs.dashboards.active_mut().add(kind, dock) {
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

    pub(super) fn dashboard_bottom(&mut self, ctx: &egui::Context, p: Palette) {
        let page = self.prefs.dashboards.active().clone();
        let widgets = page.visible(Dock::Bottom);
        if widgets.is_empty() {
            return;
        } // Give the entire area back to the editors.
        let mut edits = vec![];
        let result = egui::TopBottomPanel::bottom(egui::Id::new(("dashboard_bottom", page.id)))
            .resizable(true)
            .default_height(page.bottom_height)
            .height_range(170.0..=450.0)
            .show(ctx, |ui| {
                let height = ui.available_height();
                ui.set_min_height(height);
                if ui.available_width() < 1200.0 && widgets.len() > 1 {
                    let mut selected = self
                        .studio2_bottom_tab
                        .filter(|id| widgets.iter().any(|w| w.id == *id))
                        .unwrap_or(widgets[0].id);
                    let tabs = ui.horizontal_wrapped(|ui| {
                        for widget in &widgets {
                            ui.selectable_value(&mut selected, widget.id, widget.kind.label());
                        }
                    });
                    let tabs_height = tabs.response.rect.height() + ui.spacing().item_spacing.y;
                    self.studio2_bottom_tab = Some(selected);
                    let widget = widgets
                        .iter()
                        .find(|w| w.id == selected)
                        .expect("selected visible widget");
                    self.dashboard_tile(
                        ui,
                        p,
                        &page,
                        widget,
                        (height - tabs_height).max(70.0),
                        &mut edits,
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt(("dashboard_bottom_scroll", page.id))
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
                                                    ui, p, &page, &widget, height, &mut edits,
                                                );
                                            },
                                        );
                                    }
                                });
                            }
                        });
                }
            });
        self.prefs.dashboards.active_mut().bottom_height =
            result.response.rect.height().clamp(170.0, 450.0);
        self.apply_dashboard_edits(edits);
    }

    pub(super) fn dashboard_right(&mut self, ctx: &egui::Context, p: Palette, compact: bool) {
        if compact {
            return;
        }
        let page = self.prefs.dashboards.active().clone();
        let widgets = page.visible(Dock::Right);
        if widgets.is_empty() {
            return;
        }
        let mut edits = vec![];
        let response = egui::SidePanel::right(egui::Id::new(("dashboard_right", page.id)))
            .resizable(true)
            .default_width(page.right_width)
            .width_range(275.0..=520.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("dashboard_right_scroll", page.id))
                    .show(ui, |ui| {
                        for widget in &widgets {
                            let height = if widget.kind == WidgetKind::LiveMetrics {
                                585.0
                            } else {
                                260.0
                            };
                            self.dashboard_tile(ui, p, &page, widget, height, &mut edits);
                            ui.add_space(8.0);
                        }
                    });
            });
        self.prefs.dashboards.active_mut().right_width =
            response.response.rect.width().clamp(275.0, 520.0);
        self.apply_dashboard_edits(edits);
    }

    pub(super) fn dashboard_compact_right(&mut self, ui: &mut egui::Ui, p: Palette) {
        let page = self.prefs.dashboards.active().clone();
        let widgets = page.visible(Dock::Right);
        if widgets.is_empty() {
            return;
        }
        let mut edits = vec![];
        egui::CollapsingHeader::new("Dashboard widgets")
            .id_salt(("compact_widgets", page.id))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("compact_widgets_scroll", page.id))
                    .max_height((ui.available_height() * 0.45).clamp(80.0, 220.0))
                    .show(ui, |ui| {
                        for widget in &widgets {
                            self.dashboard_tile(ui, p, &page, widget, 280.0, &mut edits);
                        }
                    });
            });
        self.apply_dashboard_edits(edits);
    }

    fn dashboard_tile(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        page: &DashboardPage,
        widget: &Widget,
        height: f32,
        edits: &mut Vec<Edit>,
    ) {
        let editing = self.layout_edit;
        ui.push_id(("dashboard_tile", page.id, widget.id), |ui| {
            let mut body = |ui: &mut egui::Ui| {
                let edit_height = if self.layout_edit {
                    ui.horizontal(|ui| {
                        ui.dnd_drag_source(
                            egui::Id::new(("widget_drag", page.id, widget.id)),
                            DragWidget {
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
                                    for dock in [Dock::Bottom, Dock::Right] {
                                        if ui.button(format!("Move to {}", dock.label())).clicked()
                                        {
                                            edits.push(Edit::Place(widget.id, dock, None));
                                            ui.close();
                                        }
                                    }
                                }
                                ui.separator();
                                if page.dashboard_only || widget.dock == Dock::Bottom {
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
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let plot_height = (height - edit_height - 110.0).max(40.0);
                        match widget.kind {
                            WidgetKind::Throughput => {
                                chart(ui, &self.live, p, Chart::Throughput, plot_height)
                            }
                            WidgetKind::Latency => {
                                chart(ui, &self.live, p, Chart::Latency, plot_height)
                            }
                            WidgetKind::ActiveUsers => {
                                chart(ui, &self.live, p, Chart::Users, plot_height)
                            }
                            WidgetKind::Logs => self.logs_for(ui, p, true),
                            WidgetKind::StatusCodes => status_panel(ui, &self.live, p),
                            WidgetKind::Endpoints => endpoint_panel(ui, &self.live, p),
                            WidgetKind::LiveMetrics => self.studio2_metrics(ui, p),
                            WidgetKind::Unknown => {
                                ui.label("Unavailable widget");
                            }
                        }
                    });
            };
            if editing {
                let (_, dropped) = ui.dnd_drop_zone::<DragWidget, _>(
                    egui::Frame::new().inner_margin(3).corner_radius(8),
                    &mut body,
                );
                if let Some(payload) = dropped
                    && payload.page == page.id
                    && payload.widget != widget.id
                {
                    edits.push(Edit::Place(payload.widget, widget.dock, Some(widget.id)));
                }
            } else {
                body(ui);
            }
        });
    }

    fn apply_dashboard_edits(&mut self, edits: Vec<Edit>) {
        let page = self.prefs.dashboards.active_mut();
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
        let mut open = true;
        egui::Window::new("My layouts & widgets").open(&mut open).default_width(650.0).default_height(480.0).resizable(true).show(ctx, |ui| {
            ui.label("Create your own Studio 2 pages from the widget catalog. Layouts are separate from test plans.");
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("manage_dashboard_page").selected_text(&self.prefs.dashboards.active().name).show_ui(ui, |ui| {
                    for page in &self.prefs.dashboards.pages { ui.selectable_value(&mut self.prefs.dashboards.active_id,page.id,&page.name); }
                });
                if ui.button("Save layout").clicked() { self.layout_save_requested = true; }
                if ui.button("Restore default widgets").clicked() {
                    let page = self.prefs.dashboards.active_mut();
                    page.widgets = DashboardPage::default().widgets;
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.new_layout_name).hint_text("Page name").desired_width(190.0));
                if ui.button("Create blank page").clicked() {
                    match self.prefs.dashboards.create(&self.new_layout_name,false) { Ok(()) => self.new_layout_name.clear(), Err(e) => self.message = e }
                }
                if ui.button("Duplicate current page").clicked() {
                    match self.prefs.dashboards.create(&self.new_layout_name,true) { Ok(()) => self.new_layout_name.clear(), Err(e) => self.message = e }
                }
                if ui.button("Rename current page").clicked() {
                    match self.prefs.dashboards.rename_active(&self.new_layout_name) { Ok(()) => self.new_layout_name.clear(), Err(e) => self.message = e }
                }
                if ui.add_enabled(self.prefs.dashboards.pages.len() > 1,egui::Button::new("Delete page")).clicked()
                    && let Err(e) = self.prefs.dashboards.delete_active() { self.message = e; }
            });
            ui.horizontal_wrapped(|ui| {
                let page = self.prefs.dashboards.active_mut();
                ui.checkbox(&mut page.dashboard_only, "Dashboard only (hide request editor)");
                ui.label("Dashboard row height");
                ui.add_enabled(page.dashboard_only, egui::DragValue::new(&mut page.card_height).range(180.0..=500.0).suffix(" px"));
            });
            ui.label(RichText::new("Edit mode: drag :: handles or use arrows. × hides a widget; Remove deletes its configuration.").small().color(p.muted));
            let page = self.prefs.dashboards.active().clone(); let mut edits = vec![];
            egui::ScrollArea::vertical().id_salt("manage_dashboard_widgets").max_height(290.0).show(ui, |ui| {
                if page.widgets.is_empty() { ui.label("Empty page. Add widgets from the catalog below."); }
                for widget in &page.widgets {
                    ui.push_id(widget.id, |ui| {
                        theme::panel(p).show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let mut visible = widget.visible;
                                if ui.checkbox(&mut visible,widget.kind.label()).changed()
                                    && let Some(w) = self.prefs.dashboards.active_mut().widgets.iter_mut().find(|w|w.id == widget.id) { w.visible = visible; }
                                let mut dock = widget.dock;
                                ui.add_enabled_ui(!page.dashboard_only, |ui| {
                                    egui::ComboBox::from_id_salt("dock").width(70.0).selected_text(dock.label()).show_ui(ui, |ui| {
                                        ui.selectable_value(&mut dock,Dock::Bottom,"Bottom"); ui.selectable_value(&mut dock,Dock::Right,"Right");
                                    });
                                });
                                if dock != widget.dock { edits.push(Edit::Place(widget.id,dock,None)); }
                                let mut span = widget.span;
                                ui.add_enabled_ui(page.dashboard_only || widget.dock == Dock::Bottom, |ui| {
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
            self.apply_dashboard_edits(edits);
            ui.separator(); ui.strong("Widget catalog");
            ui.horizontal_wrapped(|ui| {
                for kind in WidgetKind::ALL {
                    if ui.button(format!("+ {}",kind.label())).clicked()
                        && let Err(e) = self.prefs.dashboards.active_mut().add(kind,Dock::Bottom) { self.message = e; }
                }
            });
            ui.label(RichText::new("All widgets show the current test's real data. Pages and sizes are saved on this device; Save layout writes immediately.").small().color(p.muted));
        });
        self.layout_manager_open = open;
    }
}

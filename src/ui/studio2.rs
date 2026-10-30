//! Alternative workbench: one shared plan, split request/body/load editors and live telemetry.
use super::*;

#[derive(Clone, Copy)]
enum RequestAction {
    Up(usize),
    Down(usize),
    Duplicate(usize),
    Delete(usize),
}

impl RustLoadApp {
    pub(super) fn studio2(&mut self, ctx: &egui::Context, p: Palette) {
        if self.prefs.dashboards.active().dashboard_only {
            self.dashboard_canvas(ctx, p);
            return;
        }
        let compact = ctx.content_rect().width() < 1150.0;
        self.dashboard_bottom(ctx, p);
        self.studio2_tree(ctx, p);
        self.dashboard_right(ctx, p, compact);

        let busy = self.busy();
        egui::CentralPanel::default().show(ctx, |ui| {
            if compact {
                self.dashboard_compact_right(ui, p);
            }
            let before = format!("{:?}", self.plan);
            if self.plan_settings {
                egui::ScrollArea::vertical()
                    .id_salt("studio2_settings")
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!busy, |ui| self.settings_editor(ui, p));
                        ui.separator();
                        self.studio2_load(ui, p);
                        if ui.button("Back to request editor").clicked() {
                            self.plan_settings = false;
                        }
                    });
            } else if self.selected < self.plan.requests.len() {
                let available = ui.available_height();
                let request_id = self.plan.requests[self.selected].id;
                egui::TopBottomPanel::top(if compact {
                    "studio2_request_compact"
                } else {
                    "studio2_request_split"
                })
                .frame(
                    egui::Frame::new()
                        .fill(p.surface)
                        .inner_margin(12)
                        .stroke(egui::Stroke::new(1.0_f32, p.border))
                        .corner_radius(10),
                )
                .resizable(true)
                .default_height(275.0)
                .height_range(130.0..=(available - 90.0).max(130.0))
                .show_inside(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    egui::ScrollArea::both()
                        .id_salt(("studio2_request", request_id))
                        .show(ui, |ui| {
                            ui.add_enabled_ui(!busy, |ui| {
                                compact_request_editor(
                                    ui,
                                    &mut self.plan.requests[self.selected],
                                    &mut self.tab,
                                    p,
                                );
                            });
                        });
                });
                ui.add_space(8.0);
                if ui.available_width() >= 650.0 {
                    ui.columns(2, |columns| {
                        let height = columns[0].available_height();
                        theme::panel(p).show(&mut columns[0], |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.set_min_height((height - 24.0).max(60.0));
                            egui::ScrollArea::both()
                                .id_salt(("studio2_body", request_id))
                                .max_height((height - 28.0).max(50.0))
                                .show(ui, |ui| {
                                    ui.add_enabled_ui(!busy, |ui| {
                                        split_body_editor(
                                            ui,
                                            &mut self.plan.requests[self.selected],
                                            p,
                                        )
                                    });
                                });
                        });
                        theme::panel(p).show(&mut columns[1], |ui| {
                            ui.set_min_width(ui.available_width());
                            egui::ScrollArea::vertical()
                                .id_salt("studio2_load_scroll")
                                .max_height((height - 28.0).max(50.0))
                                .show(ui, |ui| self.studio2_load(ui, p));
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.studio2_load_tab, false, "Body");
                        ui.selectable_value(&mut self.studio2_load_tab, true, "Load Configuration");
                    });
                    egui::ScrollArea::both()
                        .id_salt("studio2_lower_compact")
                        .show(ui, |ui| {
                            if self.studio2_load_tab {
                                self.studio2_load(ui, p);
                            } else {
                                ui.add_enabled_ui(!busy, |ui| {
                                    split_body_editor(ui, &mut self.plan.requests[self.selected], p)
                                });
                            }
                        });
                }
            } else {
                ui.heading("No requests yet");
                ui.label("Add a request from the Test Plan tree.");
                self.studio2_load(ui, p);
            }
            if before != format!("{:?}", self.plan) {
                self.dirty = true;
                self.acknowledged = false;
            }
        });
    }

    fn studio2_load(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.strong("Load Configuration");
        let busy = self.busy();
        ui.add_enabled_ui(!busy, |ui| compact_load_editor(ui, &mut self.plan.load));
        ui.separator();
        if busy {
            ui.colored_label(p.muted, "Running configuration is locked.");
        } else {
            ui.checkbox(
                &mut self.acknowledged,
                "I am authorized to test all configured targets.",
            );
            let origins: std::collections::BTreeSet<_> = self
                .plan
                .requests
                .iter()
                .filter_map(|r| url::Url::parse(&r.url).ok())
                .map(|u| u.origin().ascii_serialization())
                .collect();
            for origin in origins {
                ui.label(RichText::new(origin).size(11.0).color(p.muted));
            }
            if self.plan.load.users > 100 || self.plan.load.rps_limit > 100 {
                ui.colored_label(
                    p.warning,
                    "Elevated load: verify target and generator capacity.",
                );
            }
        }
    }

    pub(super) fn studio2_metrics(&self, ui: &mut egui::Ui, p: Palette) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Live Metrics");
            ui.colored_label(
                if self.busy() { p.success } else { p.muted },
                format!("{:?}", self.live.state),
            );
            ui.label(
                RichText::new(format!(
                    "{:02}:{:02}",
                    self.live.elapsed as u64 / 60,
                    self.live.elapsed as u64 % 60
                ))
                .small()
                .color(p.muted),
            );
        });
        slo_badges(ui, &self.live, p);
        ui.add_space(8.0);
        let s = &self.live;
        let target = if s.run_id.is_empty() {
            self.plan.load.users
        } else {
            s.target_users
        };
        ui.columns(2, |cols| {
            studio2_card(
                &mut cols[0],
                p,
                "Virtual Users",
                &format!("{}", s.active_users),
                p.text,
                |ui| {
                    ui.label(RichText::new(format!("/ {target}")).small().color(p.muted));
                    let fraction = s.active_users as f32 / target.max(1) as f32;
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .fill(p.accent)
                            .text(format!("{:.0}%", fraction * 100.0)),
                    );
                },
            );
            studio2_card(
                &mut cols[1],
                p,
                "Requests / sec",
                &if s.run_id.is_empty() {
                    "—".into()
                } else {
                    format!("{:.1}", s.rps)
                },
                p.text,
                |ui| {
                    ui.label(RichText::new("Last 5 seconds").small().color(p.muted));
                    if !s.run_id.is_empty() {
                        ui.label(
                            RichText::new(format!("{}/s data", format_bytes(s.bytes_rate)))
                                .small()
                                .color(p.muted),
                        );
                    }
                    sparkline(
                        ui,
                        s.series
                            .iter()
                            .map(|v| v.completed as f64 / v.interval_secs.max(0.001)),
                        p.accent,
                    );
                },
            );
        });
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            studio2_card(
                &mut cols[0],
                p,
                "P95 Latency",
                &latency_label(&s.latency, s.latency.p95),
                p.text,
                |ui| {
                    ui.label(RichText::new("Whole run").small().color(p.muted));
                    sparkline(
                        ui,
                        s.series
                            .iter()
                            .filter(|v| v.latency.samples > 0)
                            .map(|v| v.latency.p95),
                        p.p95,
                    );
                },
            );
            studio2_card(
                &mut cols[1],
                p,
                "Error Rate",
                &if s.counts.completed == 0 {
                    "—".into()
                } else {
                    format!("{:.2}%", s.counts.error_rate())
                },
                p.text,
                |ui| {
                    ui.label(RichText::new("Whole run").small().color(p.muted));
                    sparkline(
                        ui,
                        s.series
                            .iter()
                            .filter(|v| v.completed > 0)
                            .map(|v| v.failed as f64 * 100.0 / v.completed as f64),
                        p.error,
                    );
                },
            );
        });
        ui.add_space(8.0);
        studio2_card(
            ui,
            p,
            "Average Response Time",
            &latency_label(&s.latency, s.latency.mean),
            p.text,
            |ui| {
                ui.label(
                    RichText::new("Fully read responses · whole run")
                        .small()
                        .color(p.muted),
                );
                sparkline(
                    ui,
                    s.series
                        .iter()
                        .filter(|v| v.latency.samples > 0)
                        .map(|v| v.latency.mean),
                    p.info,
                );
            },
        );
        ui.add_space(8.0);
        theme::panel(p).show(ui, |ui| {
            ui.label(format!(
                "{} completed · {} failed",
                s.counts.completed, s.counts.failed
            ));
            ui.label(
                RichText::new("Response latency includes the complete response body.")
                    .small()
                    .color(p.muted),
            );
        });
        if let Some(issue) = &s.issue {
            ui.colored_label(p.warning, issue);
        }
    }

    fn studio2_tree(&mut self, ctx: &egui::Context, p: Palette) {
        let busy = self.busy();
        egui::SidePanel::left("studio2_tree").resizable(true).default_width(255.0)
            .width_range(180.0..=360.0).show(ctx, |ui| {
                ui.strong("Test Plan"); ui.separator();
                egui::ScrollArea::vertical().id_salt("studio2_tree_scroll").show(ui, |ui| {
                    let mut action = None;
                    egui::CollapsingHeader::new(&self.plan.name).id_salt("studio2_root").default_open(true).show(ui, |ui| {
                        if tree_item(ui, self.plan_settings, format!("Users ({})", self.plan.load.users)).clicked() { self.plan_settings = true; }
                        egui::CollapsingHeader::new("HTTP Requests").default_open(true).show(ui, |ui| {
                            for (i, request) in self.plan.requests.iter().enumerate() {
                                let selected = !self.plan_settings && self.selected == i;
                                let response = egui::Frame::new().fill(if selected { p.selection } else { Color32::TRANSPARENT })
                                    .corner_radius(6).inner_margin(4).show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let response = ui.add(egui::Button::selectable(selected, &request.name).truncate());
                                            let color = if request.method == "GET" { p.success } else { p.accent };
                                            ui.label(RichText::new(&request.method).monospace().size(11.0).color(color));
                                            response
                                        }).inner
                                    }).inner;
                                if response.clicked() { self.selected = i; self.plan_settings = false; }
                                response.context_menu(|ui| {
                                    ui.add_enabled_ui(!busy, |ui| {
                                        if ui.add_enabled(i > 0, egui::Button::new("Move up")).clicked() { action = Some(RequestAction::Up(i)); ui.close(); }
                                        if ui.add_enabled(i + 1 < self.plan.requests.len(), egui::Button::new("Move down")).clicked() { action = Some(RequestAction::Down(i)); ui.close(); }
                                        if ui.add_enabled(self.plan.requests.len() < 500, egui::Button::new("Duplicate")).clicked() { action = Some(RequestAction::Duplicate(i)); ui.close(); }
                                        if ui.button("Delete").clicked() { action = Some(RequestAction::Delete(i)); ui.close(); }
                                    });
                                });
                            }
                            if ui.add_enabled(!busy && self.plan.requests.len() < 500, egui::Button::new("+ Add request")).clicked() {
                                self.plan.requests.push(Request { id: self.next_id(), ..Request::default() });
                                self.selected = self.plan.requests.len() - 1; self.plan_settings = false; self.dirty = true; self.acknowledged = false;
                            }
                        });
                        egui::CollapsingHeader::new("Assertions").default_open(true).show(ui, |ui| {
                            for (i, r) in self.plan.requests.iter().enumerate() {
                                let status = r.assertions.status.map_or("200–299".into(), |v| v.to_string());
                                if tree_item(ui, !self.plan_settings && self.selected == i && self.tab == 4, format!("{} · Status {status}", r.name)).clicked() { self.selected = i; self.tab = 4; self.plan_settings = false; }
                                if let Some(max) = r.assertions.max_ms
                                    && tree_item(ui, false, format!("Response time <= {max} ms")).clicked() { self.selected = i; self.tab = 4; self.plan_settings = false; }
                            }
                        });
                        egui::CollapsingHeader::new("SLO Thresholds").default_open(true).show(ui, |ui| {
                            if self.plan.thresholds.is_empty()
                                && tree_item(ui, false, "Add SLO thresholds").clicked() { self.plan_settings = true; }
                            for t in &self.plan.thresholds {
                                if tree_item(ui, self.plan_settings, t.describe()).clicked() { self.plan_settings = true; }
                            }
                        });
                        egui::CollapsingHeader::new("CSV Data / Variables").default_open(true).show(ui, |ui| {
                            let label = if self.plan.csv.path.is_empty() { "Configure CSV source" } else { &self.plan.csv.path };
                            if tree_item(ui, self.plan_settings, label).clicked() { self.plan_settings = true; }
                            if tree_item(ui, self.plan_settings, format!("{} plan variables", self.plan.variables.len())).clicked() { self.plan_settings = true; }
                        });
                        if ui.button("Summary Report").clicked() { self.workspace = Workspace::Monitor; self.show_live = true; self.snapshot = self.live.clone(); self.report_plan = self.live_plan.clone(); }
                    });
                    if let Some(action) = action { self.studio2_request_action(action); }
                    ui.add_space(12.0);
                    ui.label(RichText::new("Sequential scenario · shared with Studio\nRight-click a request to reorder or duplicate.").small().color(p.muted));
                });
            });
    }

    fn studio2_request_action(&mut self, action: RequestAction) {
        match action {
            RequestAction::Up(i) => {
                self.plan.requests.swap(i, i - 1);
                self.selected = i - 1;
            }
            RequestAction::Down(i) => {
                self.plan.requests.swap(i, i + 1);
                self.selected = i + 1;
            }
            RequestAction::Delete(i) => {
                self.plan.requests.remove(i);
                self.selected = self
                    .selected
                    .min(self.plan.requests.len().saturating_sub(1));
            }
            RequestAction::Duplicate(i) => {
                let mut r = self.plan.requests[i].clone();
                r.id = self.next_id();
                r.name.push_str(" copy");
                self.plan.requests.insert(i + 1, r);
                self.selected = i + 1;
            }
        }
        self.dirty = true;
        self.acknowledged = false;
    }
}

fn tree_item(
    ui: &mut egui::Ui,
    selected: bool,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add(egui::Button::selectable(selected, text).truncate())
}

fn studio2_card(
    ui: &mut egui::Ui,
    p: Palette,
    label: &str,
    value: &str,
    color: Color32,
    detail: impl FnOnce(&mut egui::Ui),
) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width((ui.available_width() - 2.0).max(40.0));
        ui.set_min_height(110.0);
        ui.label(RichText::new(label).size(12.0).color(p.muted));
        ui.label(RichText::new(value).size(26.0).strong().color(color));
        detail(ui);
    });
}

fn sparkline(ui: &mut egui::Ui, values: impl Iterator<Item = f64>, color: Color32) {
    let values: Vec<_> = values.collect();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(20.0), 30.0),
        egui::Sense::hover(),
    );
    if values.len() < 2 {
        return;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(0.001);
    let points = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            egui::pos2(
                rect.left() + i as f32 / (values.len() - 1) as f32 * rect.width(),
                rect.bottom() - 3.0 - ((v - min) / span) as f32 * (rect.height() - 6.0),
            )
        })
        .collect::<Vec<_>>();
    ui.painter()
        .add(egui::Shape::line(points, egui::Stroke::new(1.4_f32, color)));
}

fn compact_request_editor(ui: &mut egui::Ui, r: &mut Request, tab: &mut usize, p: Palette) {
    ui.strong("HTTP Request");
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(ui.available_width()));
    });
    ui.horizontal(|ui| {
        ui.label("Method");
        egui::ComboBox::from_id_salt("studio2_method")
            .width(75.0)
            .selected_text(&r.method)
            .show_ui(ui, |ui| {
                for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
                    ui.selectable_value(&mut r.method, method.into(), method);
                }
            });
        ui.label("URL");
        ui.add(egui::TextEdit::singleline(&mut r.url).desired_width(ui.available_width()));
    });
    ui.horizontal_wrapped(|ui| {
        for (i, label) in [
            (0, "Params".into()),
            (1, format!("Headers ({})", r.headers.len())),
            (3, "Auth".into()),
            (4, "Assertions".into()),
            (5, "Extractors".into()),
            (6, "Advanced".into()),
        ] {
            ui.selectable_value(tab, i, label);
        }
    });
    ui.separator();
    request_options(ui, r, *tab, p);
}

fn split_body_editor(ui: &mut egui::Ui, r: &mut Request, p: Palette) {
    ui.horizontal(|ui| {
        ui.strong("Body");
        egui::ComboBox::from_id_salt("studio2_body_kind")
            .selected_text(if r.body.json { "JSON" } else { "Raw text" })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut r.body.json, true, "JSON");
                ui.selectable_value(&mut r.body.json, false, "Raw text");
            });
    });
    ui.checkbox(
        &mut r.body.persist,
        "Save body in plan (no literal secrets)",
    );
    ui.label(
        RichText::new("Use {{variables}} for sensitive values.")
            .size(11.0)
            .color(p.muted),
    );
    ui.add(
        egui::TextEdit::multiline(&mut r.body.text)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(10),
    );
}

fn compact_load_editor(ui: &mut egui::Ui, c: &mut LoadConfig) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.spacing_mut().button_padding.y = 4.0;
        ui.spacing_mut().interact_size.y = 24.0;

        egui::Grid::new("studio2_load_config")
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Virtual users");
                ui.add(egui::DragValue::new(&mut c.users).range(1..=10_000));
                ui.end_row();
                ui.label("Ramp-up time");
                ui.add_enabled(
                    c.profile == Profile::RampUp,
                    egui::DragValue::new(&mut c.ramp_secs)
                        .range(1..=86_400)
                        .suffix(" s"),
                );
                ui.end_row();
                ui.label("Total duration");
                ui.add(
                    egui::DragValue::new(&mut c.duration_secs)
                        .range(1..=86_400)
                        .suffix(" s"),
                );
                ui.end_row();
                ui.label("Load profile");
                egui::ComboBox::from_id_salt("studio2_profile")
                    .selected_text(if c.profile == Profile::Constant {
                        "Constant Users"
                    } else {
                        "Ramp-up Users"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut c.profile, Profile::Constant, "Constant Users");
                        ui.selectable_value(&mut c.profile, Profile::RampUp, "Ramp-up Users");
                    });
                ui.end_row();
                ui.label("Global RPS cap");
                ui.add(egui::DragValue::new(&mut c.rps_limit).range(1..=100_000));
                ui.end_row();
            });
        egui::CollapsingHeader::new("Timeout, think time & body limit").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Think / iteration");
                ui.add(
                    egui::DragValue::new(&mut c.think_ms)
                        .range(0..=300_000)
                        .suffix(" ms"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Request timeout");
                ui.add(
                    egui::DragValue::new(&mut c.timeout_ms)
                        .range(1..=300_000)
                        .suffix(" ms"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Body cap");
                ui.add(
                    egui::DragValue::new(&mut c.body_limit_bytes)
                        .range(1..=100 * 1024 * 1024)
                        .speed(1024)
                        .suffix(" bytes"),
                );
            });
        });
        ui.label(RichText::new("Duration includes ramp-up · Stop grace: 5s").size(11.0));
    });
}

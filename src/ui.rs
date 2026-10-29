mod studio2;

use crate::{
    core::*,
    engine::{self, RunHandle},
    fixture,
    metrics::*,
    report,
    storage::{self, History, HistoryItem},
    theme::{self, Palette},
};
use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Default, Serialize, Deserialize)]
struct Preferences {
    light: bool,
    plan_path: String,
    #[serde(default)]
    workspace: Workspace,
}
#[derive(Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
enum Workspace {
    #[default]
    Studio,
    Studio2,
    Monitor,
}
#[derive(PartialEq, Clone, Copy)]
enum FileAction {
    Open,
    Save,
    Export,
}

pub struct RustLoadApp {
    plan: TestPlan,
    prefs: Preferences,
    workspace: Workspace,
    selected: usize,
    tab: usize,
    plan_settings: bool,
    runtime: tokio::runtime::Runtime,
    run: Option<RunHandle>,
    live: Snapshot,
    snapshot: Snapshot,
    report_plan: TestPlan,
    live_plan: TestPlan,
    show_live: bool,
    history: Vec<HistoryItem>,
    dirty: bool,
    acknowledged: bool,
    message: String,
    file_action: Option<FileAction>,
    dialog_path: String,
    export_format: String,
    log_filter: String,
    level: String,
    follow: bool,
    fixture: Option<fixture::Fixture>,
    confirm_discard: bool,
    pending_new: bool,
    close_dialog: bool,
    chart_latency: bool,
    studio2_bottom_tab: usize,
    studio2_load_tab: bool,
    closing: bool,
    permit_close: bool,
    system: sysinfo::System,
    resource_sample: std::time::Instant,
    cpu: f32,
    memory: u64,
}
impl RustLoadApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let prefs: Preferences = cc
            .storage
            .and_then(|s| eframe::get_value(s, "rustload-preferences"))
            .unwrap_or_default();
        let workspace = prefs.workspace;
        let mut app = Self {
            plan: TestPlan::default(),
            prefs,
            workspace,
            selected: 0,
            tab: 0,
            plan_settings: false,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .expect("Tokio runtime"),
            run: None,
            live: Snapshot::default(),
            snapshot: Snapshot::default(),
            report_plan: TestPlan::default(),
            live_plan: TestPlan::default(),
            show_live: true,
            history: vec![],
            dirty: false,
            acknowledged: false,
            message: "Create a plan or try Local Demo. All metrics come from real HTTP requests."
                .into(),
            file_action: None,
            dialog_path: String::new(),
            export_format: "HTML".into(),
            log_filter: String::new(),
            level: "All levels".into(),
            follow: true,
            fixture: None,
            confirm_discard: false,
            pending_new: false,
            close_dialog: false,
            chart_latency: false,
            studio2_bottom_tab: 0,
            studio2_load_tab: false,
            closing: false,
            permit_close: false,
            system: sysinfo::System::new(),
            resource_sample: std::time::Instant::now() - Duration::from_secs(2),
            cpu: 0.0,
            memory: 0,
        };
        theme::apply(&cc.egui_ctx, !app.prefs.light);
        app.refresh_history();
        app
    }
    fn busy(&self) -> bool {
        self.run.is_some() && !self.live.state.terminal()
    }
    fn refresh_history(&mut self) {
        match History::open(&storage::default_db()).and_then(|h| h.list()) {
            Ok(list) => self.history = list,
            Err(e) => self.message = format!("History: {e}"),
        }
    }
    fn base(&self) -> PathBuf {
        Path::new(&self.prefs.plan_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_owned()
    }
    fn save_plan(&mut self, path: &str) -> Result<(), String> {
        if path.trim().is_empty() {
            return Err("Choose a plan path".into());
        }
        let bytes = serde_json::to_vec_pretty(&self.plan).map_err(|e| e.to_string())?;
        let target = Path::new(path);
        if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty())
            && !parent.exists()
        {
            return Err("Parent directory does not exist".into());
        }
        // Same-directory temporary file; do not truncate an existing plan on serialization/write failure.
        let temporary = target.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        use std::io::Write;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temporary, target)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())?;
        self.prefs.plan_path = path.into();
        self.dirty = false;
        self.message="Plan saved. Auth secrets and secret fields stay in this session; bodies save only when enabled.".into();
        Ok(())
    }
    fn open_plan(&mut self, path: &str) -> Result<(), String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > 16 * 1024 * 1024 {
            return Err("Plan exceeds 16 MiB".into());
        }
        let plan: TestPlan =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if plan.schema_version != SCHEMA_VERSION {
            return Err("Unsupported schema version; no automatic migration available".into());
        }
        self.plan = plan;
        self.selected = 0;
        self.dirty = false;
        self.acknowledged = false;
        self.prefs.plan_path = path.into();
        self.message =
            "Plan opened. Re-enter omitted secrets and session-only request bodies before running."
                .into();
        Ok(())
    }
    fn start_run(&mut self) {
        if !self.acknowledged {
            self.message = "Confirm you are authorized to test every configured target.".into();
            return;
        }
        match engine::start(
            &self.runtime,
            self.plan.clone(),
            &self.base(),
            storage::default_db(),
        ) {
            Ok(handle) => {
                self.live = handle.snapshots.borrow().clone();
                self.snapshot = self.live.clone();
                self.live_plan = self.plan.redacted();
                self.report_plan = self.live_plan.clone();
                self.run = Some(handle);
                self.show_live = true;
                if self.workspace != Workspace::Studio2 {
                    self.workspace = Workspace::Monitor;
                }
                self.message = "Run started. Charts and logs reflect real HTTP traffic.".into();
            }
            Err(e) => self.message = format!("Validation: {e}"),
        }
    }
    fn local_demo(&mut self) {
        if self.fixture.is_none() {
            match self.runtime.block_on(fixture::spawn(0)) {
                Ok(f) => self.fixture = Some(f),
                Err(e) => {
                    self.message = format!("Local fixture: {e}");
                    return;
                }
            }
        }
        self.plan = fixture::demo_plan(self.fixture.as_ref().expect("fixture").address);
        self.prefs.plan_path.clear();
        self.selected = 0;
        self.dirty = true;
        self.acknowledged = false;
        self.message="Local fixture started on loopback. Scenario: login -> token extraction -> products -> checkout. Review and authorize, then Run.".into();
    }
    fn toolbar(&mut self, ctx: &egui::Context, p: Palette) {
        let busy = self.busy();
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::new().fill(p.bg).inner_margin(12))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚙").size(32.0).color(p.accent));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("RustLoad").size(23.0).strong());
                        ui.label(
                            RichText::new("HTTP load testing · local engine")
                                .size(11.0)
                                .color(p.muted),
                        );
                    });
                    ui.add_space(32.0);
                    for (label, page) in [
                        ("Studio", Workspace::Studio),
                        ("Studio 2", Workspace::Studio2),
                        ("Monitor", Workspace::Monitor),
                    ] {
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).color(
                                    if self.workspace == page {
                                        p.on_accent
                                    } else {
                                        p.text
                                    },
                                ))
                                .fill(if self.workspace == page {
                                    p.accent
                                } else {
                                    p.elevated
                                }),
                            )
                            .clicked()
                        {
                            self.workspace = page;
                            self.prefs.workspace = page;
                            if page == Workspace::Studio2 && self.tab == 2 {
                                self.tab = 1;
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(if self.prefs.light {
                                "☾ Dark"
                            } else {
                                "☀ Light"
                            })
                            .clicked()
                        {
                            self.prefs.light = !self.prefs.light;
                            theme::apply(ctx, !self.prefs.light);
                        }
                        ui.label(
                            RichText::new("Local Engine  ·  v0.1.0")
                                .color(p.success)
                                .size(12.0),
                        );
                    });
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if ui.button("+ New").clicked() {
                            if self.dirty {
                                self.confirm_discard = true;
                                self.pending_new = true;
                            } else {
                                self.plan = TestPlan::default();
                                self.selected = 0;
                                self.acknowledged = false;
                                self.prefs.plan_path.clear();
                            }
                        }
                        if ui.button("Open").clicked() {
                            if self.dirty {
                                self.confirm_discard = true;
                                self.pending_new = false;
                            } else {
                                self.file_action = Some(FileAction::Open);
                                self.dialog_path = self.prefs.plan_path.clone();
                            }
                        }
                        if ui.button("Save").clicked() {
                            if self.prefs.plan_path.is_empty() {
                                self.file_action = Some(FileAction::Save);
                                self.dialog_path = "rustload-plan.json".into();
                            } else {
                                let path = self.prefs.plan_path.clone();
                                if let Err(e) = self.save_plan(&path) {
                                    self.message = e;
                                }
                            }
                        }
                        if ui.button("Save as…").clicked() {
                            self.file_action = Some(FileAction::Save);
                            self.dialog_path = self.prefs.plan_path.clone();
                        }
                        if ui
                            .add_enabled(!self.dirty, egui::Button::new("Local Demo"))
                            .on_hover_text("Save or discard your current changes first")
                            .clicked()
                        {
                            self.local_demo();
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new(
                                RichText::new("▶ Run Test").strong().color(p.on_accent),
                            )
                            .fill(p.accent),
                        )
                        .clicked()
                    {
                        self.start_run();
                    }
                    if ui
                        .add_enabled(
                            busy,
                            egui::Button::new("■ Stop")
                                .fill(p.error)
                                .stroke(egui::Stroke::NONE),
                        )
                        .clicked()
                        && let Some(run) = &self.run
                    {
                        run.stop.cancel();
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new("Reset view"))
                        .clicked()
                    {
                        self.snapshot = Snapshot::default();
                        self.show_live = false;
                        if self.workspace == Workspace::Studio2 {
                            self.run = None;
                            self.live = Snapshot::default();
                        }
                    }
                    if ui
                        .add_enabled(
                            if self.workspace == Workspace::Studio2 {
                                self.live.state.terminal()
                            } else {
                                self.snapshot.state.terminal()
                            },
                            egui::Button::new("Export Report"),
                        )
                        .clicked()
                    {
                        if self.workspace == Workspace::Studio2 {
                            self.snapshot = self.live.clone();
                            self.report_plan = self.live_plan.clone();
                            self.show_live = true;
                        }
                        self.file_action = Some(FileAction::Export);
                        self.dialog_path = "rustload-report.html".into();
                    }
                    ui.label(
                        RichText::new(if self.dirty { "Unsaved changes" } else { "" })
                            .color(p.warning)
                            .size(12.0),
                    );
                });
            });
    }
    fn sidebar(&mut self, ctx: &egui::Context, p: Palette) {
        egui::SidePanel::left("navigation")
            .resizable(true)
            .default_width(230.0)
            .width_range(170.0..=380.0)
            .show(ctx, |ui| {
                if self.workspace == Workspace::Studio {
                    ui.heading("Test Plan");
                    ui.separator();
                    ui.label(RichText::new(&self.plan.name).strong());
                    if ui
                        .selectable_label(self.plan_settings, "⚙  Users & plan settings")
                        .clicked()
                    {
                        self.plan_settings = true;
                    }
                    ui.label(
                        RichText::new(format!(
                            "{} virtual users · {}s",
                            self.plan.load.users, self.plan.load.duration_secs
                        ))
                        .small()
                        .color(p.muted),
                    );
                    ui.add_space(8.0);
                    ui.label("HTTP Requests");
                    let busy = self.busy();
                    let mut move_to = None;
                    let mut delete = None;
                    let mut duplicate = None;
                    for (i, r) in self.plan.requests.iter().enumerate() {
                        let response = ui.selectable_label(
                            !self.plan_settings && self.selected == i,
                            format!("   {}  {}", r.method, r.name),
                        );
                        if response.clicked() {
                            self.selected = i;
                            self.plan_settings = false;
                        }
                        response.context_menu(|ui| {
                            ui.add_enabled_ui(!busy, |ui| {
                                if ui.button("Duplicate").clicked() {
                                    duplicate = Some(i);
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(i > 0, egui::Button::new("Move up"))
                                    .clicked()
                                {
                                    move_to = Some((i, i - 1));
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        i + 1 < self.plan.requests.len(),
                                        egui::Button::new("Move down"),
                                    )
                                    .clicked()
                                {
                                    move_to = Some((i, i + 1));
                                    ui.close();
                                }
                                if ui.button("Delete").clicked() {
                                    delete = Some(i);
                                    ui.close();
                                }
                            });
                        });
                    }
                    if let Some((a, b)) = move_to {
                        self.plan.requests.swap(a, b);
                        self.selected = b;
                        self.dirty = true;
                    }
                    if let Some(i) = delete {
                        self.plan.requests.remove(i);
                        self.selected = self
                            .selected
                            .min(self.plan.requests.len().saturating_sub(1));
                        self.dirty = true;
                    }
                    if let Some(i) = duplicate {
                        let mut r = self.plan.requests[i].clone();
                        r.id = self.next_id();
                        r.name.push_str(" copy");
                        self.plan.requests.insert(i + 1, r);
                        self.selected = i + 1;
                        self.dirty = true;
                    }
                    if ui
                        .add_enabled(
                            !busy && self.plan.requests.len() < 500,
                            egui::Button::new("+ Add request"),
                        )
                        .clicked()
                    {
                        let r = Request {
                            id: self.next_id(),
                            ..Request::default()
                        };
                        self.plan.requests.push(r);
                        self.selected = self.plan.requests.len() - 1;
                        self.plan_settings = false;
                        self.dirty = true;
                    }
                    ui.separator();
                    ui.label(
                        RichText::new("Right-click requests to reorder, duplicate or delete.")
                            .small()
                            .color(p.muted),
                    );
                    ui.add_space(16.0);
                    ui.label("Assertions & extractors");
                    ui.label(
                        RichText::new("Configure per request in the editor.")
                            .small()
                            .color(p.muted),
                    );
                } else {
                    ui.horizontal(|ui| {
                        ui.heading("Test Runs");
                        if ui.small_button("↻").clicked() {
                            self.refresh_history();
                        }
                    });
                    ui.separator();
                    if !self.live.run_id.is_empty()
                        && ui
                            .selectable_label(
                                self.show_live,
                                format!("Current run\n{:?}", self.live.state),
                            )
                            .clicked()
                    {
                        self.show_live = true;
                        self.snapshot = self.live.clone();
                        self.report_plan = self.live_plan.clone();
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut selected = None;
                        for item in &self.history {
                            let color = match item.state.as_str() {
                                "Completed" => p.success,
                                "Failed" => p.error,
                                _ => p.muted,
                            };
                            if theme::panel(p)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(RichText::new(&item.name).strong());
                                    ui.label(
                                        RichText::new(format!(
                                            "{} · {:.0}s · {} req",
                                            item.state, item.elapsed, item.completed
                                        ))
                                        .small()
                                        .color(color),
                                    );
                                    ui.button("Open summary").clicked()
                                })
                                .inner
                            {
                                selected = Some(item.id.clone());
                            }
                            ui.add_space(4.0);
                        }
                        if let Some(id) = selected {
                            match History::open(&storage::default_db()).and_then(|h| h.read(&id)) {
                                Ok((s, p)) => {
                                    self.snapshot = s;
                                    self.report_plan = p;
                                    self.show_live = false;
                                }
                                Err(e) => self.message = e,
                            }
                        }
                        if self.history.is_empty() {
                            ui.label(
                                RichText::new("Your completed test runs will appear here.")
                                    .color(p.muted),
                            );
                        }
                    });
                }
            });
    }
    fn next_id(&self) -> u64 {
        self.plan.requests.iter().map(|r| r.id).max().unwrap_or(0) + 1
    }
    fn configuration(&mut self, ui: &mut egui::Ui, p: Palette) {
        if self.busy() {
            ui.heading("Live Metrics");
            ui.label(format!("{:?} · {:.1}s", self.live.state, self.live.elapsed));
            for (label, value) in [
                (
                    "Virtual users",
                    format!("{} / {}", self.live.active_users, self.live.target_users),
                ),
                ("Requests / sec", format!("{:.1}", self.live.rps)),
                (
                    "P95 latency",
                    latency_label(&self.live.latency, self.live.latency.p95),
                ),
                (
                    "Error rate",
                    format!("{:.2}%", self.live.counts.error_rate()),
                ),
            ] {
                metric(ui, p, label, &value, p.accent);
                ui.add_space(8.0);
            }
        } else {
            ui.heading("Load Configuration");
            let before = format!("{:?}", self.plan.load);
            load_editor(ui, &mut self.plan.load);
            if before != format!("{:?}", self.plan.load) {
                self.dirty = true;
                self.acknowledged = false;
            }
            ui.separator();
            ui.label(
                RichText::new("Closed-loop users. RPS caps starts across all users.")
                    .color(p.muted),
            );
            ui.checkbox(
                &mut self.acknowledged,
                "I am authorized to test all targets in this plan.",
            );
            let origins: std::collections::BTreeSet<_> = self
                .plan
                .requests
                .iter()
                .filter_map(|r| url::Url::parse(&r.url).ok())
                .map(|u| u.origin().ascii_serialization())
                .collect();
            for origin in origins {
                ui.label(RichText::new(origin).small().color(p.muted));
            }
            if self.plan.load.users > 100 || self.plan.load.rps_limit > 100 {
                ui.colored_label(
                    p.warning,
                    "Elevated load: verify target and local capacity.",
                );
            }
        }
    }
    fn studio(&mut self, ctx: &egui::Context, p: Palette) {
        let busy = self.busy();
        let compact = ctx.content_rect().width() < 1150.0;
        if !compact {
            egui::SidePanel::right("configuration")
                .resizable(true)
                .default_width(285.0)
                .width_range(240.0..=400.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.configuration(ui, p));
                });
        }
        egui::TopBottomPanel::bottom("studio_charts")
            .resizable(true)
            .default_height(220.0)
            .height_range(120.0..=450.0)
            .show(ctx, |ui| {
                if compact {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.chart_latency, false, "Throughput");
                        ui.selectable_value(&mut self.chart_latency, true, "Latency");
                    });
                    chart(
                        ui,
                        &self.live,
                        p,
                        if self.chart_latency {
                            Chart::Latency
                        } else {
                            Chart::Rps
                        },
                        125.0,
                    );
                } else {
                    ui.columns(2, |columns| {
                        chart(&mut columns[0], &self.live, p, Chart::Rps, 145.0);
                        chart(&mut columns[1], &self.live, p, Chart::Latency, 145.0);
                    });
                }
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if compact {
                    egui::CollapsingHeader::new("Load configuration & authorization")
                        .default_open(true)
                        .show(ui, |ui| self.configuration(ui, p));
                    ui.separator();
                }
                let before = format!("{:?}", self.plan);
                ui.add_enabled_ui(!busy, |ui| {
                    if self.plan_settings {
                        self.settings_editor(ui, p);
                    } else if let Some(request) = self.plan.requests.get_mut(self.selected) {
                        request_editor(ui, request, &mut self.tab, p);
                    } else {
                        ui.heading("No requests yet");
                        ui.label("Add a request from the Test Plan panel.");
                    }
                });
                if before != format!("{:?}", self.plan) {
                    self.dirty = true;
                    self.acknowledged = false;
                }
                if busy {
                    ui.colored_label(
                        p.muted,
                        "The running configuration is locked. Stop the test to edit.",
                    );
                }
            });
        });
    }
    fn settings_editor(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.heading("Plan & data");
        ui.label("Plan name");
        ui.text_edit_singleline(&mut self.plan.name);
        ui.add_space(12.0);
        ui.heading("Plan variables");
        field_editor(ui, &mut self.plan.variables, "variables");
        ui.colored_label(p.muted,"Variables are copied into each user. CSV values override plan variables; extraction updates only that user's state.");
        ui.separator();
        ui.heading("CSV data source");
        ui.label("Path (relative to saved plan directory)");
        ui.text_edit_singleline(&mut self.plan.csv.path);
        ui.checkbox(
            &mut self.plan.csv.reuse_rows,
            "Reuse rows when there are more users than data rows",
        );
        ui.label("Header names become variables. One row per user, fixed for the whole run.");
        ui.add_space(16.0);
        if ui.button("Validate plan").clicked() {
            self.message=match self.plan.validate(&self.base()){Ok(())=>"Plan structure and CSV are valid. Runtime variables/extractions are checked during execution.".into(),Err(e)=>format!("Validation: {e}")};
        }
    }
    fn monitor(&mut self, ctx: &egui::Context, p: Palette) {
        egui::TopBottomPanel::bottom("monitor_logs")
            .resizable(true)
            .default_height(180.0)
            .height_range(120.0..=400.0)
            .show(ctx, |ui| self.logs(ui, p));
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let s = &self.snapshot;
                ui.horizontal_wrapped(|ui| {
                    ui.heading(if s.name.is_empty() { "Monitor" } else { &s.name });
                    ui.colored_label(if s.state == RunState::Failed { p.error } else { p.success }, format!("{:?}", s.state));
                    ui.label(format!("{:02}:{:02}", s.elapsed as u64 / 60, s.elapsed as u64 % 60));
                });
                if s.run_id.is_empty() {
                    ui.add_space(24.0); ui.heading("Ready when you are");
                    ui.label("Build a test in Studio, authorize its targets and Run. Real metrics will appear here.");
                    return;
                }
                if let Some(issue) = &s.issue { ui.colored_label(p.warning, issue); }
                ui.label(RichText::new("RPS: last 5s · Percentiles: entire run, fully read responses · Latency in ms").small().color(p.muted));
                let cards = [
                    ("Virtual Users", format!("{} / {}", s.active_users, s.target_users), p.success),
                    ("Requests / sec", format!("{:.1}", s.rps), p.accent),
                    ("P50 Latency", latency_label(&s.latency, s.latency.p50), p.p50),
                    ("P95 Latency", latency_label(&s.latency, s.latency.p95), p.p95),
                    ("P99 Latency", latency_label(&s.latency, s.latency.p99), p.p99),
                    ("Error Rate", format!("{:.2}%", s.counts.error_rate()), p.error),
                    ("Request Count", s.counts.completed.to_string(), p.text),
                ];
                let columns = if ui.available_width() > 1100.0 { 7 } else if ui.available_width() > 750.0 { 4 } else { 2 };
                for row in cards.chunks(columns) {
                    ui.columns(columns, |cols| {
                        for (col, (label, value, color)) in cols.iter_mut().zip(row) { metric(col, p, label, value, *color); }
                    });
                    ui.add_space(8.0);
                }
                if ui.available_width() > 800.0 {
                    ui.columns(2, |cols| {
                        chart(&mut cols[0], s, p, Chart::Rps, 130.0);
                        chart(&mut cols[1], s, p, Chart::Latency, 130.0);
                    });
                    ui.add_space(8.0);
                    ui.columns(2, |cols| {
                        status_panel(&mut cols[0], s, p);
                        chart(&mut cols[1], s, p, Chart::Users, 100.0);
                    });
                } else {
                    chart(ui, s, p, Chart::Rps, 130.0);
                    chart(ui, s, p, Chart::Latency, 130.0);
                    status_panel(ui, s, p);
                    chart(ui, s, p, Chart::Users, 100.0);
                }
                ui.add_space(8.0);
                endpoint_panel(ui, s, p);
                ui.add_space(8.0);
            });
        });
    }
    fn logs(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.logs_for(ui, p, false);
    }
    fn logs_for(&mut self, ui: &mut egui::Ui, p: Palette, live: bool) {
        theme::panel(p).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.heading("Live Logs");
                egui::ComboBox::from_id_salt("level")
                    .selected_text(&self.level)
                    .show_ui(ui, |ui| {
                        for l in ["All levels", "INFO", "ERROR"] {
                            ui.selectable_value(&mut self.level, l.into(), l);
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut self.log_filter)
                        .hint_text("Filter logs…")
                        .desired_width(170.0),
                );
                ui.checkbox(&mut self.follow, "Follow");
            });
            ui.label(
                RichText::new("Bounded 5,000 events · no request bodies or credential values")
                    .small()
                    .color(p.muted),
            );
            let source = if live { &self.live } else { &self.snapshot };
            let rows: Vec<_> = source
                .logs
                .iter()
                .filter(|l| {
                    (self.level == "All levels" || l.level == self.level)
                        && l.message
                            .to_ascii_lowercase()
                            .contains(&self.log_filter.to_ascii_lowercase())
                })
                .collect();
            egui::ScrollArea::vertical()
                .id_salt("logs")
                .max_height(200.0)
                .stick_to_bottom(self.follow)
                .show_rows(ui, 18.0, rows.len(), |ui, range| {
                    for index in range {
                        let log = rows[index];
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:7.2}s", log.elapsed))
                                    .monospace()
                                    .color(p.muted),
                            );
                            ui.label(RichText::new(format!("{:5}", log.level)).monospace().color(
                                if log.level == "ERROR" {
                                    p.error
                                } else {
                                    p.info
                                },
                            ));
                            ui.label(RichText::new(&log.message).monospace().size(12.0));
                        });
                    }
                });
        });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_discard {
            egui::Window::new("Unsaved plan")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Save the plan first, or discard your current changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Keep editing").clicked() {
                            self.confirm_discard = false;
                        }
                        if ui.button("Discard changes").clicked() {
                            self.confirm_discard = false;
                            self.dirty = false;
                            if self.pending_new {
                                self.plan = TestPlan::default();
                                self.prefs.plan_path.clear();
                                self.selected = 0;
                                self.acknowledged = false;
                            } else {
                                self.file_action = Some(FileAction::Open);
                            }
                        }
                    });
                });
        }
        if let Some(action) = self.file_action {
            egui::Window::new(match action {
                FileAction::Open => "Open plan",
                FileAction::Save => "Save plan",
                FileAction::Export => "Export report",
            })
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label("File path");
                ui.add(
                    egui::TextEdit::singleline(&mut self.dialog_path).desired_width(f32::INFINITY),
                );
                if action == FileAction::Save {
                    ui.label("Secrets are omitted. Saving to an existing path replaces that file.");
                }
                if action == FileAction::Export {
                    ui.horizontal(|ui| {
                        for format in ["HTML", "JSON", "CSV"] {
                            if ui
                                .selectable_value(&mut self.export_format, format.into(), format)
                                .changed()
                            {
                                self.dialog_path = Path::new(&self.dialog_path)
                                    .with_extension(format.to_ascii_lowercase())
                                    .to_string_lossy()
                                    .to_string();
                            }
                        }
                    });
                    ui.label("Export replaces an existing file at the selected path.");
                }
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.file_action = None;
                    }
                    if ui.button("Continue").clicked() {
                        let path = self.dialog_path.clone();
                        let result = match action {
                            FileAction::Open => self.open_plan(&path),
                            FileAction::Save => self.save_plan(&path),
                            FileAction::Export => {
                                let points = History::open(&storage::default_db())
                                    .and_then(|h| h.points(&self.snapshot.run_id));
                                let points = points.unwrap_or_else(|_| self.snapshot.series.iter().cloned().collect());
                                let mut summary = self.snapshot.clone();
                                if points.iter().map(|p|p.completed).sum::<u64>() != summary.counts.completed {
                                    summary.issue = Some("Time series incomplete: only available intervals are exported; summary counters are authoritative.".into());
                                }
                                report::export(Path::new(&path), &self.export_format, &summary, &self.report_plan, &points)
                            }
                        };
                        match result {
                            Ok(()) => {
                                self.file_action = None;
                                if action == FileAction::Export {
                                    self.message =
                                        format!("{} report saved to {path}", self.export_format);
                                }
                            }
                            Err(e) => self.message = e,
                        }
                    }
                });
            });
        }
        if self.close_dialog {
            egui::Window::new("Close RustLoad?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("An active test will be stopped and its partial results saved.");
                    if self.dirty {
                        ui.label(
                            "The plan has unsaved changes. Save it before closing to keep them.",
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Keep working").clicked() {
                            self.close_dialog = false;
                        }
                        if self.dirty && ui.button("Save plan…").clicked() {
                            self.file_action = Some(FileAction::Save);
                            self.dialog_path = self.prefs.plan_path.clone();
                            self.close_dialog = false;
                        }
                        if ui
                            .button(if self.dirty {
                                "Discard plan & close"
                            } else {
                                "Stop & close"
                            })
                            .clicked()
                        {
                            if let Some(run) = &self.run {
                                run.stop.cancel();
                            }
                            self.closing = true;
                            self.close_dialog = false;
                        }
                    });
                });
        }
    }
}
impl eframe::App for RustLoadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.resource_sample.elapsed() >= Duration::from_secs(1) {
            let pid = sysinfo::Pid::from_u32(std::process::id());
            self.system
                .refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            if let Some(process) = self.system.process(pid) {
                self.cpu = process.cpu_usage();
                self.memory = process.memory();
            }
            self.resource_sample = std::time::Instant::now();
        }
        if let Some(run) = &mut self.run
            && run.snapshots.has_changed().unwrap_or(true)
        {
            let previous = self.live.state;
            self.live = run.snapshots.borrow_and_update().clone();
            if self.show_live {
                self.snapshot = self.live.clone();
                self.report_plan = self.live_plan.clone();
            }
            if self.live.state.terminal() && !previous.terminal() {
                self.refresh_history();
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.permit_close {
            if self.dirty || self.busy() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_dialog = true;
            } else {
                self.permit_close = true;
            }
        }
        if self.closing && !self.busy() {
            self.permit_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if !self.busy() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            if self.prefs.plan_path.is_empty() {
                self.file_action = Some(FileAction::Save);
                self.dialog_path = "rustload-plan.json".into();
            } else {
                let path = self.prefs.plan_path.clone();
                if let Err(e) = self.save_plan(&path) {
                    self.message = e;
                }
            }
        }
        if !self.busy() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O))
        {
            if self.dirty {
                self.confirm_discard = true;
                self.pending_new = false;
            } else {
                self.file_action = Some(FileAction::Open);
                self.dialog_path = self.prefs.plan_path.clone();
            }
        }
        let p = theme::palette(!self.prefs.light);
        self.toolbar(ctx, p);
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&self.message).size(12.0).color(p.muted));
            });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Load generator · CPU {:.1}% (one core = 100%) · RAM {:.1} MiB",
                        self.cpu,
                        self.memory as f64 / 1_048_576.0
                    ))
                    .size(11.0)
                    .color(p.muted),
                );
                let cores = std::thread::available_parallelism().map_or(1, usize::from) as f32;
                if self.memory > 1024 * 1024 * 1024 || self.cpu > cores * 90.0 {
                    ui.colored_label(
                        p.warning,
                        "Local capacity warning: results may be generator-limited",
                    );
                }
            });
        });
        match self.workspace {
            Workspace::Studio => {
                self.sidebar(ctx, p);
                self.studio(ctx, p);
            }
            Workspace::Studio2 => self.studio2(ctx, p),
            Workspace::Monitor => {
                self.sidebar(ctx, p);
                self.monitor(ctx, p);
            }
        }
        self.dialogs(ctx);
        ctx.request_repaint_after(Duration::from_millis(if self.busy() { 100 } else { 500 }));
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "rustload-preferences", &self.prefs);
    }
}
impl Drop for RustLoadApp {
    fn drop(&mut self) {
        if let Some(run) = &self.run {
            run.stop.cancel();
        }
        if let Some(f) = &self.fixture {
            f.stop.cancel();
        }
    }
}

fn latency_label(latency: &Latency, value: f64) -> String {
    if latency.samples == 0 {
        "—".into()
    } else {
        format!("{value:.1} ms")
    }
}
fn status_panel(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    theme::panel(p).show(ui, |ui| {
        ui.strong("HTTP Status Distribution");
        if s.statuses.is_empty() {
            ui.label("No HTTP responses yet");
        }
        let total: u64 = s.statuses.values().sum();
        for (status, count) in &s.statuses {
            let fraction = *count as f32 / total.max(1) as f32;
            ui.horizontal(|ui| {
                ui.label(status.to_string());
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(110.0)
                        .fill(if *status < 400 { p.success } else { p.error }),
                );
                ui.label(format!("{} · {:.1}%", count, fraction * 100.0));
            });
        }
        ui.separator();
        ui.label(
            RichText::new(format!(
                "Network {} · Timeout {} · Body limit {} · Local preparation {}",
                s.counts.network_errors,
                s.counts.timeouts,
                s.counts.body_errors,
                s.counts.preparation_errors
            ))
            .small(),
        );
        ui.label(
            RichText::new(format!(
                "Unexpected status {} · Assertion/extraction {}",
                s.counts.status_errors, s.counts.assertion_errors
            ))
            .small(),
        );
        ui.label(
            RichText::new(format!(
                "Cancelled {} · In-flight {}",
                s.counts.cancelled,
                s.counts.in_flight()
            ))
            .small(),
        );
    });
}
fn endpoint_panel(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Endpoint Breakdown · slowest P95 first");
        let mut endpoints: Vec<_> = s.endpoints.iter().collect();
        endpoints.sort_by(|a, b| b.latency.p95.total_cmp(&a.latency.p95));
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("endpoints")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for title in [
                        "Request",
                        "Method",
                        "Completed",
                        "Errors",
                        "Avg RPS",
                        "P50",
                        "P90",
                        "P95",
                        "P99",
                    ] {
                        ui.strong(title);
                    }
                    ui.end_row();
                    for e in endpoints {
                        ui.label(&e.name);
                        ui.label(&e.method);
                        ui.label(e.counts.completed.to_string());
                        ui.label(e.counts.failed.to_string());
                        ui.label(format!(
                            "{:.1}",
                            e.counts.completed as f64 / s.elapsed.max(0.001)
                        ));
                        for value in [e.latency.p50, e.latency.p90, e.latency.p95, e.latency.p99] {
                            ui.label(latency_label(&e.latency, value));
                        }
                        ui.end_row();
                    }
                });
        });
    });
}
fn metric(ui: &mut egui::Ui, p: Palette, label: &str, value: &str, color: Color32) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width((ui.available_width() - 4.0).max(40.0));
        ui.label(RichText::new(label).size(12.0).color(p.muted));
        ui.add_space(6.0);
        ui.label(RichText::new(value).size(27.0).strong().color(color));
    });
}
#[derive(Clone, Copy)]
enum Chart {
    Rps,
    Throughput,
    Latency,
    Users,
}
fn chart(ui: &mut egui::Ui, s: &Snapshot, p: Palette, kind: Chart, height: f32) {
    let (id, title) = match kind {
        Chart::Rps => ("rps", "Requests per Second"),
        Chart::Throughput => ("throughput", "Throughput (requests per second)"),
        Chart::Latency => ("latency", "Response Time (ms) · interval percentiles"),
        Chart::Users => ("users", "Active Virtual Users"),
    };
    theme::panel(p).show(ui, |ui| {
        ui.strong(title);
        if s.series.is_empty() {
            ui.label(RichText::new("Waiting for measurements…").color(p.muted));
        }
        Plot::new(id)
            .height(height)
            .include_y(0.0)
            .grid_spacing(egui::emath::Rangef::new(25.0, 100.0))
            .show_axes([true, true])
            .legend(Legend::default())
            .allow_drag(true)
            .allow_zoom(true)
            .show(ui, |plot| {
                let line = |name: &str, color: Color32, values: Vec<[f64; 2]>| {
                    Line::new(name, PlotPoints::from(values))
                        .color(color)
                        .width(1.8_f32)
                };
                let values = |f: fn(&Point) -> f64| {
                    s.series
                        .iter()
                        .map(|v| [v.elapsed, f(v)])
                        .collect::<Vec<_>>()
                };
                match kind {
                    Chart::Rps | Chart::Throughput => {
                        let rates = |f: fn(&Point) -> u64| {
                            s.series
                                .iter()
                                .map(|point| {
                                    [
                                        point.elapsed,
                                        f(point) as f64 / point.interval_secs.max(0.001),
                                    ]
                                })
                                .collect()
                        };
                        plot.line(line(
                            if matches!(kind, Chart::Rps) {
                                "Total"
                            } else {
                                "Requests/sec"
                            },
                            p.accent,
                            rates(|v| v.completed),
                        ));
                        if matches!(kind, Chart::Rps) {
                            plot.line(line("Success", p.success, rates(|v| v.successful)));
                            plot.line(line("Failed", p.error, rates(|v| v.failed)));
                        }
                    }
                    Chart::Latency => {
                        for (name, color, f) in [
                            (
                                "P50",
                                p.p50,
                                (|v: &Point| v.latency.p50) as fn(&Point) -> f64,
                            ),
                            ("P95", p.p95, |v: &Point| v.latency.p95),
                            ("P99", p.p99, |v: &Point| v.latency.p99),
                        ] {
                            plot.line(line(
                                name,
                                color,
                                s.series
                                    .iter()
                                    .filter(|v| v.latency.samples > 0)
                                    .map(|v| [v.elapsed, f(v)])
                                    .collect(),
                            ));
                        }
                    }
                    Chart::Users => {
                        plot.line(line("Active users", p.success, values(|v| v.users as f64)))
                    }
                }
            });
        ui.label(
            RichText::new("Elapsed seconds · drag to pan, scroll to zoom")
                .size(11.0)
                .color(p.muted),
        );
    });
}
fn load_editor(ui: &mut egui::Ui, c: &mut LoadConfig) {
    egui::Grid::new("load_config")
        .num_columns(2)
        .spacing([8.0, 10.0])
        .show(ui, |ui| {
            ui.label("Virtual users");
            ui.add(egui::DragValue::new(&mut c.users).range(1..=10_000));
            ui.end_row();
            ui.label("Total duration");
            ui.add(
                egui::DragValue::new(&mut c.duration_secs)
                    .range(1..=86_400)
                    .suffix(" s"),
            );
            ui.end_row();
            ui.label("Profile");
            egui::ComboBox::from_id_salt("profile")
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
            if c.profile == Profile::RampUp {
                ui.label("Ramp-up");
                ui.add(
                    egui::DragValue::new(&mut c.ramp_secs)
                        .range(1..=86_400)
                        .suffix(" s"),
                );
                ui.end_row();
            }
            ui.label("Global RPS cap");
            ui.add(egui::DragValue::new(&mut c.rps_limit).range(1..=100_000));
            ui.end_row();
            ui.label("Think / iteration");
            ui.add(
                egui::DragValue::new(&mut c.think_ms)
                    .range(0..=300_000)
                    .suffix(" ms"),
            );
            ui.end_row();
            ui.label("Request timeout");
            ui.add(
                egui::DragValue::new(&mut c.timeout_ms)
                    .range(1..=300_000)
                    .suffix(" ms"),
            );
            ui.end_row();
            ui.label("Body cap");
            ui.add(
                egui::DragValue::new(&mut c.body_limit_bytes)
                    .range(1..=100 * 1024 * 1024)
                    .speed(1024)
                    .suffix(" bytes"),
            );
            ui.end_row();
        });
    ui.label("Total duration includes ramp-up. Stop allows up to 5s for in-flight requests.");
}
fn field_editor(ui: &mut egui::Ui, fields: &mut Vec<Field>, id: &str) {
    let mut remove = None;
    egui::Grid::new(id)
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Value");
            ui.strong("Secret");
            ui.end_row();
            for (i, field) in fields.iter_mut().enumerate() {
                ui.add_sized([140.0, 24.0], egui::TextEdit::singleline(&mut field.name));
                let hidden = field.secret || sensitive(&field.name);
                ui.add_sized(
                    [210.0, 24.0],
                    egui::TextEdit::singleline(&mut field.value).password(hidden),
                );
                ui.add_enabled(
                    !sensitive(&field.name),
                    egui::Checkbox::without_text(&mut field.secret),
                );
                if ui.small_button("×").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        fields.remove(i);
    }
    if ui.button("+ Add field").clicked() {
        fields.push(Field::default());
    }
    ui.small("Secret values are omitted from saved plans, history and reports.");
}
fn request_editor(ui: &mut egui::Ui, r: &mut Request, tab: &mut usize, p: Palette) {
    ui.heading("HTTP Request");
    ui.add_space(8.0);
    egui::Grid::new("request_fields")
        .num_columns(2)
        .spacing([12.0, 12.0])
        .show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(f32::INFINITY));
            ui.end_row();
            ui.label("Method");
            egui::ComboBox::from_id_salt("method")
                .selected_text(&r.method)
                .show_ui(ui, |ui| {
                    for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
                        ui.selectable_value(&mut r.method, method.into(), method);
                    }
                });
            ui.end_row();
            ui.label("URL");
            ui.add(egui::TextEdit::singleline(&mut r.url).desired_width(f32::INFINITY));
            ui.end_row();
        });
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        for (i, name) in [
            "Params",
            "Headers",
            "Body",
            "Auth",
            "Assertions",
            "Extractors",
            "Advanced",
        ]
        .iter()
        .enumerate()
        {
            ui.selectable_value(tab, i, *name);
        }
    });
    ui.separator();
    request_options(ui, r, *tab, p);
}

fn request_options(ui: &mut egui::Ui, r: &mut Request, tab: usize, p: Palette) {
    match tab {
        0 => field_editor(ui, &mut r.query, "query"),
        1 => field_editor(ui, &mut r.headers, "headers"),
        2 => {
            ui.checkbox(&mut r.body.json, "JSON body");
            ui.checkbox(
                &mut r.body.persist,
                "Persist body in plan (ensure it contains no credentials)",
            );
            ui.colored_label(p.warning,"Bodies stay session-only by default. Prefer {{secret_variable}} placeholders for sensitive values.");
            ui.add(
                egui::TextEdit::multiline(&mut r.body.text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(14),
            );
        }
        3 => {
            egui::ComboBox::from_id_salt("auth")
                .selected_text(format!("{:?}", r.auth.kind))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut r.auth.kind, AuthKind::None, "None");
                    ui.selectable_value(&mut r.auth.kind, AuthKind::Basic, "Basic");
                    ui.selectable_value(&mut r.auth.kind, AuthKind::Bearer, "Bearer");
                });
            match r.auth.kind {
                AuthKind::None => {
                    ui.label("No authentication");
                }
                AuthKind::Basic => {
                    ui.label("Username");
                    ui.text_edit_singleline(&mut r.auth.username);
                    ui.label("Password (session-only)");
                    ui.add(egui::TextEdit::singleline(&mut r.auth.password).password(true));
                }
                AuthKind::Bearer => {
                    ui.label("Token or {{variable}} (session-only)");
                    ui.add(egui::TextEdit::singleline(&mut r.auth.bearer).password(true));
                }
            }
            ui.label("Cookie sessions are not supported in v0.1.");
        }
        4 => {
            ui.heading("Response assertions");
            let mut enabled = r.assertions.status.is_some();
            if ui
                .checkbox(&mut enabled, "Exact expected status (otherwise 200–299)")
                .changed()
            {
                r.assertions.status = enabled.then_some(200);
            }
            if let Some(s) = &mut r.assertions.status {
                ui.add(egui::DragValue::new(s).range(100..=599));
            }
            let mut timed = r.assertions.max_ms.is_some();
            if ui.checkbox(&mut timed, "Maximum response time").changed() {
                r.assertions.max_ms = timed.then_some(1000);
            }
            if let Some(ms) = &mut r.assertions.max_ms {
                ui.add(egui::DragValue::new(ms).range(1..=300_000).suffix(" ms"));
            }
            ui.label("JSON Pointer (optional, e.g. /created)");
            ui.text_edit_singleline(&mut r.assertions.json_pointer);
            ui.label("Expected JSON value (e.g. true, 123, or \"text\")");
            ui.text_edit_singleline(&mut r.assertions.json_expected);
        }
        5 => {
            ui.heading("JSON response extraction");
            ui.label(
                "RFC 6901 pointers, e.g. /access_token. Values belong to this virtual user only.",
            );
            let mut remove = None;
            for (i, e) in r.extractors.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut e.variable)
                            .hint_text("Variable")
                            .desired_width(160.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut e.pointer)
                            .hint_text("/json/pointer")
                            .desired_width(180.0),
                    );
                    if ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                r.extractors.remove(i);
            }
            if ui.button("+ Add extractor").clicked() {
                r.extractors.push(Extractor::default());
            }
        }
        _ => {
            let mut enabled = r.timeout_ms.is_some();
            if ui
                .checkbox(&mut enabled, "Override request timeout")
                .changed()
            {
                r.timeout_ms = enabled.then_some(10_000);
            }
            if let Some(t) = &mut r.timeout_ms {
                ui.add(egui::DragValue::new(t).range(1..=300_000).suffix(" ms"));
            }
            ui.label("Redirects, automatic retries and cookie management are disabled.");
            ui.label("On a failed step, the remaining scenario steps are skipped and the next iteration begins after think time.");
        }
    }
}

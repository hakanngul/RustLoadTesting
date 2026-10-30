//! Monitor view comparing two saved runs.
use super::*;
use crate::compare::{
    Change, CompareMetric, CompareVerdict, Comparison, DEFAULT_NOISE_PCT, Delta, compare,
    run_time_label,
};

pub(super) struct LoadedRun {
    id: String,
    snapshot: Snapshot,
    plan: TestPlan,
    points: Vec<Point>,
}
impl LoadedRun {
    fn load(history: &History, id: &str) -> Result<Self, String> {
        let (snapshot, plan) = history.read(id)?;
        let points = history
            .points(id)
            .unwrap_or_else(|_| snapshot.series.iter().cloned().collect());
        Ok(Self {
            id: id.to_owned(),
            snapshot,
            plan,
            points,
        })
    }
}
pub(super) struct ComparisonView {
    base: LoadedRun,
    candidate: LoadedRun,
}
impl ComparisonView {
    pub(super) fn load(base: &str, candidate: &str) -> Result<Self, String> {
        let history = History::open(&storage::default_db())?;
        Ok(Self {
            base: LoadedRun::load(&history, base)?,
            candidate: LoadedRun::load(&history, candidate)?,
        })
    }
}

fn value_label(metric: CompareMetric, value: Option<f64>) -> String {
    match (metric, value) {
        (_, None) => "—".into(),
        (CompareMetric::Throughput, Some(v)) => format!("{v:.1}"),
        (CompareMetric::ErrorRate, Some(v)) => format!("{v:.2}%"),
        (CompareMetric::DataRate, Some(v)) => format!("{}/s", format_bytes(v)),
        (_, Some(v)) => format!("{v:.2}"),
    }
}
fn change_label(d: &Delta) -> String {
    match (d.change, d.change_pct) {
        (Change::NoData, _) => "no data".into(),
        // Avoid "-0.0%" for changes that round to zero.
        (_, Some(pct)) => format!("{:+.1}%", if pct.abs() < 0.05 { 0.0 } else { pct }),
        (_, None) => "from 0".into(),
    }
}
fn change_color(change: Change, p: Palette) -> Color32 {
    match change {
        Change::Improved => p.success,
        Change::Regressed => p.error,
        Change::Changed => p.info,
        Change::Unchanged | Change::NoData => p.muted,
    }
}
fn run_label(run: &LoadedRun) -> String {
    format!(
        "{} · {} · {:?} · {} req in {:.0}s",
        run.snapshot.name,
        run_time_label(&run.id),
        run.snapshot.state,
        run.snapshot.counts.completed,
        run.snapshot.elapsed
    )
}

fn overlay_chart(
    ui: &mut egui::Ui,
    p: Palette,
    id: &str,
    title: &str,
    view: &ComparisonView,
    color: Color32,
    value: fn(&Point) -> Option<f64>,
) {
    theme::panel(p).show(ui, |ui| {
        ui.strong(title);
        Plot::new(id)
            .height(150.0)
            .include_y(0.0)
            .legend(Legend::default())
            .allow_drag(true)
            .allow_zoom(true)
            .show(ui, |plot| {
                for (name, run, line_color, dashed) in [
                    ("Baseline", &view.base, p.muted, true),
                    ("Candidate", &view.candidate, color, false),
                ] {
                    let points: Vec<[f64; 2]> = run
                        .points
                        .iter()
                        .filter_map(|v| value(v).map(|y| [v.elapsed, y]))
                        .collect();
                    let mut line = Line::new(name, PlotPoints::from(points))
                        .color(line_color)
                        .width(1.8_f32);
                    if dashed {
                        line = line.style(egui_plot::LineStyle::dashed_dense());
                    }
                    plot.line(line);
                }
            });
        ui.label(
            RichText::new("Elapsed seconds from each run's start")
                .size(11.0)
                .color(p.muted),
        );
    });
}

fn overall_table(ui: &mut egui::Ui, comparison: &Comparison, p: Palette) {
    egui::Grid::new("compare_overall")
        .striped(true)
        .min_col_width(80.0)
        .show(ui, |ui| {
            for title in ["Metric", "Baseline", "Candidate", "Change", "Result"] {
                ui.strong(title);
            }
            ui.end_row();
            for d in &comparison.overall {
                ui.label(d.metric.label());
                ui.label(value_label(d.metric, d.base));
                ui.label(value_label(d.metric, d.candidate));
                ui.colored_label(change_color(d.change, p), change_label(d));
                ui.colored_label(change_color(d.change, p), format!("{:?}", d.change));
                ui.end_row();
            }
        });
}

fn endpoint_table(ui: &mut egui::Ui, comparison: &Comparison, p: Palette) {
    const COLUMNS: [CompareMetric; 5] = [
        CompareMetric::Throughput,
        CompareMetric::P50,
        CompareMetric::P95,
        CompareMetric::P99,
        CompareMetric::ErrorRate,
    ];
    egui::ScrollArea::horizontal()
        .id_salt("compare_endpoints_scroll")
        .show(ui, |ui| {
            egui::Grid::new("compare_endpoints")
                .striped(true)
                .min_col_width(70.0)
                .show(ui, |ui| {
                    ui.strong("Request");
                    for m in COLUMNS {
                        ui.strong(m.label());
                    }
                    ui.end_row();
                    for e in &comparison.endpoints {
                        ui.label(&e.name);
                        for m in COLUMNS {
                            if let Some(d) = e.deltas.iter().find(|d| d.metric == m) {
                                ui.colored_label(change_color(d.change, p), change_label(d))
                                    .on_hover_text(format!(
                                        "{} → {}",
                                        value_label(m, d.base),
                                        value_label(m, d.candidate)
                                    ));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

impl RustLoadApp {
    pub(super) fn comparison_view(&mut self, ui: &mut egui::Ui, p: Palette) {
        let Some(view) = &self.comparison else {
            return;
        };
        let mut noise = self.prefs.compare_noise_pct.unwrap_or(DEFAULT_NOISE_PCT);
        let comparison = compare(
            &view.base.snapshot,
            &view.base.plan,
            &view.candidate.snapshot,
            &view.candidate.plan,
            noise,
        );
        let mut swap = false;
        let mut close = false;
        egui::ScrollArea::vertical()
            .id_salt("comparison_scroll")
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("Run comparison");
                    let (text, color) = match comparison.verdict {
                        CompareVerdict::Improved => ("Improved", p.success),
                        CompareVerdict::Regressed => ("Regressed", p.error),
                        CompareVerdict::Mixed => ("Mixed", p.warning),
                        CompareVerdict::Unchanged => ("Unchanged", p.muted),
                        CompareVerdict::NoData => ("No data", p.muted),
                    };
                    ui.label(RichText::new(text).strong().color(color));
                    swap = ui.button("Swap").clicked();
                    close = ui.button("Close comparison").clicked();
                });
                ui.label(RichText::new(format!("Baseline: {}", run_label(&view.base))).color(p.muted));
                ui.label(RichText::new(format!("Candidate: {}", run_label(&view.candidate))).color(p.text));
                ui.horizontal_wrapped(|ui| {
                    ui.label("Noise band ±");
                    ui.add(
                        egui::DragValue::new(&mut noise)
                            .range(0.0..=50.0)
                            .speed(0.1)
                            .suffix(" %"),
                    );
                    ui.label(
                        RichText::new(
                            "Changes inside the band count as unchanged. This is a tolerance, not a significance test.",
                        )
                        .small()
                        .color(p.muted),
                    );
                });
                if !comparison.warnings.is_empty() {
                    ui.add_space(4.0);
                    ui.colored_label(p.warning, "Not a like-for-like comparison:");
                    for w in &comparison.warnings {
                        ui.colored_label(p.warning, format!("• {w}"));
                    }
                }
                ui.add_space(8.0);
                theme::panel(p).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.strong("Whole run");
                    ui.label(
                        RichText::new(
                            "Throughput and data are averages over each run (completed / elapsed). Latency uses whole-run histograms.",
                        )
                        .small()
                        .color(p.muted),
                    );
                    overall_table(ui, &comparison, p);
                });
                ui.add_space(8.0);
                let charts = |ui: &mut egui::Ui, which: usize| {
                    if which == 0 {
                        overlay_chart(ui, p, "compare_throughput", "Throughput (requests per second)", view, p.accent, |v| {
                            Some(v.completed as f64 / v.interval_secs.max(0.001))
                        });
                    } else {
                        overlay_chart(ui, p, "compare_p95", "P95 latency (ms) · interval", view, p.p95, |v| {
                            (v.latency.samples > 0).then_some(v.latency.p95)
                        });
                    }
                };
                if ui.available_width() > 800.0 {
                    ui.columns(2, |cols| {
                        charts(&mut cols[0], 0);
                        charts(&mut cols[1], 1);
                    });
                } else {
                    charts(ui, 0);
                    charts(ui, 1);
                }
                ui.add_space(8.0);
                theme::panel(p).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.strong("Per request · change from baseline (hover for values)");
                    endpoint_table(ui, &comparison, p);
                    for (label, names) in [
                        ("Only in baseline", &comparison.only_base),
                        ("Only in candidate", &comparison.only_candidate),
                    ] {
                        if !names.is_empty() {
                            ui.label(
                                RichText::new(format!("{label}: {}", names.join(", ")))
                                    .small()
                                    .color(p.muted),
                            );
                        }
                    }
                });
            });
        if (noise - self.prefs.compare_noise_pct.unwrap_or(DEFAULT_NOISE_PCT)).abs() > f64::EPSILON
        {
            self.prefs.compare_noise_pct = Some(noise);
        }
        if swap && let Some(view) = &mut self.comparison {
            std::mem::swap(&mut view.base, &mut view.candidate);
            self.compare_base = Some(view.base.id.clone());
        }
        if close {
            self.comparison = None;
        }
    }
}

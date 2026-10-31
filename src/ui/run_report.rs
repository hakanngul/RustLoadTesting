//! Report: a read-only summary of the run Monitor has open (live or history),
//! laid out to answer "did it pass?" first and "where is the problem?" next.
//! It has no editable layout and never changes Monitor's pages.
use super::*;
use crate::metrics::{SloOutcome, ThresholdStatus};

/// Endpoint table column the Report sorts by.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SortBy {
    Request,
    Completed,
    Errors,
    P50,
    #[default]
    P95,
    P99,
}

impl RustLoadApp {
    pub(super) fn report(&mut self, ctx: &egui::Context, p: Palette) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.snapshot.run_id.is_empty() {
                ui.add_space(24.0);
                ui.heading("No run to report yet");
                ui.label(
                    RichText::new("Run a test, or open one from Test Runs on the left.")
                        .color(p.muted),
                );
                return;
            }
            // Measured outside the scroll area: inside it the width can lag a
            // window resize by the content's previous size.
            let width = ui.available_width();
            let wide = width >= 1000.0;
            egui::ScrollArea::vertical()
                .id_salt("report_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.spacing_mut().item_spacing.y = 12.0;
                    let s = &self.snapshot;
                    verdict_band(ui, s, &self.report_plan, p);
                    key_metrics(ui, s, p);
                    secondary_metrics(ui, s, p);
                    if wide {
                        ui.horizontal_top(|ui| {
                            let left = (ui.available_width() - 12.0) * 0.64;
                            ui.allocate_ui_with_layout(
                                egui::vec2(left, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| timeline(ui, s, p),
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| side_panels(ui, s, p),
                            );
                        });
                    } else {
                        timeline(ui, s, p);
                        side_panels(ui, s, p);
                    }
                    let mut sort = self.report_sort;
                    endpoint_table(ui, s, p, &mut sort);
                    self.report_sort = sort;
                    self.logs_drawer(ui, p);
                });
        });
    }

    /// Logs stay folded to one line until asked for; "Errors only" opens them
    /// filtered to ERROR.
    fn logs_drawer(&mut self, ui: &mut egui::Ui, p: Palette) {
        let (info, errors) = self.snapshot.logs.iter().fold((0, 0), |(i, e), l| {
            if l.level == "ERROR" {
                (i, e + 1)
            } else {
                (i + 1, e)
            }
        });
        theme::panel(p).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.strong("Live Logs");
                ui.label(
                    RichText::new(format!("· {info} INFO · {errors} ERROR (kept events)"))
                        .color(if errors > 0 { p.error } else { p.muted }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.report_logs_open {
                        "Hide logs"
                    } else {
                        "Show logs"
                    };
                    if ui.button(label).clicked() {
                        self.report_logs_open = !self.report_logs_open;
                    }
                    if ui.button("Errors only").clicked() {
                        self.level = "ERROR".into();
                        self.report_logs_open = true;
                    }
                });
            });
            if self.report_logs_open {
                self.logs_for(ui, p, false, 240.0);
            }
        });
    }
}

/// Verdict word and colour: the SLO verdict when the plan has thresholds,
/// otherwise how the run ended and whether anything failed.
fn verdict(s: &Snapshot, p: Palette) -> (&'static str, Color32) {
    if !s.state.terminal() {
        return ("RUNNING", p.accent);
    }
    match &s.slo {
        Some(SloOutcome {
            verdict: Verdict::Passed,
            ..
        }) => ("PASSED", p.success),
        Some(SloOutcome {
            verdict: Verdict::Failed,
            ..
        }) => ("FAILED", p.error),
        Some(_) => ("INCONCLUSIVE", p.warning),
        None => match s.state {
            RunState::Completed if s.counts.failed == 0 => ("COMPLETED", p.success),
            RunState::Completed => ("COMPLETED WITH ERRORS", p.warning),
            RunState::Cancelled => ("CANCELLED", p.warning),
            _ => ("FAILED", p.error),
        },
    }
}

fn verdict_band(ui: &mut egui::Ui, s: &Snapshot, plan: &TestPlan, p: Palette) {
    let (word, color) = verdict(s, p);
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .stroke(egui::Stroke::new(1.0_f32, color))
        .corner_radius(14)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                egui::Frame::new()
                    .fill(color)
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.label(RichText::new(word).size(20.0).strong().color(p.on_accent));
                    });
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&s.name).size(18.0).strong());
                        ui.label(
                            RichText::new(format!(
                                "{:?} · {:02}:{:02}",
                                s.state,
                                s.elapsed as u64 / 60,
                                s.elapsed as u64 % 60
                            ))
                            .color(color),
                        );
                    });
                    let load = &plan.load;
                    let mut summary = if load.profile == Profile::ArrivalRate {
                        format!("{} · {} s", load.profile_label(), load.total_secs())
                    } else {
                        format!(
                            "{} users · {} · {} s",
                            load.max_users(),
                            load.profile_label(),
                            load.total_secs()
                        )
                    };
                    summary.push_str(&format!(
                        " · {} requests · {} failed",
                        s.counts.completed, s.counts.failed
                    ));
                    if s.warmup != Warmup::None {
                        summary.push_str(&format!(
                            " · warmup {} excluded ({} requests)",
                            s.warmup.describe(),
                            s.warmup_counts.started
                        ));
                    }
                    ui.add(egui::Label::new(RichText::new(summary).color(p.muted)).wrap());
                });
            });
            if let Some(outcome) = &s.slo {
                ui.add_space(8.0);
                // Framed chips are groups, which horizontal_wrapped cannot
                // break; rows are filled by measured width instead.
                let chips: Vec<_> = outcome
                    .results
                    .iter()
                    .map(|r| {
                        let (mark, color) = match r.status {
                            ThresholdStatus::Pass => ("Pass", p.success),
                            ThresholdStatus::Fail => ("Fail", p.error),
                            ThresholdStatus::NoData => ("No data", p.warning),
                        };
                        (mark, color, r.threshold.describe(), r.actual_label())
                    })
                    .collect();
                let text_width = |ui: &egui::Ui, text: &str| {
                    ui.painter()
                        .layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.0), p.text)
                        .size()
                        .x
                };
                let gap = ui.spacing().item_spacing.x;
                let mut rows: Vec<Vec<usize>> = vec![vec![]];
                let mut used = 0.0;
                for (i, (mark, _, rule, actual)) in chips.iter().enumerate() {
                    let width = 20.0 + 10.0 + 3.0 * gap
                        + text_width(ui, mark)
                        + text_width(ui, rule)
                        + text_width(ui, actual)
                        + 6.0;
                    if used > 0.0 && used + width > ui.available_width() {
                        rows.push(vec![]);
                        used = 0.0;
                    }
                    used += width + gap;
                    rows.last_mut().expect("a row").push(i);
                }
                for row in rows {
                    ui.horizontal(|ui| {
                        for i in row {
                            let (mark, color, rule, actual) = &chips[i];
                            egui::Frame::new()
                                .fill(p.surface)
                                .stroke(egui::Stroke::new(1.0_f32, p.border))
                                .corner_radius(6)
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (dot, _) = ui.allocate_exact_size(
                                            egui::vec2(10.0, 10.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(dot.center(), 4.5, *color);
                                        ui.label(RichText::new(*mark).size(12.0).strong().color(*color));
                                        ui.label(RichText::new(rule).size(12.0));
                                        ui.label(RichText::new(actual).size(12.0).strong());
                                    });
                                });
                        }
                    });
                }
            } else if s.state.terminal() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("No SLO thresholds in this plan · add them in plan settings for a Passed/Failed verdict")
                        .size(12.0)
                        .color(p.muted),
                );
            }
        });
}

/// The four numbers that matter most, large; the rest go in the strip below.
fn key_metrics(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    let p95_slo = s.slo.as_ref().and_then(|o| {
        o.results
            .iter()
            .find(|r| r.threshold.metric == SloMetric::P95 && r.threshold.request_id.is_none())
            .map(|r| r.threshold.value)
    });
    let measured = s.measured_elapsed();
    let throughput_avg = (measured > 0.0).then(|| s.counts.completed as f64 / measured);
    let error_rate = s.counts.error_rate();
    let columns = if ui.available_width() >= 760.0 { 4 } else { 2 };
    let cards: [Card; 4] = [
        Box::new(|ui| {
            card_head(ui, p, "Throughput");
            metric_value(ui, &format!("{:.1} req/s", s.rps), p.accent, 30.0);
            card_note(
                ui,
                p,
                &throughput_avg.map_or("Last 5 s".into(), |v| {
                    format!("Last 5 s · measured window avg {v:.2}")
                }),
            );
        }),
        Box::new(|ui| {
            card_head(ui, p, "P95 latency");
            metric_value(ui, &latency_label(&s.latency, s.latency.p95), p.p95, 30.0);
            match p95_slo {
                Some(limit) if s.latency.samples > 0 => {
                    let share = (s.latency.p95 / limit) as f32;
                    ui.horizontal(|ui| {
                        let fill = if share > 1.0 { p.error } else { p.p95 };
                        theme::meter(ui, p, share, fill, (ui.available_width() - 130.0).max(40.0));
                        card_note(
                            ui,
                            p,
                            &format!("{:.0}% of {limit:.0} ms SLO", share * 100.0),
                        );
                    });
                }
                _ => card_note(ui, p, "Whole run, fully read responses"),
            }
        }),
        Box::new(|ui| {
            card_head(ui, p, "Error rate");
            let color = if s.counts.failed > 0 {
                p.error
            } else {
                p.success
            };
            metric_value(ui, &format!("{error_rate:.2} %"), color, 30.0);
            card_note(
                ui,
                p,
                &format!("{} of {} completed", s.counts.failed, s.counts.completed),
            );
        }),
        Box::new(|ui| {
            card_head(ui, p, "Virtual users");
            // A finished run has none left running; show the peak it reached.
            let (users, note) = if s.state.terminal() {
                let peak = s.series.iter().map(|v| v.users).max().unwrap_or(0);
                (peak, "Peak reached / target")
            } else {
                (s.active_users, "Running now / target")
            };
            metric_value(
                ui,
                &format!("{users} / {}", s.target_users),
                p.success,
                30.0,
            );
            card_note(ui, p, note);
        }),
    ];
    for row in cards.chunks(columns) {
        ui.columns(columns, |cols| {
            for (col, card) in cols.iter_mut().zip(row) {
                theme::panel(p).show(col, |ui| {
                    ui.set_min_width((ui.available_width() - 4.0).max(40.0));
                    card(ui);
                });
            }
        });
    }
}

/// One key-metric card's content.
type Card<'a> = Box<dyn Fn(&mut egui::Ui) + 'a>;

fn card_head(ui: &mut egui::Ui, p: Palette, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(p.muted));
}

fn card_note(ui: &mut egui::Ui, p: Palette, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(11.0).color(p.muted)).truncate());
}

fn secondary_metrics(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 24.0;
            let item = |ui: &mut egui::Ui, label: &str, value: String, color: Color32| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new(label).color(p.muted));
                    ui.label(RichText::new(value).strong().color(color));
                });
            };
            item(ui, "P50", latency_label(&s.latency, s.latency.p50), p.p50);
            item(ui, "P99", latency_label(&s.latency, s.latency.p99), p.p99);
            item(
                ui,
                "Data / sec",
                format!("{}/s", format_bytes(s.bytes_rate)),
                p.text,
            );
            item(ui, "Requests", s.counts.completed.to_string(), p.text);
            if s.warmup != Warmup::None {
                item(
                    ui,
                    "Warmup excluded",
                    s.warmup_counts.started.to_string(),
                    p.text,
                );
            }
            if s.counts.dropped_iterations > 0 {
                item(
                    ui,
                    "Dropped iterations",
                    s.counts.dropped_iterations.to_string(),
                    p.warning,
                );
            }
        });
    });
}

/// Requests/sec, response time and users on one time axis: the x axes and the
/// hover cursor are linked, the warmup is shaded and the P95/P99 SLO limits
/// for all requests are drawn as dashed lines.
fn timeline(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Timeline").size(16.0).strong());
        ui.label(
            RichText::new("One time axis: zoom or hover one chart and the others follow · double-click to reset")
                .size(11.0)
                .color(p.muted),
        );
        // Zoom, hidden series and the axis link belong to one run, so another
        // run opens with its whole timeline.
        let link = egui::Id::new(("report_timeline", &s.run_id));
        let warmup_end = (s.warmup != Warmup::None && s.measured_from > 0.0).then_some(s.measured_from);
        let rate = |f: fn(&Point) -> u64| -> Vec<[f64; 2]> {
            s.series
                .iter()
                .map(|v| [v.elapsed, f(v) as f64 / v.interval_secs.max(0.001)])
                .collect()
        };

        // Requests per second.
        let mut series = vec![("Total", p.accent), ("Failed", p.error)];
        if warmup_end.is_some() {
            series.push(("Warmup", p.muted));
        }
        let header = PlotHeader::show(ui, p, "Requests / sec", ("report_rps", &s.run_id), &series);
        let total = rate(|v| v.completed);
        let peak = total.iter().map(|v| v[1]).fold(1.0, f64::max);
        timeline_plot("report_rps", header.id, 170.0, link)
            .show(ui, |plot| {
                start_plot(plot, &header, warmup_end, peak * 1.1, p);
                if header.visible("Total") {
                    plot.line(Line::new("Total", PlotPoints::from(total)).color(p.accent).width(2.0_f32));
                }
                if header.visible("Failed") {
                    plot.line(Line::new("Failed", PlotPoints::from(rate(|v| v.failed))).color(p.error).width(1.6_f32));
                }
                if warmup_end.is_some() && header.visible("Warmup") {
                    let mut points = rate(|v| v.warmup_completed);
                    let end = s
                        .series
                        .iter()
                        .rposition(|v| v.warmup_completed > 0)
                        .map_or(0, |i| (i + 2).min(points.len()));
                    points.truncate(end);
                    plot.line(Line::new("Warmup", PlotPoints::from(points)).color(p.muted).width(1.6_f32));
                }
            });

        // Response time with the SLO limits.
        // One limit keeps the data readable: the all-requests P95 SLO, else P99.
        let limit_for = |metric: SloMetric| {
            s.slo.iter().flat_map(|o| &o.results).find_map(|r| {
                (r.threshold.request_id.is_none() && r.threshold.metric == metric)
                    .then_some((metric, r.threshold.value))
            })
        };
        let measured: Vec<&Point> = s.series.iter().filter(|v| v.latency.samples > 0).collect();
        let peak = measured.iter().map(|v| v.latency.p99).fold(1.0, f64::max);
        let limit = limit_for(SloMetric::P95).or_else(|| limit_for(SloMetric::P99));
        // A limit far above the data would flatten it; name it instead.
        let off_scale = limit.filter(|(_, value)| !measured.is_empty() && *value > peak * 2.5);
        let limits: Vec<(SloMetric, f64)> = limit.filter(|_| off_scale.is_none()).into_iter().collect();
        let mut series = vec![("P50", p.p50), ("P95", p.p95), ("P99", p.p99)];
        if !limits.is_empty() {
            series.push(("SLO limit", p.warning));
        }
        let header = PlotHeader::show(ui, p, "Response time (ms) · interval percentiles", ("report_latency", &s.run_id), &series);
        if let Some((metric, value)) = off_scale {
            ui.label(
                RichText::new(format!(
                    "{} SLO ≤ {value:.0} ms is far above this run (highest interval P99 {} ms), so it is not drawn",
                    metric.label(),
                    short_ms(peak)
                ))
                .size(11.0)
                .color(p.muted),
            );
        }
        let mut plot = timeline_plot("report_latency", header.id, 190.0, link);
        if header.visible("SLO limit") {
            for (_, value) in &limits {
                plot = plot.include_y(*value * 1.05);
            }
        }
        plot.show(ui, |plot| {
            let top = limits.iter().map(|l| l.1).fold(peak, f64::max) * 1.1;
            start_plot(plot, &header, warmup_end, top, p);
            for (name, color, f) in [
                ("P50", p.p50, (|v: &Point| v.latency.p50) as fn(&Point) -> f64),
                ("P95", p.p95, |v: &Point| v.latency.p95),
                ("P99", p.p99, |v: &Point| v.latency.p99),
            ] {
                if header.visible(name) {
                    let points: Vec<[f64; 2]> = measured.iter().map(|v| [v.elapsed, f(v)]).collect();
                    plot.line(Line::new(name, PlotPoints::from(points)).color(color).width(1.8_f32));
                }
            }
            if header.visible("SLO limit") {
                for (metric, value) in &limits {
                    plot.hline(
                        egui_plot::HLine::new(format!("{} SLO ≤ {value:.0} ms", metric.label()), *value)
                            .color(p.warning)
                            .style(egui_plot::LineStyle::dashed_loose()),
                    );
                }
            }
        });

        // Active users.
        let header = PlotHeader::show(ui, p, "Active virtual users", ("report_users", &s.run_id), &[("Users", p.success)]);
        let users: Vec<[f64; 2]> = s.series.iter().map(|v| [v.elapsed, v.users as f64]).collect();
        let peak = users.iter().map(|v| v[1]).fold(1.0, f64::max);
        timeline_plot("report_users", header.id, 90.0, link)
            .show(ui, |plot| {
                start_plot(plot, &header, warmup_end, peak * 1.1, p);
                if header.visible("Users") {
                    plot.line(Line::new("Users", PlotPoints::from(users)).color(p.success).width(2.0_f32));
                }
            });
        ui.label(RichText::new("Elapsed seconds · grey band: warmup, not measured").size(11.0).color(p.muted));
    });
}

/// 1-2-5 steps for about five labelled y lines. egui_plot's default uses
/// powers of ten only, which leaves a 0–900 ms axis with a single label.
fn nice_steps(input: egui_plot::GridInput) -> [f64; 3] {
    let range = (input.bounds.1 - input.bounds.0).abs().max(f64::EPSILON);
    let rough = range / 5.0;
    let magnitude = 10_f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= rough)
        .unwrap_or(10.0 * magnitude);
    [step / 5.0, step, step * 5.0]
}

/// One of the timeline charts: x axis and cursor linked to the others, the
/// same y axis width so their time axes line up, 1-2-5 y labels.
fn timeline_plot(name: &str, id: egui::Id, height: f32, link: egui::Id) -> Plot<'_> {
    Plot::new(name)
        .id(id)
        .height(height)
        .include_y(0.0)
        .link_axis(link, [true, false])
        .link_cursor(link, [true, false])
        .y_axis_min_width(48.0)
        .y_grid_spacer(egui_plot::uniform_grid_spacer(nice_steps))
}

/// Applies a pending "Reset zoom" and shades the warmup up to `top`.
fn start_plot(
    plot: &mut egui_plot::PlotUi,
    header: &PlotHeader,
    warmup_end: Option<f64>,
    top: f64,
    p: Palette,
) {
    if header.reset {
        plot.set_auto_bounds(true);
    }
    shade_warmup(plot, warmup_end, top, p);
}

/// Grey band over the warmup seconds.
fn shade_warmup(plot: &mut egui_plot::PlotUi, end: Option<f64>, top: f64, p: Palette) {
    if let Some(end) = end {
        plot.polygon(
            egui_plot::Polygon::new(
                "Warmup (not measured)",
                PlotPoints::from(vec![[0.0, 0.0], [end, 0.0], [end, top], [0.0, top]]),
            )
            .fill_color(p.muted.gamma_multiply(0.15))
            .stroke(egui::Stroke::NONE),
        );
    }
}

fn side_panels(ui: &mut egui::Ui, s: &Snapshot, p: Palette) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("Why did requests fail?").size(16.0).strong());
        if s.errors.is_empty() && s.counts.failed == 0 {
            egui::Frame::new()
                .fill(p.success.gamma_multiply(0.12))
                .corner_radius(10)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("No failures.").strong().color(p.success));
                        ui.label(format!(
                            "{} of {} completed requests succeeded.",
                            s.counts.successful, s.counts.completed
                        ));
                    });
                });
        } else {
            ui.label(
                RichText::new(format!(
                    "{} failed of {} completed",
                    s.counts.failed, s.counts.completed
                ))
                .color(p.error),
            );
            failure_reasons(ui, s, p);
        }
    });
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("HTTP status").size(16.0).strong());
        let total: u64 = s.statuses.values().sum();
        if total == 0 {
            ui.label(RichText::new("No HTTP responses").color(p.muted));
        }
        for (status, count) in &s.statuses {
            let share = *count as f32 / total.max(1) as f32;
            ui.horizontal(|ui| {
                ui.label(status.to_string());
                let fill = if *status < 400 { p.success } else { p.error };
                theme::meter(ui, p, share, fill, (ui.available_width() - 110.0).max(40.0));
                ui.label(
                    RichText::new(format!("{count} · {:.1}%", share * 100.0))
                        .size(12.0)
                        .color(p.muted),
                );
            });
        }
        ui.label(
            RichText::new(format!(
                "Network {} · Timeout {} · Body limit {} · Assertion {} · Cancelled {}",
                s.counts.network_errors,
                s.counts.timeouts,
                s.counts.body_errors,
                s.counts.assertion_errors,
                s.counts.cancelled
            ))
            .size(12.0)
            .color(p.muted),
        );
    });
    let mut ranked: Vec<&Endpoint> = s
        .endpoints
        .iter()
        .filter(|e| e.latency.samples > 0)
        .collect();
    ranked.sort_by(|a, b| b.latency.p95.total_cmp(&a.latency.p95));
    if let Some(slowest) = ranked.first() {
        theme::panel(p).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new("Slowest request (P95)")
                    .size(12.0)
                    .color(p.muted),
            );
            ui.label(
                RichText::new(format!("{} {}", slowest.method, slowest.name))
                    .size(18.0)
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(short_ms(slowest.latency.p95) + " ms")
                        .strong()
                        .color(p.p95),
                );
                if let Some(next) = ranked.get(1).filter(|n| n.latency.p95 > 0.0) {
                    ui.label(
                        RichText::new(format!(
                            "· {:.1}× the next slowest ({} {} ms)",
                            slowest.latency.p95 / next.latency.p95,
                            next.name,
                            short_ms(next.latency.p95)
                        ))
                        .color(p.muted),
                    );
                }
            });
        });
    }
}

fn endpoint_table(ui: &mut egui::Ui, s: &Snapshot, p: Palette, sort: &mut (SortBy, bool)) {
    theme::panel(p).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Endpoints").size(16.0).strong());
            ui.label(
                RichText::new("· click a column to sort")
                    .size(12.0)
                    .color(p.muted),
            );
        });
        let mut rows: Vec<&Endpoint> = s.endpoints.iter().collect();
        let key = |e: &Endpoint, by: SortBy| -> f64 {
            match by {
                SortBy::Request => 0.0,
                SortBy::Completed => e.counts.completed as f64,
                SortBy::Errors => e.counts.failed as f64,
                SortBy::P50 => e.latency.p50,
                SortBy::P95 => e.latency.p95,
                SortBy::P99 => e.latency.p99,
            }
        };
        let (by, descending) = *sort;
        rows.sort_by(|a, b| {
            let order = if by == SortBy::Request {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else {
                key(a, by).total_cmp(&key(b, by))
            };
            if descending { order.reverse() } else { order }
        });
        let slowest = s
            .endpoints
            .iter()
            .filter(|e| e.latency.samples > 0)
            .map(|e| e.latency.p95)
            .fold(0.0, f64::max);
        egui::ScrollArea::horizontal()
            .id_salt("report_endpoints")
            .show(ui, |ui| {
                egui::Grid::new("report_endpoint_grid")
                    .striped(true)
                    .min_col_width(70.0)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        for (title, column) in [
                            ("Request", Some(SortBy::Request)),
                            ("Method", None),
                            ("Completed", Some(SortBy::Completed)),
                            ("Errors", Some(SortBy::Errors)),
                            ("P50", Some(SortBy::P50)),
                            ("P95", Some(SortBy::P95)),
                            ("P99", Some(SortBy::P99)),
                            ("P95 vs slowest", None),
                        ] {
                            match column {
                                Some(column) => {
                                    let active = sort.0 == column;
                                    let text = RichText::new(title).strong().color(if active {
                                        p.text
                                    } else {
                                        p.muted
                                    });
                                    let response = ui.add(egui::Button::new(text).frame(false));
                                    if active {
                                        // Triangle after the title: down = largest first.
                                        let r = response.rect;
                                        let c = egui::pos2(r.right() + 4.0, r.center().y);
                                        let points = if sort.1 {
                                            vec![
                                                c + egui::vec2(-4.0, -2.5),
                                                c + egui::vec2(4.0, -2.5),
                                                c + egui::vec2(0.0, 3.5),
                                            ]
                                        } else {
                                            vec![
                                                c + egui::vec2(-4.0, 2.5),
                                                c + egui::vec2(4.0, 2.5),
                                                c + egui::vec2(0.0, -3.5),
                                            ]
                                        };
                                        ui.painter().add(egui::Shape::convex_polygon(
                                            points,
                                            p.accent,
                                            egui::Stroke::NONE,
                                        ));
                                    }
                                    if response.on_hover_text("Sort by this column").clicked() {
                                        // A new column starts with the largest (A–Z for names).
                                        *sort = if sort.0 == column {
                                            (column, !sort.1)
                                        } else {
                                            (column, column != SortBy::Request)
                                        };
                                    }
                                }
                                None => {
                                    ui.label(RichText::new(title).strong().color(p.muted));
                                }
                            }
                        }
                        ui.end_row();
                        for e in rows {
                            let is_slowest =
                                slowest > 0.0 && e.latency.samples > 0 && e.latency.p95 >= slowest;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&e.name).strong());
                                if is_slowest && s.endpoints.len() > 1 {
                                    ui.label(RichText::new("slowest").size(11.0).color(p.p95));
                                }
                            });
                            ui.label(&e.method);
                            ui.label(e.counts.completed.to_string());
                            ui.label(
                                RichText::new(e.counts.failed.to_string())
                                    .color(if e.counts.failed > 0 { p.error } else { p.text }),
                            );
                            ui.label(latency_label(&e.latency, e.latency.p50));
                            ui.label(
                                RichText::new(latency_label(&e.latency, e.latency.p95)).strong(),
                            );
                            ui.label(latency_label(&e.latency, e.latency.p99));
                            let share = if slowest > 0.0 && e.latency.samples > 0 {
                                (e.latency.p95 / slowest) as f32
                            } else {
                                0.0
                            };
                            theme::meter(ui, p, share, p.p95, 180.0);
                            ui.end_row();
                        }
                    });
            });
    });
}

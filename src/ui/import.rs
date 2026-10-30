//! Import dialog: browser "Copy as cURL" text or a HAR file into the plan.
use super::*;
use crate::import::{HAR_SIZE_LIMIT, IMPORT_LIMIT, Imported, from_curl, from_har};

pub(super) struct ImportDialog {
    har: bool,
    curl: String,
    path: String,
    api_only: bool,
    replace: bool,
    preview: Option<Result<Imported, String>>,
}
impl Default for ImportDialog {
    fn default() -> Self {
        Self {
            har: false,
            curl: String::new(),
            path: String::new(),
            api_only: true,
            replace: false,
            preview: None,
        }
    }
}
impl ImportDialog {
    fn parse(&self, first_id: u64) -> Result<Imported, String> {
        if !self.har {
            return from_curl(&self.curl, first_id);
        }
        let path = self.path.trim();
        let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
        if meta.len() > HAR_SIZE_LIMIT {
            return Err("HAR file exceeds 50 MiB".into());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        from_har(&bytes, first_id, self.api_only)
    }
}

impl RustLoadApp {
    pub(super) fn import_dialog(&mut self, ctx: &egui::Context, p: Palette) {
        let Some(dialog) = &mut self.import else {
            return;
        };
        let first_id = if dialog.replace {
            1
        } else {
            self.plan.requests.iter().map(|r| r.id).max().unwrap_or(0) + 1
        };
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Import requests")
            .open(&mut open)
            .collapsible(false)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.selectable_label(!dialog.har, "Copy as cURL").clicked() {
                        dialog.har = false;
                        dialog.preview = None;
                    }
                    if ui.selectable_label(dialog.har, "HAR file").clicked() {
                        dialog.har = true;
                        dialog.preview = None;
                    }
                });
                ui.separator();
                let before = (dialog.curl.clone(), dialog.path.clone(), dialog.api_only);
                if dialog.har {
                    ui.label("HAR file path (DevTools > Network > Save all as HAR)");
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(f32::INFINITY));
                    ui.checkbox(&mut dialog.api_only, "Only XHR/fetch requests (skip pages, scripts, styles, images)");
                } else {
                    ui.label("Paste one or more commands from DevTools > Copy as cURL (bash)");
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut dialog.curl)
                                .code_editor()
                                .desired_rows(7)
                                .desired_width(f32::INFINITY)
                                .hint_text("curl 'https://api.example.com/login' -H 'content-type: application/json' --data-raw '{…}'"),
                        );
                    });
                }
                if before != (dialog.curl.clone(), dialog.path.clone(), dialog.api_only) {
                    dialog.preview = None;
                }
                ui.horizontal(|ui| {
                    ui.radio_value(&mut dialog.replace, false, "Add to current requests");
                    ui.radio_value(&mut dialog.replace, true, "Replace current requests");
                });
                ui.label(
                    RichText::new(
                        "Only requests are imported, never responses. Authorization, Cookie and other credential values stay in this session and are not saved. Bodies are session-only unless you enable Save body.",
                    )
                    .small()
                    .color(p.muted),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Preview").clicked() {
                        dialog.preview = Some(dialog.parse(first_id));
                    }
                    let ready = matches!(&dialog.preview, Some(Ok(i)) if !i.requests.is_empty());
                    if ui.add_enabled(ready, egui::Button::new("Import")).clicked() {
                        apply = true;
                    }
                });
                match &dialog.preview {
                    None => {}
                    Some(Err(e)) => {
                        ui.colored_label(p.error, e);
                    }
                    Some(Ok(imported)) => {
                        ui.separator();
                        ui.strong(format!("{} request(s) ready", imported.requests.len()));
                        egui::ScrollArea::vertical().id_salt("import_preview").max_height(200.0).show(ui, |ui| {
                            for r in imported.requests.iter().take(50) {
                                ui.label(RichText::new(format!("{} {}", r.method, r.url)).monospace().size(12.0));
                            }
                            if imported.requests.len() > 50 {
                                ui.label(RichText::new(format!("…and {} more", imported.requests.len() - 50)).color(p.muted));
                            }
                            for w in &imported.warnings {
                                ui.colored_label(p.warning, format!("• {w}"));
                            }
                        });
                    }
                }
            });
        if apply {
            // Parse again so IDs follow the plan as it is now.
            let replace = dialog.replace;
            match dialog.parse(first_id) {
                Ok(imported) => self.apply_import(imported, replace),
                Err(e) => dialog.preview = Some(Err(e)),
            }
        } else if !open {
            self.import = None;
        }
    }

    fn apply_import(&mut self, imported: Imported, replace: bool) {
        if self.busy() {
            self.message = "Stop the running test before importing requests.".into();
            return;
        }
        let count = imported.requests.len();
        let existing = if replace { 0 } else { self.plan.requests.len() };
        if existing + count > IMPORT_LIMIT {
            self.message = format!(
                "Import would exceed {IMPORT_LIMIT} requests ({existing} + {count}); replace instead or import fewer"
            );
            return;
        }
        let mut note = String::new();
        if replace {
            // New requests reuse low IDs, so request-scoped thresholds would point at the wrong step.
            let before = self.plan.thresholds.len();
            self.plan.thresholds.retain(|t| t.request_id.is_none());
            let removed = before - self.plan.thresholds.len();
            if removed > 0 {
                note = format!(" Removed {removed} request-scoped SLO threshold(s).");
            }
            self.plan.requests = imported.requests;
            self.selected = 0;
        } else {
            self.selected = self.plan.requests.len();
            self.plan.requests.extend(imported.requests);
        }
        self.plan_settings = false;
        self.dirty = true;
        self.acknowledged = false;
        self.import = None;
        self.message = format!(
            "Imported {count} request(s). Review them and authorize the targets before running.{note}"
        );
    }
}

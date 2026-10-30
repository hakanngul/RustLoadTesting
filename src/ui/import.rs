//! Import dialog: browser "Copy as cURL" text, a HAR file or an OpenAPI
//! document into the plan.
use super::*;
use crate::import::{HAR_SIZE_LIMIT, IMPORT_LIMIT, Imported, from_curl, from_har, from_openapi};

#[derive(Clone, Copy, PartialEq)]
enum Source {
    Curl,
    Har,
    OpenApi,
}

pub(super) struct ImportDialog {
    source: Source,
    curl: String,
    path: String,
    api_only: bool,
    /// OpenAPI only: overrides the document's server URL when set.
    base_url: String,
    replace: bool,
    preview: Option<Result<Imported, String>>,
}
impl Default for ImportDialog {
    fn default() -> Self {
        Self {
            source: Source::Curl,
            curl: String::new(),
            path: String::new(),
            api_only: true,
            base_url: String::new(),
            replace: false,
            preview: None,
        }
    }
}
impl ImportDialog {
    fn parse(&self, first_id: u64) -> Result<Imported, String> {
        if self.source == Source::Curl {
            return from_curl(&self.curl, first_id);
        }
        let path = self.path.trim();
        let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
        if meta.len() > HAR_SIZE_LIMIT {
            return Err("File exceeds 50 MiB".into());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if self.source == Source::Har {
            from_har(&bytes, first_id, self.api_only)
        } else {
            from_openapi(&bytes, first_id, &self.base_url)
        }
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
                    for (source, label) in [
                        (Source::Curl, "Copy as cURL"),
                        (Source::Har, "HAR file"),
                        (Source::OpenApi, "OpenAPI (JSON)"),
                    ] {
                        if ui.selectable_label(dialog.source == source, label).clicked() {
                            dialog.source = source;
                            dialog.preview = None;
                        }
                    }
                });
                ui.separator();
                let before = (
                    dialog.curl.clone(),
                    dialog.path.clone(),
                    dialog.api_only,
                    dialog.base_url.clone(),
                );
                if dialog.source == Source::Har {
                    ui.label("HAR file path (DevTools > Network > Save all as HAR)");
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(f32::INFINITY));
                    ui.checkbox(&mut dialog.api_only, "Only XHR/fetch requests (skip pages, scripts, styles, images)");
                } else if dialog.source == Source::OpenApi {
                    ui.label("OpenAPI 3.x or Swagger 2.0 JSON file path");
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(f32::INFINITY));
                    ui.label("Base URL (optional; replaces the document's server URL)");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.base_url)
                            .desired_width(f32::INFINITY)
                            .hint_text("e.g. https://staging.example.com"),
                    );
                    ui.label(
                        RichText::new(
                            "One request per GET/POST/PUT/PATCH/DELETE operation. Path parameters become {{variables}}; required query/header parameters and JSON bodies use the document's examples.",
                        )
                        .small()
                        .color(p.muted),
                    );
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
                if before
                    != (
                        dialog.curl.clone(),
                        dialog.path.clone(),
                        dialog.api_only,
                        dialog.base_url.clone(),
                    )
                {
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
        // Path parameters and credentials the requests refer to; existing values win.
        let mut added = Vec::new();
        for v in imported.variables {
            if !self.plan.variables.iter().any(|f| f.name == v.name) {
                added.push(v.name.clone());
                self.plan.variables.push(v);
            }
        }
        if !added.is_empty() {
            note.push_str(&format!(" Added variables: {}.", added.join(", ")));
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

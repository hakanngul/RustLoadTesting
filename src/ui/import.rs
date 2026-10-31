//! Import dialog: browser "Copy as cURL" text, a HAR file or an OpenAPI
//! document into the plan.
use super::*;
use crate::import::{
    IMPORT_LIMIT, Imported, OpenApiSpec, ServerChoice, from_curl, from_har, load_openapi,
    read_import_file,
};
use std::collections::BTreeSet;

const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];

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
    /// OpenAPI only: the parsed document and the operation selection.
    picker: Option<Picker>,
}

/// Chooses which OpenAPI operations to import. The document is parsed once;
/// requests are built for the selection only when it changes.
struct Picker {
    spec: OpenApiSpec,
    selected: BTreeSet<String>,
    filter: String,
    method: Option<&'static str>,
    tag: Option<String>,
    tags: Vec<String>,
    /// Server and variable values from the document's lists.
    choice: ServerChoice,
    /// The base URL requests get (from `choice` or the Base URL field).
    base: Result<String, String>,
    /// Requests of the current selection, for the warnings; rebuilt on change.
    built: Option<Imported>,
}
impl Picker {
    fn new(spec: OpenApiSpec, base_override: &str) -> Self {
        let mut picker = Self {
            selected: spec.operations.iter().map(|o| o.key.clone()).collect(),
            tags: spec.tags(),
            choice: ServerChoice::defaults(&spec, 0),
            spec,
            filter: String::new(),
            method: None,
            tag: None,
            base: Ok(String::new()),
            built: None,
        };
        picker.select_server(base_override);
        picker
    }
    /// Re-derives the base URL from the choice and the Base URL field.
    fn select_server(&mut self, base_override: &str) {
        self.base = self.spec.select_server(&self.choice, base_override);
        self.built = None;
    }
    /// Indices of the operations the filters show.
    fn visible(&self) -> Vec<usize> {
        self.spec
            .operations
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                o.matches(&self.filter)
                    && self.method.is_none_or(|m| o.method == m)
                    && self.tag.as_ref().is_none_or(|t| o.tags.contains(t))
            })
            .map(|(i, _)| i)
            .collect()
    }
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
            picker: None,
        }
    }
}
impl ImportDialog {
    fn parse(&self, first_id: u64) -> Result<Imported, String> {
        if self.source == Source::Curl {
            return from_curl(&self.curl, first_id);
        }
        let bytes = read_import_file(&self.path)?;
        from_har(&bytes, first_id, self.api_only)
    }

    /// Reads the document and the local files it refers to (relative to its
    /// folder).
    fn load_openapi(&self) -> Result<OpenApiSpec, String> {
        let path = self.path.trim();
        let bytes = read_import_file(path)?;
        let dir = std::path::Path::new(path)
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        load_openapi(&bytes, Some(dir))
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
        // Requests the plan can still take with the chosen mode.
        let existing = self.plan.requests.len();
        let room = if dialog.replace {
            IMPORT_LIMIT
        } else {
            IMPORT_LIMIT.saturating_sub(existing)
        };
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Import requests")
            .open(&mut open)
            .collapsible(false)
            .default_width(680.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (source, label) in [
                        (Source::Curl, "Copy as cURL"),
                        (Source::Har, "HAR file"),
                        (Source::OpenApi, "OpenAPI (JSON/YAML)"),
                    ] {
                        if ui.selectable_label(dialog.source == source, label).clicked() {
                            dialog.source = source;
                            dialog.preview = None;
                            dialog.picker = None;
                        }
                    }
                });
                ui.separator();
                let before = (dialog.curl.clone(), dialog.path.clone(), dialog.api_only);
                let base_before = dialog.base_url.clone();
                if dialog.source == Source::Har {
                    ui.label("HAR file path (DevTools > Network > Save all as HAR)");
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(f32::INFINITY));
                    ui.checkbox(&mut dialog.api_only, "Only XHR/fetch requests (skip pages, scripts, styles, images)");
                } else if dialog.source == Source::OpenApi {
                    ui.label("OpenAPI 3.x or Swagger 2.0 file path (.json, .yaml or .yml)");
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(f32::INFINITY));
                    ui.label("Base URL (optional; replaces the selected server)");
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.base_url)
                            .desired_width(f32::INFINITY)
                            .hint_text("e.g. https://staging.example.com"),
                    );
                    ui.label(
                        RichText::new(
                            "Preview lists the GET/POST/PUT/PATCH/DELETE operations in document order; choose which to import. $refs to JSON/YAML files in the document's folder are read; URLs are never downloaded. Path parameters become {{variables}}; required query/header parameters use the document's examples; JSON bodies use examples, else type placeholders to review. Bearer/OAuth and API keys become secret variable references.",
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
                if before != (dialog.curl.clone(), dialog.path.clone(), dialog.api_only) {
                    dialog.preview = None;
                    dialog.picker = None;
                }
                if base_before != dialog.base_url
                    && let Some(picker) = &mut dialog.picker
                {
                    // The parsed document stays; only the URLs change.
                    picker.select_server(&dialog.base_url);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(&mut dialog.replace, false, "Add to current requests");
                    ui.radio_value(&mut dialog.replace, true, "Replace current requests");
                });
                ui.label(
                    RichText::new(
                        "Only requests are imported, never responses. Authorization, Cookie and other credential values stay in this session and are not saved; a secret that is exactly one {{variable}} reference is saved as that reference. Bodies are session-only unless you enable Save body.",
                    )
                    .small()
                    .color(p.muted),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Preview").clicked() {
                        if dialog.source == Source::OpenApi {
                            match dialog.load_openapi() {
                                Ok(spec) => {
                                    dialog.picker = Some(Picker::new(spec, &dialog.base_url));
                                    dialog.preview = None;
                                }
                                Err(e) => {
                                    dialog.picker = None;
                                    dialog.preview = Some(Err(e));
                                }
                            }
                        } else {
                            dialog.preview = Some(dialog.parse(first_id));
                        }
                    }
                    let ready = match &dialog.picker {
                        Some(picker) if dialog.source == Source::OpenApi => {
                            !picker.selected.is_empty()
                                && picker.selected.len() <= room
                                && picker.base.is_ok()
                        }
                        _ => matches!(&dialog.preview, Some(Ok(i)) if !i.requests.is_empty()),
                    };
                    if ui.add_enabled(ready, egui::Button::new("Import")).clicked() {
                        apply = true;
                    }
                });
                if let Some(picker) = &mut dialog.picker {
                    let kept = if dialog.replace { 0 } else { existing };
                    server_ui(ui, picker, p, &dialog.base_url);
                    picker_ui(ui, picker, p, kept, first_id);
                }
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
            let replace = dialog.replace;
            let imported = match &dialog.picker {
                // Build from the parsed document so IDs follow the plan as it is now.
                Some(picker) if dialog.source == Source::OpenApi => {
                    Ok(picker.spec.build(&picker.selected, first_id))
                }
                // Parse again so IDs follow the plan as it is now.
                _ => dialog.parse(first_id),
            };
            match imported {
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

/// Server and server-variable choices (only where the document offers more
/// than one value) and the base URL the requests get.
fn server_ui(ui: &mut egui::Ui, picker: &mut Picker, p: Palette, base_override: &str) {
    ui.separator();
    let overridden = !base_override.trim().is_empty();
    let spec = &picker.spec;
    let mut choice = picker.choice.clone();
    let has_choices = spec.servers.len() > 1
        || spec
            .servers
            .get(choice.server)
            .is_some_and(|s| s.variables.iter().any(|v| v.options.len() > 1));
    if has_choices {
        ui.add_enabled_ui(!overridden, |ui| {
            // Only combo boxes (labels inside them), top-aligned: a centered
            // wrapping row steps each one down as the row grows.
            let row = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 0.0), row, |ui| {
                if spec.servers.len() > 1 {
                    let label = |i: usize| {
                        let s = &spec.servers[i];
                        let text = if s.description.is_empty() {
                            s.url.clone()
                        } else {
                            format!("{} · {}", s.description, s.url)
                        };
                        crate::core::clip(&text, 70)
                    };
                    egui::ComboBox::from_id_salt("openapi_server")
                        .selected_text(format!("Server: {}", label(choice.server)))
                        .truncate()
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            for i in 0..spec.servers.len() {
                                if ui.selectable_label(choice.server == i, label(i)).clicked() {
                                    choice = ServerChoice::defaults(spec, i);
                                }
                            }
                        });
                }
                if let Some(server) = spec.servers.get(choice.server) {
                    for (i, var) in server.variables.iter().enumerate() {
                        if var.options.len() < 2 {
                            continue;
                        }
                        let current = choice.values.get(i).cloned().unwrap_or_default();
                        egui::ComboBox::from_id_salt(("openapi_server_var", choice.server, i))
                            .selected_text(format!(
                                "{}: {}",
                                var.name,
                                crate::core::clip(&current, 40)
                            ))
                            .show_ui(ui, |ui| {
                                for option in &var.options {
                                    if ui
                                        .selectable_label(
                                            *option == current,
                                            crate::core::clip(option, 60),
                                        )
                                        .clicked()
                                        && let Some(value) = choice.values.get_mut(i)
                                    {
                                        value.clone_from(option);
                                    }
                                }
                            });
                    }
                }
            });
        });
    }
    if choice != picker.choice {
        picker.choice = choice;
        picker.select_server(base_override);
    }
    match &picker.base {
        Ok(base) => {
            let source = if overridden {
                "Base URL field"
            } else {
                "document"
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Requests go to").color(p.muted));
                ui.label(RichText::new(base).monospace().size(12.0));
                ui.label(RichText::new(format!("({source})")).small().color(p.muted));
            });
        }
        Err(e) => {
            ui.colored_label(p.error, e);
        }
    }
    if picker.spec.referenced_files > 0 {
        ui.label(
            RichText::new(format!(
                "Read {} referenced file(s) from the document's folder",
                picker.spec.referenced_files
            ))
            .small()
            .color(p.muted),
        );
    }
}

/// The OpenAPI operation list: filters, Select all / None for the filtered
/// operations, one checkbox per operation, the selection count against the
/// plan limit and the warnings for the selection.
fn picker_ui(ui: &mut egui::Ui, picker: &mut Picker, p: Palette, kept: usize, first_id: u64) {
    // `kept`: requests already in the plan that stay after the import.
    let room = IMPORT_LIMIT.saturating_sub(kept);
    ui.separator();
    let total = picker.spec.operations.len();
    if total == 0 {
        ui.colored_label(
            p.warning,
            "The document has no GET/POST/PUT/PATCH/DELETE operations",
        );
        return;
    }
    // Top-aligned: the text field and the combo boxes differ in height.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut picker.filter)
                .hint_text("Filter by path, name or operationId")
                .desired_width(240.0),
        );
        egui::ComboBox::from_id_salt("openapi_method")
            .selected_text(picker.method.unwrap_or("All methods"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut picker.method, None, "All methods");
                for m in METHODS {
                    ui.selectable_value(&mut picker.method, Some(m), m);
                }
            });
        if !picker.tags.is_empty() {
            egui::ComboBox::from_id_salt("openapi_tag")
                .selected_text(picker.tag.as_deref().unwrap_or("All tags"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut picker.tag, None, "All tags");
                    for t in &picker.tags {
                        ui.selectable_value(&mut picker.tag, Some(t.clone()), t);
                    }
                });
        }
    });
    let visible = picker.visible();
    let Picker { spec, selected, .. } = picker;
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(format!("Select all ({})", visible.len()))
            .on_hover_text("Select every operation the filters show")
            .clicked()
        {
            for &i in &visible {
                changed |= selected.insert(spec.operations[i].key.clone());
            }
        }
        if ui
            .button("None")
            .on_hover_text("Clear the operations the filters show")
            .clicked()
        {
            for &i in &visible {
                changed |= selected.remove(&spec.operations[i].key);
            }
        }
        let shown = if visible.len() < total {
            format!(" · {} shown", visible.len())
        } else {
            String::new()
        };
        ui.strong(format!(
            "{} of {total} operations selected{shown}",
            selected.len()
        ));
    });
    // Height left on screen below this point for the list and the warnings, so
    // the dialog (with the server row above) still fits a short window.
    let left = ui.ctx().content_rect().bottom() - ui.cursor().top() - 48.0;
    let budget = left.clamp(90.0, 400.0);
    if visible.is_empty() {
        ui.label(RichText::new("No operations match the filters").color(p.muted));
    } else {
        let row_height = ui.spacing().interact_size.y;
        egui::ScrollArea::vertical()
            .id_salt("openapi_operations")
            .max_height(budget * 0.65)
            .auto_shrink([false, true])
            .show_rows(ui, row_height, visible.len(), |ui, range| {
                for &i in &visible[range] {
                    let op = &spec.operations[i];
                    let mut on = selected.contains(&op.key);
                    ui.horizontal(|ui| {
                        let mut toggled = ui.checkbox(&mut on, "").changed();
                        ui.add_sized(
                            [58.0, row_height],
                            egui::Label::new(
                                RichText::new(&op.method)
                                    .monospace()
                                    .size(12.0)
                                    .color(p.accent),
                            ),
                        );
                        let mut job = egui::text::LayoutJob::default();
                        let font = |size| egui::FontId::monospace(size);
                        let format = |color, size| egui::TextFormat {
                            font_id: font(size),
                            color,
                            ..Default::default()
                        };
                        job.append(&op.path, 0.0, format(p.text, 12.0));
                        if op.name != op.path {
                            job.append(&op.name, 12.0, format(p.muted, 12.0));
                        }
                        if !op.tags.is_empty() {
                            job.append(&op.tags.join(", "), 12.0, format(p.info, 11.0));
                        }
                        if ui
                            .add(egui::Label::new(job).truncate().sense(egui::Sense::click()))
                            .clicked()
                        {
                            on = !on;
                            toggled = true;
                        }
                        if toggled {
                            changed = true;
                            if on {
                                selected.insert(op.key.clone());
                            } else {
                                selected.remove(&op.key);
                            }
                        }
                    });
                }
            });
    }
    if selected.len() > room {
        let held = if kept == 0 {
            String::new()
        } else {
            format!(" ({kept} already in the plan)")
        };
        ui.colored_label(
            p.error,
            format!(
                "{} operations selected, but a plan holds at most {IMPORT_LIMIT} requests{held}. Select at most {room}{}.",
                selected.len(),
                if kept == 0 { "" } else { " or replace current requests" }
            ),
        );
    }
    if changed {
        picker.built = None;
    }
    if picker.built.is_none() && !picker.selected.is_empty() {
        picker.built = Some(picker.spec.build(&picker.selected, first_id));
    }
    if let Some(built) = picker
        .built
        .as_ref()
        // Over the limit, the error above says what to do instead.
        .filter(|_| !picker.selected.is_empty() && picker.selected.len() <= room)
        && !built.warnings.is_empty()
    {
        egui::ScrollArea::vertical()
            .id_salt("openapi_warnings")
            .max_height(budget * 0.35)
            .show(ui, |ui| {
                for w in &built.warnings {
                    ui.colored_label(p.warning, format!("• {w}"));
                }
            });
    }
}

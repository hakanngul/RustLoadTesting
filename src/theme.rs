use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub elevated: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub accent: Color32,
    pub hover: Color32,
    pub on_accent: Color32,
    pub selection: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,
    pub p50: Color32,
    pub p95: Color32,
    pub p99: Color32,
}
const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: c(0x131A20),
            surface: c(0x1B2229),
            elevated: c(0x232C35),
            border: c(0x303B46),
            text: c(0xF3F6FA),
            muted: c(0xA5B1BD),
            disabled: c(0x788592),
            accent: c(0xFF873E),
            hover: c(0xFFA15F),
            on_accent: c(0x131A20),
            selection: c(0x3D2C23),
            success: c(0x3DDC91),
            warning: c(0xFFB44A),
            error: c(0xFF5E68),
            info: c(0x4EA9FF),
            p50: c(0x4EA9FF),
            p95: c(0xB085FF),
            p99: c(0xFF4F97),
        }
    } else {
        Palette {
            bg: c(0xF5F7FA),
            surface: c(0xFFFFFF),
            elevated: c(0xEAF0F4),
            border: c(0xD7DFE7),
            text: c(0x18222C),
            muted: c(0x52616F),
            disabled: c(0x82919E),
            accent: c(0xBD5419),
            hover: c(0x9F4211),
            on_accent: c(0xFFFFFF),
            selection: c(0xFCE8DC),
            success: c(0x137E50),
            warning: c(0x946000),
            error: c(0xBE3142),
            info: c(0x1668C1),
            p50: c(0x1668C1),
            p95: c(0x7951B4),
            p99: c(0xB92368),
        }
    }
}
pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.surface;
    visuals.extreme_bg_color = p.bg;
    visuals.faint_bg_color = p.elevated;
    visuals.text_edit_bg_color = Some(p.elevated);
    visuals.override_text_color = Some(p.text);
    visuals.selection.bg_fill = p.selection;
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    visuals.window_stroke = Stroke::new(1.0_f32, p.border);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.error;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(6);
        widget.bg_stroke = Stroke::new(1.0_f32, p.border);
        widget.fg_stroke = Stroke::new(1.0_f32, p.text);
        widget.bg_fill = p.elevated;
        widget.weak_bg_fill = p.elevated;
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.hover);
    visuals.widgets.active.bg_fill = p.selection;
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 8.0);
        s.text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles
            .insert(TextStyle::Button, FontId::proportional(13.0));
        s.text_styles
            .insert(TextStyle::Small, FontId::proportional(12.0));
        s.text_styles
            .insert(TextStyle::Heading, FontId::proportional(18.0));
        s.text_styles
            .insert(TextStyle::Monospace, FontId::monospace(13.0));
    });
}
pub fn panel(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(10)
        .inner_margin(12)
}
/// Thin progress meter: an empty track stays empty at 0 (egui's ProgressBar
/// always paints a dot) and the fill never covers a text label.
pub fn meter(ui: &mut egui::Ui, p: Palette, fraction: f32, fill: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, p.elevated);
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let mut filled = rect;
        filled.set_width((rect.width() * fraction).max(2.0));
        painter.rect_filled(filled, 4.0, fill);
    }
}

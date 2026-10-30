//! User-owned dashboard layouts. Contains no request data, secrets or execution state.
use serde::{Deserialize, Serialize};

pub const MAX_PAGES: usize = 20;
pub const MAX_WIDGETS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetKind {
    Throughput,
    Latency,
    Logs,
    ActiveUsers,
    StatusCodes,
    Endpoints,
    LiveMetrics,
    LatencyHistogram,
    #[serde(other)]
    Unknown,
}
impl WidgetKind {
    pub const ALL: [Self; 8] = [
        Self::Throughput,
        Self::Latency,
        Self::Logs,
        Self::ActiveUsers,
        Self::StatusCodes,
        Self::Endpoints,
        Self::LiveMetrics,
        Self::LatencyHistogram,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Throughput => "Throughput",
            Self::Latency => "Latency",
            Self::Logs => "Recent Events / Logs",
            Self::ActiveUsers => "Active Users",
            Self::StatusCodes => "HTTP Status Distribution",
            Self::Endpoints => "Endpoint Breakdown",
            Self::LiveMetrics => "Live Metrics",
            Self::LatencyHistogram => "Latency Distribution",
            Self::Unknown => "Unavailable widget",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dock {
    Bottom,
    Right,
}
impl Dock {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bottom => "Bottom",
            Self::Right => "Right",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Span {
    #[default]
    Small,
    Medium,
    Full,
}
impl Span {
    pub fn units(self) -> u32 {
        match self {
            Self::Small => 4,
            Self::Medium => 6,
            Self::Full => 12,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Full => "Full row",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    pub id: u64,
    pub kind: WidgetKind,
    pub dock: Dock,
    pub visible: bool,
    #[serde(default)]
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DashboardPage {
    pub id: u64,
    pub name: String,
    pub widgets: Vec<Widget>,
    pub bottom_height: f32,
    pub right_width: f32,
    pub next_widget_id: u64,
    pub dashboard_only: bool,
    pub card_height: f32,
}
impl Default for DashboardPage {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Studio 2".into(),
            bottom_height: 255.0,
            right_width: 315.0,
            next_widget_id: 5,
            dashboard_only: false,
            card_height: 260.0,
            widgets: vec![
                Widget {
                    id: 1,
                    kind: WidgetKind::Throughput,
                    dock: Dock::Bottom,
                    visible: true,
                    span: Span::Small,
                },
                Widget {
                    id: 2,
                    kind: WidgetKind::Latency,
                    dock: Dock::Bottom,
                    visible: true,
                    span: Span::Small,
                },
                Widget {
                    id: 3,
                    kind: WidgetKind::Logs,
                    dock: Dock::Bottom,
                    visible: true,
                    span: Span::Small,
                },
                Widget {
                    id: 4,
                    kind: WidgetKind::LiveMetrics,
                    dock: Dock::Right,
                    visible: true,
                    span: Span::Small,
                },
            ],
        }
    }
}
impl DashboardPage {
    pub fn visible(&self, dock: Dock) -> Vec<Widget> {
        self.widgets
            .iter()
            .filter(|w| w.visible && w.dock == dock)
            .cloned()
            .collect()
    }
    pub fn add(&mut self, kind: WidgetKind, dock: Dock) -> Result<u64, String> {
        if kind == WidgetKind::Unknown {
            return Err("This widget is unavailable".into());
        }
        if self.widgets.len() >= MAX_WIDGETS {
            return Err(format!("A layout supports at most {MAX_WIDGETS} widgets"));
        }
        let id = self.next_widget_id.max(
            self.widgets
                .iter()
                .map(|w| w.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("Widget ID exhausted")?,
        );
        let next = id.checked_add(1).ok_or("Widget ID exhausted")?;
        self.widgets.push(Widget {
            id,
            kind,
            dock,
            visible: true,
            span: Span::Small,
        });
        self.next_widget_id = next;
        Ok(id)
    }
    pub fn hide(&mut self, id: u64) {
        if let Some(w) = self.widgets.iter_mut().find(|w| w.id == id) {
            w.visible = false;
        }
    }
    pub fn remove(&mut self, id: u64) {
        self.widgets.retain(|w| w.id != id);
    }
    pub fn place(&mut self, id: u64, dock: Dock, before: Option<u64>) {
        if before == Some(id) {
            return;
        }
        let Some(index) = self.widgets.iter().position(|w| w.id == id) else {
            return;
        };
        if let Some(target) = before
            && !self
                .widgets
                .iter()
                .any(|w| w.id == target && (self.dashboard_only || w.dock == dock))
        {
            return;
        }
        let mut widget = self.widgets.remove(index);
        if !self.dashboard_only {
            widget.dock = dock;
        }
        let at = before
            .and_then(|target| self.widgets.iter().position(|w| w.id == target))
            .unwrap_or(self.widgets.len());
        self.widgets.insert(at, widget);
    }
    pub fn shift(&mut self, id: u64, forward: bool) {
        let Some(widget) = self.widgets.iter().find(|w| w.id == id) else {
            return;
        };
        let dock = widget.dock;
        let positions: Vec<_> = self
            .widgets
            .iter()
            .enumerate()
            .filter(|(_, w)| self.dashboard_only || w.dock == dock)
            .map(|(i, _)| i)
            .collect();
        let Some(at) = positions.iter().position(|i| self.widgets[*i].id == id) else {
            return;
        };
        let next = if forward {
            at.checked_add(1).filter(|i| *i < positions.len())
        } else {
            at.checked_sub(1)
        };
        if let Some(next) = next {
            self.widgets.swap(positions[at], positions[next]);
        }
    }
    pub fn normalize(&mut self) {
        self.name = self
            .name
            .chars()
            .take(60)
            .collect::<String>()
            .trim()
            .to_owned();
        if self.name.is_empty() {
            self.name = "My layout".into();
        }
        self.bottom_height = if self.bottom_height.is_finite() {
            self.bottom_height.clamp(170.0, 450.0)
        } else {
            255.0
        };
        self.right_width = if self.right_width.is_finite() {
            self.right_width.clamp(275.0, 520.0)
        } else {
            315.0
        };
        self.card_height = if self.card_height.is_finite() {
            self.card_height.clamp(180.0, 500.0)
        } else {
            260.0
        };
        self.widgets.retain(|w| w.kind != WidgetKind::Unknown);
        self.widgets.truncate(MAX_WIDGETS);
        let mut ids = std::collections::BTreeSet::new();
        for (index, widget) in self.widgets.iter_mut().enumerate() {
            if widget.id == 0 || !ids.insert(widget.id) {
                let mut replacement = index as u64 + 1;
                while !ids.insert(replacement) {
                    replacement += 1;
                }
                widget.id = replacement;
            }
        }
        self.next_widget_id = self.next_widget_id.max(
            self.widgets
                .iter()
                .map(|w| w.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dashboards {
    pub active_id: u64,
    pub pages: Vec<DashboardPage>,
    pub next_page_id: u64,
}
impl Default for Dashboards {
    fn default() -> Self {
        Self {
            active_id: 1,
            pages: vec![DashboardPage::default()],
            next_page_id: 2,
        }
    }
}
impl Dashboards {
    pub fn active(&self) -> &DashboardPage {
        self.pages
            .iter()
            .find(|p| p.id == self.active_id)
            .unwrap_or(&self.pages[0])
    }
    pub fn active_mut(&mut self) -> &mut DashboardPage {
        let index = self
            .pages
            .iter()
            .position(|p| p.id == self.active_id)
            .unwrap_or(0);
        &mut self.pages[index]
    }
    pub fn create(&mut self, name: &str, copy_active: bool) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err("Layout names must contain 1–60 characters".into());
        }
        if self.pages.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            return Err("Choose a unique layout name".into());
        }
        if self.pages.len() >= MAX_PAGES {
            return Err(format!("At most {MAX_PAGES} layouts are supported"));
        }
        let id = self.next_page_id.max(
            self.pages
                .iter()
                .map(|p| p.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("Layout ID exhausted")?,
        );
        let next = id.checked_add(1).ok_or("Layout ID exhausted")?;
        let mut page = if copy_active {
            self.active().clone()
        } else {
            DashboardPage {
                widgets: vec![],
                dashboard_only: true,
                ..DashboardPage::default()
            }
        };
        page.id = id;
        page.name = name.into();
        self.pages.push(page);
        self.active_id = id;
        self.next_page_id = next;
        Ok(())
    }
    pub fn delete_active(&mut self) -> Result<(), String> {
        if self.pages.len() == 1 {
            return Err("Keep at least one layout; its widgets can all be hidden".into());
        }
        self.pages.retain(|p| p.id != self.active_id);
        self.active_id = self.pages[0].id;
        Ok(())
    }
    pub fn rename_active(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err("Layout names must contain 1–60 characters".into());
        }
        if self
            .pages
            .iter()
            .any(|p| p.id != self.active_id && p.name.eq_ignore_ascii_case(name))
        {
            return Err("Choose a unique layout name".into());
        }
        self.active_mut().name = name.into();
        Ok(())
    }
    pub fn normalize(&mut self) {
        self.pages.truncate(MAX_PAGES);
        if self.pages.is_empty() {
            *self = Self::default();
            return;
        }
        let mut ids = std::collections::BTreeSet::new();
        for (index, page) in self.pages.iter_mut().enumerate() {
            page.normalize();
            if page.id == 0 || !ids.insert(page.id) {
                let mut replacement = index as u64 + 1;
                while !ids.insert(replacement) {
                    replacement += 1;
                }
                page.id = replacement;
            }
        }
        if !self.pages.iter().any(|p| p.id == self.active_id) {
            self.active_id = self.pages[0].id;
        }
        self.next_page_id = self.next_page_id.max(
            self.pages
                .iter()
                .map(|p| p.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
    }
}

/// Twelve-column rows. A partly-filled row expands to use its whole width.
pub fn rows(widgets: &[Widget]) -> Vec<Vec<Widget>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut units = 0;
    for widget in widgets {
        if units + widget.span.units() > 12 && !row.is_empty() {
            rows.push(std::mem::take(&mut row));
            units = 0;
        }
        units += widget.span.units();
        row.push(widget.clone());
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

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
    /// The eight Monitor KPI cards (users, RPS, data/sec, P50/P95/P99, errors, count).
    KpiCards,
    /// Requests per second with Total/Success/Failed lines.
    RequestsPerSecond,
    #[serde(other)]
    Unknown,
}
impl WidgetKind {
    pub const ALL: [Self; 10] = [
        Self::Throughput,
        Self::Latency,
        Self::Logs,
        Self::ActiveUsers,
        Self::StatusCodes,
        Self::Endpoints,
        Self::LiveMetrics,
        Self::LatencyHistogram,
        Self::KpiCards,
        Self::RequestsPerSecond,
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
            Self::KpiCards => "KPI Cards",
            Self::RequestsPerSecond => "Requests per Second",
            Self::Unknown => "Unavailable widget",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dock {
    Bottom,
    Right,
    /// The grid below a workspace's fixed header (Monitor only).
    Main,
}
impl Dock {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bottom => "Bottom",
            Self::Right => "Right",
            Self::Main => "Main area",
        }
    }
}

/// Top-level work areas. Each owns an independent set of dashboard pages.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum Workspace {
    #[default]
    Studio,
    Studio2,
    Monitor,
}
impl Workspace {
    pub const ALL: [Self; 3] = [Self::Studio, Self::Studio2, Self::Monitor];
    pub fn label(self) -> &'static str {
        match self {
            Self::Studio => "Studio",
            Self::Studio2 => "Studio 2",
            Self::Monitor => "Monitor",
        }
    }
    /// Docks a workbench page may use here. Studio keeps Load Configuration on
    /// the right, so its widgets dock only at the bottom.
    pub fn docks(self) -> &'static [Dock] {
        match self {
            Self::Studio => &[Dock::Bottom],
            Self::Studio2 => &[Dock::Bottom, Dock::Right],
            Self::Monitor => &[Dock::Main, Dock::Bottom, Dock::Right],
        }
    }
    /// Only Studio 2 can hide its editors for a dashboard-only page.
    pub fn allows_dashboard_only(self) -> bool {
        self == Self::Studio2
    }
    /// The page that reproduces this workspace's built-in layout.
    pub fn default_page(self) -> DashboardPage {
        let widget = |id, kind, dock, span| Widget {
            id,
            kind,
            dock,
            visible: true,
            span,
        };
        match self {
            Self::Studio2 => DashboardPage::default(),
            Self::Studio => DashboardPage {
                name: "Studio".into(),
                bottom_height: 240.0,
                next_widget_id: 3,
                widgets: vec![
                    widget(1, WidgetKind::RequestsPerSecond, Dock::Bottom, Span::Medium),
                    widget(2, WidgetKind::Latency, Dock::Bottom, Span::Medium),
                ],
                ..DashboardPage::default()
            },
            Self::Monitor => DashboardPage {
                name: "Monitor".into(),
                bottom_height: 180.0,
                card_height: 240.0,
                next_widget_id: 8,
                widgets: vec![
                    widget(1, WidgetKind::KpiCards, Dock::Main, Span::Full),
                    widget(2, WidgetKind::RequestsPerSecond, Dock::Main, Span::Medium),
                    widget(3, WidgetKind::Latency, Dock::Main, Span::Medium),
                    widget(4, WidgetKind::StatusCodes, Dock::Main, Span::Medium),
                    widget(5, WidgetKind::ActiveUsers, Dock::Main, Span::Medium),
                    widget(6, WidgetKind::Endpoints, Dock::Main, Span::Full),
                    widget(7, WidgetKind::Logs, Dock::Bottom, Span::Full),
                ],
                ..DashboardPage::default()
            },
        }
    }
    pub fn default_dashboards(self) -> Dashboards {
        Dashboards {
            active_id: 1,
            pages: vec![self.default_page()],
            next_page_id: 2,
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
    /// Replace the widgets with `defaults`, giving each a fresh ID so plot
    /// state of removed widgets never attaches to the restored ones.
    pub fn reset_widgets(&mut self, defaults: Vec<Widget>) {
        self.widgets.clear();
        for default in defaults {
            if let Ok(id) = self.add(default.kind, default.dock)
                && let Some(w) = self.widgets.iter_mut().find(|w| w.id == id)
            {
                w.visible = default.visible;
                w.span = default.span;
            }
        }
    }
    /// Keep the page within what `workspace` can show.
    pub fn fit(&mut self, workspace: Workspace) {
        if !workspace.allows_dashboard_only() {
            self.dashboard_only = false;
        }
        let docks = workspace.docks();
        for widget in &mut self.widgets {
            if !docks.contains(&widget.dock) {
                widget.dock = docks[0];
            }
        }
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
        self.create_for(Workspace::Studio2, name, copy_active)
    }
    /// Add a blank or duplicated page; blank pages follow `workspace`'s rules.
    pub fn create_for(
        &mut self,
        workspace: Workspace,
        name: &str,
        copy_active: bool,
    ) -> Result<(), String> {
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
                dashboard_only: workspace.allows_dashboard_only(),
                ..workspace.default_page()
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
    /// Studio 2 rules; see [`Dashboards::normalize_for`].
    pub fn normalize(&mut self) {
        self.normalize_for(Workspace::Studio2);
    }
    /// Repair a restored page set for `workspace` without resetting valid pages.
    pub fn normalize_for(&mut self, workspace: Workspace) {
        self.pages.truncate(MAX_PAGES);
        if self.pages.is_empty() {
            *self = workspace.default_dashboards();
            return;
        }
        let mut ids = std::collections::BTreeSet::new();
        for (index, page) in self.pages.iter_mut().enumerate() {
            page.normalize();
            page.fit(workspace);
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

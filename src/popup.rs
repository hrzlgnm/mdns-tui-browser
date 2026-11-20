// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::daemon_status::{DaemonSnapshot, IpVersion, RunningSnapshot};
use crate::scroll::ScrollState;
use crate::terminal::{colors_disabled, with_monochrome_selection};

fn calculate_wrapped_line_count(lines: &[Line], width: u16) -> usize {
    if width == 0 || lines.is_empty() {
        return 0;
    }
    let width = width as usize;
    lines
        .iter()
        .map(|line| {
            let line_width = line.width();
            if line_width == 0 {
                1
            } else {
                line_width.div_ceil(width)
            }
        })
        .sum()
}

#[derive(Debug, Clone)]
pub struct HelpPopup {
    pub active: bool,
    pub scroll: ScrollState,
    should_open_release_notes: bool,
}

impl HelpPopup {
    pub fn new() -> Self {
        Self {
            active: false,
            scroll: ScrollState::new(),
            should_open_release_notes: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Closes the popup and scrolls it back to the top.
    pub fn hide(&mut self) {
        self.active = false;
        self.scroll.reset();
    }

    pub fn handle_key_event(&mut self, key: KeyEvent, terminal_area: Rect) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                let popup_area = create_centered_popup(terminal_area, 60, 70);
                let inner_area = Rect::new(
                    popup_area.x + 1,
                    popup_area.y + 1,
                    popup_area.width.saturating_sub(2),
                    popup_area.height.saturating_sub(2),
                );
                let max_visible_lines = inner_area.height as usize;

                self.scroll.visible_items = max_visible_lines;

                let help_content = generate_help_content();
                let total_wrapped_lines =
                    calculate_wrapped_line_count(&help_content, inner_area.width) + 1;

                handle_popup_scroll(
                    key.code,
                    &mut self.scroll.offset,
                    total_wrapped_lines,
                    max_visible_lines,
                );
                true
            }
            KeyCode::Char('r') => {
                self.should_open_release_notes = true;
                true
            }
            _ => {
                self.hide();
                true
            }
        }
    }

    pub fn render(&self, f: &mut Frame) {
        if self.active {
            render_help_popup(f, self.scroll.offset);
        }
    }

    /// Returns and clears the one-shot release-notes action flag.
    pub fn take_release_notes_flag(&mut self) -> bool {
        std::mem::take(&mut self.should_open_release_notes)
    }
}

impl Default for HelpPopup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsPopup {
    pub active: bool,
    pub scroll: ScrollState,
}

impl MetricsPopup {
    pub fn new() -> Self {
        Self {
            active: false,
            scroll: ScrollState::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    pub fn handle_key_event(
        &mut self,
        key: KeyEvent,
        terminal_area: Rect,
        metrics: &BTreeMap<String, u64>,
        daemon: Option<&DaemonSnapshot>,
    ) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                let (_, inner_area) = metrics_popup_areas(terminal_area);
                let max_visible_lines = inner_area.height as usize;

                self.scroll.visible_items = max_visible_lines;

                let metrics_content = generate_metrics_content(metrics, daemon);
                let total_wrapped_lines = metrics_content_height(&metrics_content, inner_area);

                self.scroll.offset = self
                    .scroll
                    .offset
                    .min(max_scroll_offset(total_wrapped_lines, max_visible_lines));
                handle_popup_scroll(
                    key.code,
                    &mut self.scroll.offset,
                    total_wrapped_lines,
                    max_visible_lines,
                );
                true
            }
            _ => {
                self.active = false;
                self.scroll.reset();
                true
            }
        }
    }

    pub fn render(
        &self,
        f: &mut Frame,
        terminal_area: Rect,
        metrics: &BTreeMap<String, u64>,
        daemon: Option<&DaemonSnapshot>,
    ) {
        if self.active {
            render_metrics_popup(f, terminal_area, self.scroll.offset, metrics, daemon);
        }
    }
}

impl Default for MetricsPopup {
    fn default() -> Self {
        Self::new()
    }
}

/// Popup for selecting a URL when a service has multiple URLs.
#[derive(Debug, Clone)]
pub struct UrlSelectionPopup {
    pub active: bool,
    urls: Vec<String>,
    selected_index: usize,
    confirmed_selection: bool,
}

impl UrlSelectionPopup {
    /// Creates a new empty UrlSelectionPopup.
    pub fn new() -> Self {
        Self {
            active: false,
            urls: Vec::new(),
            selected_index: 0,
            confirmed_selection: false,
        }
    }

    /// Returns whether the popup is currently visible.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Shows the popup with the given URLs.
    /// Resets selection to the first item and clears any previous confirmation.
    /// # Example
    /// ```ignore
    /// popup.show(vec!["http://example.com".to_string(), "http://test.com".to_string()]);
    /// ```
    pub fn show(&mut self, urls: Vec<String>) {
        self.urls = urls;
        self.selected_index = 0;
        self.active = true;
        self.confirmed_selection = false;
    }

    /// Hides the popup and clears all state including selection and confirmation.
    pub fn hide(&mut self) {
        self.active = false;
        self.urls.clear();
        self.selected_index = 0;
        self.confirmed_selection = false;
    }

    /// Handles key events for navigation and confirmation.
    /// - Up/Down: navigate selection
    /// - Enter: confirm selection
    /// - Esc or any other key: cancel and close
    pub fn handle_key_event(&mut self, key: KeyEvent, _terminal_area: Rect) -> bool {
        if !self.active {
            return false;
        }

        match key.code {
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                true
            }
            KeyCode::Down => {
                if self.selected_index < self.urls.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
                true
            }
            KeyCode::Enter => {
                self.confirmed_selection = true;
                true
            }
            KeyCode::Esc => {
                self.hide();
                true
            }
            _ => {
                self.hide();
                true
            }
        }
    }

    /// Renders the popup to the frame if active.
    pub fn render(&self, f: &mut Frame, terminal_area: Rect) {
        if !self.active || self.urls.is_empty() {
            return;
        }

        let popup_area = create_centered_popup(terminal_area, 60, 70);

        f.render_widget(ratatui::widgets::Clear, popup_area);

        let background_block = Block::default().style(Style::default().bg(Color::Black));
        f.render_widget(background_block, popup_area);

        let inner_area = Rect::new(
            popup_area.x + 1,
            popup_area.y + 1,
            popup_area.width.saturating_sub(2),
            popup_area.height.saturating_sub(2),
        );

        let items: Vec<ListItem> = self
            .urls
            .iter()
            .map(|url| ListItem::new(url.as_str()))
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected_index));

        let list = List::new(items)
            .style(Style::default().fg(Color::White))
            .highlight_style(with_monochrome_selection(
                Style::default().bg(Color::DarkGray).fg(Color::White),
                colors_disabled(),
            ))
            .highlight_symbol("> ");

        f.render_stateful_widget(list, inner_area, &mut list_state);

        let border_block = Block::default()
            .borders(Borders::ALL)
            .title("Select URL")
            .title_style(Style::default().add_modifier(ratatui::style::Modifier::BOLD));
        f.render_widget(border_block, popup_area);
    }

    /// Returns the selected URL and closes the popup.
    /// Returns `None` if the popup is not active or selection was not confirmed via Enter.
    /// # Example
    /// ```ignore
    /// if let Some(url) = popup.take_selected_url() {
    ///     open::that_detached(&url).ok();
    /// }
    /// ```
    pub fn take_selected_url(&mut self) -> Option<String> {
        if self.active && self.confirmed_selection && self.selected_index < self.urls.len() {
            let url = self.urls.remove(self.selected_index);
            self.hide();
            Some(url)
        } else {
            None
        }
    }
}

impl Default for UrlSelectionPopup {
    /// Creates a default empty UrlSelectionPopup.
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PopupState {
    pub help_popup: HelpPopup,
    pub metrics_popup: MetricsPopup,
    pub url_selection_popup: UrlSelectionPopup,
}

impl PopupState {
    pub fn new() -> Self {
        Self {
            help_popup: HelpPopup::new(),
            metrics_popup: MetricsPopup::new(),
            url_selection_popup: UrlSelectionPopup::new(),
        }
    }

    /// Whether any popup is open and takes key input.
    pub fn any_active(&self) -> bool {
        self.help_popup.is_active()
            || self.metrics_popup.is_active()
            || self.url_selection_popup.is_active()
    }

    pub fn toggle_help(&mut self) {
        self.help_popup.toggle();
    }

    pub fn toggle_metrics(&mut self) {
        self.metrics_popup.toggle();
    }

    /// Closes the other popups and opens the metrics popup, keeping its
    /// scroll position when it is already open.
    pub fn show_only_metrics(&mut self) {
        self.help_popup.hide();
        self.url_selection_popup.hide();
        self.metrics_popup.active = true;
    }

    /// Shows the URL selection popup with the given URLs.
    /// Calls [`UrlSelectionPopup::show`] internally.
    pub fn show_url_selection(&mut self, urls: Vec<String>) {
        self.url_selection_popup.show(urls);
    }

    pub fn handle_key_event(
        &mut self,
        key: KeyEvent,
        terminal_area: Rect,
        metrics: &BTreeMap<String, u64>,
        daemon: Option<&DaemonSnapshot>,
    ) -> bool {
        if self.url_selection_popup.is_active() {
            self.url_selection_popup
                .handle_key_event(key, terminal_area)
        } else if self.help_popup.is_active() {
            self.help_popup.handle_key_event(key, terminal_area)
        } else if self.metrics_popup.is_active() {
            self.metrics_popup
                .handle_key_event(key, terminal_area, metrics, daemon)
        } else {
            false
        }
    }

    pub fn render(
        &self,
        f: &mut Frame,
        terminal_area: Rect,
        metrics: &BTreeMap<String, u64>,
        daemon: Option<&DaemonSnapshot>,
    ) {
        self.help_popup.render(f);
        if !self.help_popup.is_active() {
            self.metrics_popup.render(f, terminal_area, metrics, daemon);
        }
        self.url_selection_popup.render(f, terminal_area);
    }
}

impl Default for PopupState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn handle_popup_scroll(
    key_code: KeyCode,
    scroll_offset: &mut usize,
    total_content_lines: usize,
    max_visible_lines: usize,
) -> bool {
    match key_code {
        KeyCode::Up => {
            if *scroll_offset > 0 {
                *scroll_offset -= 1;
            }
            true
        }
        KeyCode::Down => {
            *scroll_offset = std::cmp::min(
                *scroll_offset + 1,
                max_scroll_offset(total_content_lines, max_visible_lines),
            );
            true
        }
        _ => false,
    }
}

pub fn render_help_popup(f: &mut Frame, help_scroll_offset: usize) {
    let version = env!("CARGO_PKG_VERSION");
    let help_content = generate_help_content();

    let popup_area = create_centered_popup(f.area(), 60, 70);

    f.render_widget(ratatui::widgets::Clear, popup_area);

    let background_block = ratatui::widgets::Block::default()
        .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));
    f.render_widget(background_block, popup_area);

    let inner_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let help_paragraph = Paragraph::new(help_content)
        .style(ratatui::style::Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((help_scroll_offset as u16, 0));

    f.render_widget(help_paragraph, inner_area);

    let border_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Key Bindings | v{}", version))
        .title_style(ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD));
    f.render_widget(border_block, popup_area);
}

/// The metrics popup's outer area and the content area inside its border.
fn metrics_popup_areas(terminal_area: Rect) -> (Rect, Rect) {
    let popup_area = create_centered_popup(terminal_area, 60, 70);
    let inner_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );
    (popup_area, inner_area)
}

/// Height of the wrapped metrics content, plus one line of bottom padding.
fn metrics_content_height(content: &[Line], inner_area: Rect) -> usize {
    calculate_wrapped_line_count(content, inner_area.width) + 1
}

/// Limits `offset` to the last position that still shows content.
///
/// The content shrinks between polls when the daemon drops an interface,
/// and a smaller terminal wraps it differently, so an offset that was
/// valid when scrolled can point past the end and blank the popup.
fn clamp_metrics_scroll(offset: usize, content: &[Line], inner_area: Rect) -> usize {
    offset.min(max_scroll_offset(
        metrics_content_height(content, inner_area),
        inner_area.height as usize,
    ))
}

/// The last scroll offset at which `total_lines` still fill `visible_lines`.
fn max_scroll_offset(total_lines: usize, visible_lines: usize) -> usize {
    total_lines.saturating_sub(visible_lines)
}

pub fn render_metrics_popup(
    f: &mut Frame,
    terminal_area: Rect,
    metrics_scroll_offset: usize,
    metrics: &BTreeMap<String, u64>,
    daemon: Option<&DaemonSnapshot>,
) {
    let metrics_content = generate_metrics_content(metrics, daemon);

    let (popup_area, inner_area) = metrics_popup_areas(terminal_area);

    f.render_widget(ratatui::widgets::Clear, popup_area);

    let background_block = ratatui::widgets::Block::default()
        .style(ratatui::style::Style::default().bg(ratatui::style::Color::Black));
    f.render_widget(background_block, popup_area);

    let metrics_scroll_offset =
        clamp_metrics_scroll(metrics_scroll_offset, &metrics_content, inner_area);

    let metrics_paragraph = Paragraph::new(metrics_content)
        .style(ratatui::style::Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((metrics_scroll_offset as u16, 0));

    f.render_widget(metrics_paragraph, inner_area);

    let border_block = Block::default()
        .borders(Borders::ALL)
        .title("Daemon Status & Metrics")
        .title_style(ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD));
    f.render_widget(border_block, popup_area);
}

pub fn generate_help_content() -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""),
        Line::from(" Help Popup Controls:"),
        Line::from("   ↑/↓               - Scroll this help content"),
        Line::from("   r                 - Open release notes in browser"),
        Line::from("   Any other key     - Close this help popup"),
        Line::from(" "),
        Line::from(" Navigation:"),
        Line::from("   ←/→ or h/l        - Switch between service types"),
        Line::from("   H/L               - Page through service types"),
        Line::from("   PageUp/Down       - Scroll services list by page"),
        Line::from("   b/f/Space         - Scroll services list by page"),
        Line::from("   Home/End          - Jump to first/last service"),
        Line::from("   Ctrl+Home/End     - Jump to first/last service type"),
        Line::from(" "),
        Line::from(" Service Details:"),
        Line::from("   Shift+↑/↓ or J/K  - Scroll service details"),
        Line::from(" "),
        Line::from(" Actions:"),
        Line::from(
            "   Enter             - Open service URL in browser (_http._tcp or URL from TXT)",
        ),
        Line::from("   v                 - Verify selected service (5s timeout)"),
        Line::from("   d                 - Remove offline services"),
        Line::from("   D                 - Clear stale service types"),
        Line::from("   m                 - Show daemon status and metrics"),
        Line::from("   /                 - Enter quick filter mode"),
        Line::from("   n                 - Clear current filter"),
        Line::from("   a                 - Enter add new service type mode"),
        Line::from("   ?                 - Toggle this help popup"),
        Line::from("   Ctrl+j            - Dump state to json file"),
        Line::from("   q or Ctrl+c       - Quit the application"),
    ];

    #[cfg(unix)]
    lines.push(Line::from("   Ctrl+Z            - Suspend the application"));

    lines.extend([
        Line::from(" "),
        Line::from(" Sorting:"),
        Line::from(
            "   s                 - Cycle sort field: Host → Type → Name → Port → Addr → Time",
        ),
        Line::from("   S                 - Cycle sort field backward"),
        Line::from("   o                 - Toggle sort direction (↑/↓)"),
        Line::from(" "),
        Line::from("   Sort field highlighted in white (underlined), direction in cyan (bold)"),
        Line::from(" "),
        Line::from(" Quick Filter:"),
        Line::from("   Enter             - Apply filter"),
        Line::from("   Esc               - Cancel filter input"),
        Line::from("   Backspace         - Delete last character"),
        Line::from(" "),
        Line::from("   Terms fuzzy-match any field, case-insensitively, in any order"),
        Line::from("   Terms of 4+ characters tolerate one typo, except numbers and IPs"),
        Line::from("   Contiguous matches first, then fuzzy matches, then typo matches"),
        Line::from(" "),
        Line::from("   Status keywords (all given must hold):"),
        Line::from("     'online'          - Only online services"),
        Line::from("     'offline'         - Only offline services"),
        Line::from("     'flapping'        - Only flapping services"),
        Line::from(" "),
        Line::from(" Add Service Type Mode:"),
        Line::from("   Enter             - Add service type and start browsing"),
        Line::from("   Esc               - Cancel input"),
        Line::from("   Backspace         - Delete last character"),
        Line::from(" "),
    ]);
    lines
}

/// Lines describing the daemon's sockets and interfaces, with failures
/// highlighted.
fn generate_daemon_status_content(daemon: &DaemonSnapshot) -> Vec<Line<'static>> {
    let failure_style = Style::default().fg(Color::Yellow);
    let running = match daemon {
        DaemonSnapshot::Running(running) => running,
        DaemonSnapshot::Shutdown => {
            return vec![Line::styled("   Daemon has SHUT DOWN", failure_style)];
        }
    };
    let mut lines = Vec::new();
    for version in IpVersion::ALL {
        lines.push(match running.bind_error(version) {
            None => Line::from(format!("   {} socket: bound", version.label())),
            Some(err) => Line::styled(
                format!("   {} socket: FAILED ({})", version.label(), err),
                failure_style,
            ),
        });
    }
    lines.extend(generate_interface_lines(running, failure_style));
    lines
}

fn generate_interface_lines(running: &RunningSnapshot, failure_style: Style) -> Vec<Line<'static>> {
    if running.interfaces.is_empty() {
        return vec![Line::from("   Interfaces: none enabled")];
    }
    let mut lines = vec![Line::from("   Interfaces:")];
    for intf in &running.interfaces {
        lines.push(Line::from(format!("     {} (#{})", intf.name, intf.index)));
        for version in IpVersion::ALL {
            let Some(membership) = intf.membership(version) else {
                continue;
            };
            let addrs = membership
                .addrs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(match &membership.join_error {
                None => Line::from(format!("       {} {}: joined", version.label(), addrs)),
                Some(err) => Line::styled(
                    format!(
                        "       {} {}: JOIN FAILED ({})",
                        version.label(),
                        addrs,
                        err
                    ),
                    failure_style,
                ),
            });
        }
    }
    lines
}

/// Lines of the metrics popup: controls, the daemon status when known,
/// then application and daemon metrics.
pub fn generate_metrics_content(
    metrics: &BTreeMap<String, u64>,
    daemon: Option<&DaemonSnapshot>,
) -> Vec<Line<'static>> {
    let mut metrics_content: Vec<Line> = vec![
        Line::from(""),
        Line::from(" Metrics Controls:"),
        Line::from("   ↑/↓               - Scroll this metrics content"),
        Line::from("   Any other key     - Close this metrics popup"),
        Line::from(" "),
    ];

    if let Some(daemon) = daemon {
        metrics_content.push(Line::from(" Daemon Status:"));
        metrics_content.extend(generate_daemon_status_content(daemon));
        metrics_content.push(Line::from(" "));
    }

    metrics_content.push(Line::from(" Service Discovery Metrics:"));
    metrics_content.push(Line::from(" "));

    let mut custom_metrics = Vec::new();
    let mut daemon_metrics = Vec::new();

    for (key, value) in metrics.iter() {
        if *value > 0 {
            if key.starts_with("daemon_") {
                let clean_key = key.strip_prefix("daemon_").unwrap().replace('_', " ");
                daemon_metrics.push((clean_key, *value));
            } else {
                let formatted_key = key.replace('_', " ");
                custom_metrics.push((formatted_key, *value));
            }
        }
    }

    custom_metrics.sort_by(|a, b| a.0.cmp(&b.0));
    daemon_metrics.sort_by(|a, b| a.0.cmp(&b.0));

    if !custom_metrics.is_empty() {
        metrics_content.push(Line::from(" Custom Metrics:"));
        for (key, value) in &custom_metrics {
            metrics_content.push(Line::from(format!("   {}: {}", key, value)));
        }
        metrics_content.push(Line::from(" "));
    }

    if !daemon_metrics.is_empty() {
        metrics_content.push(Line::from(" Daemon Metrics (from ServiceDaemon):"));
        for (key, value) in &daemon_metrics {
            metrics_content.push(Line::from(format!("   {}: {}", key, value)));
        }
        metrics_content.push(Line::from(" "));
    }

    if custom_metrics.is_empty() && daemon_metrics.is_empty() {
        metrics_content.push(Line::from("   No metrics collected yet"));
    }

    metrics_content
}

pub fn create_centered_popup(parent_area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let popup_width = (parent_area.width * width_percent) / 100;
    let popup_height = (parent_area.height * height_percent) / 100;

    let margin_x = std::cmp::min(2, parent_area.width.saturating_sub(popup_width) / 2);
    let margin_y = std::cmp::min(1, parent_area.height.saturating_sub(popup_height) / 2);

    let x = parent_area.x + (parent_area.width - popup_width) / 2 + margin_x;
    let y = parent_area.y + (parent_area.height - popup_height) / 2 + margin_y;

    let adjusted_width = popup_width - (margin_x * 2);
    let adjusted_height = popup_height - (margin_y * 2);

    Rect::new(x, y, adjusted_width, adjusted_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::daemon_status::{InterfaceSnapshot, MembershipSnapshot};

    #[test]
    fn test_calculate_wrapped_line_count_empty_slice() {
        let lines: Vec<Line> = vec![];
        let count = calculate_wrapped_line_count(&lines, 80);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_calculate_wrapped_line_count_width_zero() {
        let lines = vec![Line::from("test")];
        let count = calculate_wrapped_line_count(&lines, 0);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_calculate_wrapped_line_count_no_wrapping() {
        let lines = vec![Line::from("short")];
        let count = calculate_wrapped_line_count(&lines, 80);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_calculate_wrapped_line_count_with_wrapping() {
        let lines = vec![Line::from(
            "this is a very long line that will definitely wrap",
        )];
        let count = calculate_wrapped_line_count(&lines, 20);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_calculate_wrapped_line_count_mixed_lines() {
        let lines = vec![
            Line::from(""),
            Line::from("short"),
            Line::from("this is a longer line that will wrap"),
            Line::from("a"),
        ];
        let count = calculate_wrapped_line_count(&lines, 15);
        assert_eq!(count, 6);
    }

    #[test]
    fn test_calculate_wrapped_line_count_exact_width() {
        let lines = vec![Line::from("123456789012345")];
        let count = calculate_wrapped_line_count(&lines, 15);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_help_popup_new() {
        let popup = HelpPopup::new();
        assert!(!popup.is_active());
        assert_eq!(popup.scroll.offset, 0);
    }

    #[test]
    fn test_metrics_popup_new() {
        let popup = MetricsPopup::new();
        assert!(!popup.is_active());
        assert_eq!(popup.scroll.offset, 0);
    }

    #[test]
    fn test_help_popup_toggle() {
        let mut popup = HelpPopup::new();
        assert!(!popup.is_active());

        popup.toggle();
        assert!(popup.is_active());

        popup.toggle();
        assert!(!popup.is_active());
    }

    #[test]
    fn test_metrics_popup_toggle() {
        let mut popup = MetricsPopup::new();
        assert!(!popup.is_active());

        popup.toggle();
        assert!(popup.is_active());

        popup.toggle();
        assert!(!popup.is_active());
    }

    #[test]
    fn test_release_notes_flag_lifecycle() {
        use crossterm::event::KeyEvent;

        let terminal_area = Rect::new(0, 0, 80, 24);
        let mut popup = HelpPopup::new();
        popup.active = true;
        popup.scroll.offset = 5;

        assert!(popup.active);
        assert_eq!(popup.scroll.offset, 5);
        assert!(!popup.take_release_notes_flag());

        let r_key = KeyEvent::from(crossterm::event::KeyCode::Char('r'));
        popup.handle_key_event(r_key, terminal_area);

        assert!(popup.active);
        assert!(popup.take_release_notes_flag());
        assert!(!popup.take_release_notes_flag());
    }

    #[test]
    fn test_popup_state_new() {
        let state = PopupState::new();
        assert!(!state.help_popup.is_active());
        assert!(!state.metrics_popup.is_active());
    }

    #[test]
    fn test_popup_state_toggle_help() {
        let mut state = PopupState::new();
        assert!(!state.help_popup.is_active());

        state.toggle_help();
        assert!(state.help_popup.is_active());

        state.toggle_help();
        assert!(!state.help_popup.is_active());
    }

    #[test]
    fn test_popup_state_toggle_metrics() {
        let mut state = PopupState::new();
        assert!(!state.metrics_popup.is_active());

        state.toggle_metrics();
        assert!(state.metrics_popup.is_active());

        state.toggle_metrics();
        assert!(!state.metrics_popup.is_active());
    }

    #[test]
    fn test_popup_state_is_any_active() {
        let mut state = PopupState::new();
        assert!(!state.help_popup.active && !state.metrics_popup.active);

        state.help_popup.active = true;
        assert!(state.help_popup.active || state.metrics_popup.active);

        state.help_popup.active = false;
        state.metrics_popup.active = true;
        assert!(state.help_popup.active || state.metrics_popup.active);
    }

    #[test]
    fn test_popup_state_clone() {
        let mut state = PopupState::new();
        state.help_popup.active = true;
        state.help_popup.scroll.offset = 5;

        let cloned = state.clone();
        assert!(cloned.help_popup.active);
        assert_eq!(cloned.help_popup.scroll.offset, 5);
    }

    #[test]
    fn test_handle_popup_scroll_up() {
        let mut offset = 5;
        let result = handle_popup_scroll(KeyCode::Up, &mut offset, 20, 10);
        assert!(result);
        assert_eq!(offset, 4);
    }

    #[test]
    fn test_handle_popup_scroll_up_at_boundary() {
        let mut offset = 0;
        let result = handle_popup_scroll(KeyCode::Up, &mut offset, 20, 10);
        assert!(result);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_handle_popup_scroll_down() {
        let mut offset = 5;
        let result = handle_popup_scroll(KeyCode::Down, &mut offset, 20, 10);
        assert!(result);
        assert_eq!(offset, 6);
    }

    #[test]
    fn test_handle_popup_scroll_down_at_max() {
        let mut offset = 10;
        let result = handle_popup_scroll(KeyCode::Down, &mut offset, 20, 10);
        assert!(result);
        assert_eq!(offset, 10);
    }

    #[test]
    fn test_handle_popup_scroll_invalid_key() {
        let mut offset = 5;
        let result = handle_popup_scroll(KeyCode::Char('x'), &mut offset, 20, 10);
        assert!(!result);
        assert_eq!(offset, 5);
    }

    #[test]
    fn test_generate_help_content() {
        let content = generate_help_content();
        assert!(!content.is_empty());
        assert!(
            content
                .iter()
                .any(|line| line.to_string().contains("Help Popup Controls"))
        );
    }

    #[test]
    fn test_generate_help_content_lists_verify_key() {
        let content = generate_help_content();
        assert!(
            content_text(&content)
                .iter()
                .any(|line| line.contains("Verify selected service")),
            "help must document the v key"
        );
    }

    #[test]
    fn test_generate_metrics_content_empty() {
        let metrics: BTreeMap<String, u64> = BTreeMap::new();
        let content = generate_metrics_content(&metrics, None);
        assert!(!content.is_empty());
        assert!(
            content
                .iter()
                .any(|line| line.to_string().contains("No metrics collected yet"))
        );
    }

    #[test]
    fn test_generate_metrics_content_with_data() {
        let mut metrics = BTreeMap::new();
        metrics.insert("test_metric".to_string(), 42);
        metrics.insert("daemon_events_received".to_string(), 100);
        let content = generate_metrics_content(&metrics, None);
        assert!(!content.is_empty());
    }

    fn content_text(content: &[Line]) -> Vec<String> {
        content.iter().map(|line| line.to_string()).collect()
    }

    #[test]
    fn test_generate_metrics_content_without_daemon_omits_status_section() {
        let content = generate_metrics_content(&BTreeMap::new(), None);
        assert!(
            !content_text(&content)
                .iter()
                .any(|line| line.contains("Daemon Status"))
        );
    }

    #[test]
    fn test_generate_metrics_content_with_daemon_lists_sockets_and_interfaces() {
        let daemon = DaemonSnapshot::Running(RunningSnapshot {
            ipv4_bind_error: None,
            ipv6_bind_error: Some("Address family not supported".to_string()),
            interfaces: vec![InterfaceSnapshot {
                name: "wlan0".to_string(),
                index: 3,
                ipv4: Some(MembershipSnapshot {
                    addrs: vec!["10.0.0.7".parse().unwrap()],
                    join_error: Some("No such device".to_string()),
                }),
                ipv6: None,
            }],
        });
        let text = content_text(&generate_metrics_content(&BTreeMap::new(), Some(&daemon)));
        for expected in [
            " Daemon Status:",
            "   IPv4 socket: bound",
            "   IPv6 socket: FAILED (Address family not supported)",
            "   Interfaces:",
            "     wlan0 (#3)",
            "       IPv4 10.0.0.7: JOIN FAILED (No such device)",
        ] {
            assert!(
                text.iter().any(|line| line == expected),
                "missing {expected:?}"
            );
        }
        let status_pos = text.iter().position(|l| l == " Daemon Status:");
        let metrics_pos = text.iter().position(|l| l == " Service Discovery Metrics:");
        assert!(status_pos < metrics_pos);
    }

    fn daemon_with_interfaces(count: u32) -> DaemonSnapshot {
        DaemonSnapshot::Running(RunningSnapshot {
            ipv4_bind_error: None,
            ipv6_bind_error: None,
            interfaces: (0..count)
                .map(|index| InterfaceSnapshot {
                    name: format!("veth{index}"),
                    index,
                    ipv4: Some(MembershipSnapshot {
                        addrs: vec![format!("10.0.{}.1", index % 256).parse().unwrap()],
                        join_error: None,
                    }),
                    ipv6: None,
                })
                .collect(),
        })
    }

    #[test]
    fn test_clamp_metrics_scroll_shrunk_content_limits_offset() {
        let terminal_area = Rect::new(0, 0, 100, 40);
        let (_, inner_area) = metrics_popup_areas(terminal_area);
        let metrics = BTreeMap::new();

        let large = generate_metrics_content(&metrics, Some(&daemon_with_interfaces(100)));
        let scrolled_to_end = clamp_metrics_scroll(usize::MAX, &large, inner_area);
        assert!(scrolled_to_end > 0);
        assert_eq!(
            clamp_metrics_scroll(scrolled_to_end, &large, inner_area),
            scrolled_to_end
        );

        let small = generate_metrics_content(&metrics, Some(&daemon_with_interfaces(1)));
        assert_eq!(clamp_metrics_scroll(scrolled_to_end, &small, inner_area), 0);
    }

    #[test]
    fn test_metrics_popup_up_after_shrink_scrolls_from_last_valid_offset() {
        let terminal_area = Rect::new(0, 0, 100, 40);
        let metrics = BTreeMap::new();
        let large = daemon_with_interfaces(100);
        let mut popup = MetricsPopup::new();
        popup.active = true;
        for _ in 0..500 {
            popup.handle_key_event(
                KeyEvent::from(KeyCode::Down),
                terminal_area,
                &metrics,
                Some(&large),
            );
        }
        assert!(popup.scroll.offset > 0);

        let medium = daemon_with_interfaces(10);
        let (_, inner_area) = metrics_popup_areas(terminal_area);
        let medium_max = clamp_metrics_scroll(
            usize::MAX,
            &generate_metrics_content(&metrics, Some(&medium)),
            inner_area,
        );
        assert!(medium_max > 0 && medium_max < popup.scroll.offset);

        popup.handle_key_event(
            KeyEvent::from(KeyCode::Up),
            terminal_area,
            &metrics,
            Some(&medium),
        );
        assert_eq!(popup.scroll.offset, medium_max - 1);
    }

    #[test]
    fn test_render_metrics_popup_scrolled_past_shrunk_content_stays_visible() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 40)).unwrap();
        let small = daemon_with_interfaces(1);
        terminal
            .draw(|f| {
                render_metrics_popup(f, f.area(), 1_000, &BTreeMap::new(), Some(&small));
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Metrics Controls"));
        assert!(rendered.contains("veth0"));
    }

    #[test]
    fn test_generate_metrics_content_shutdown_daemon_shown() {
        let text = content_text(&generate_metrics_content(
            &BTreeMap::new(),
            Some(&DaemonSnapshot::Shutdown),
        ));
        assert!(text.iter().any(|line| line == "   Daemon has SHUT DOWN"));
    }

    #[test]
    fn test_generate_metrics_content_no_interfaces_says_none_enabled() {
        let daemon = DaemonSnapshot::Running(RunningSnapshot {
            ipv4_bind_error: None,
            ipv6_bind_error: None,
            interfaces: Vec::new(),
        });
        let text = content_text(&generate_metrics_content(&BTreeMap::new(), Some(&daemon)));
        assert!(
            text.iter()
                .any(|line| line == "   Interfaces: none enabled")
        );
    }

    #[test]
    fn test_create_centered_popup() {
        let parent = Rect::new(0, 0, 100, 50);
        let popup = create_centered_popup(parent, 60, 70);

        assert!(popup.width > 0);
        assert!(popup.height > 0);
        assert!(popup.x >= parent.x);
        assert!(popup.y >= parent.y);
    }

    #[test]
    fn test_help_popup_handle_key_scroll() {
        let mut popup = HelpPopup::new();
        popup.active = true;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        );
        let result = popup.handle_key_event(key, terminal_area);
        assert!(result);
        assert!(popup.is_active());
    }

    #[test]
    fn test_help_popup_handle_key_close() {
        let mut popup = HelpPopup::new();
        popup.active = true;
        popup.scroll.offset = 10;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(
            crossterm::event::KeyCode::Char('x'),
            crossterm::event::KeyModifiers::NONE,
        );
        let result = popup.handle_key_event(key, terminal_area);
        assert!(result);
        assert!(!popup.is_active());
        assert_eq!(popup.scroll.offset, 0);
    }

    #[test]
    fn test_metrics_popup_handle_key_scroll() {
        let mut popup = MetricsPopup::new();
        popup.active = true;
        let terminal_area = Rect::new(0, 0, 80, 24);
        let metrics = BTreeMap::new();

        let key = KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        );
        let result = popup.handle_key_event(key, terminal_area, &metrics, None);
        assert!(result);
        assert!(popup.is_active());
    }

    #[test]
    fn test_metrics_popup_handle_key_close() {
        let mut popup = MetricsPopup::new();
        popup.active = true;
        popup.scroll.offset = 10;
        let terminal_area = Rect::new(0, 0, 80, 24);
        let metrics = BTreeMap::new();

        let key = KeyEvent::new(
            crossterm::event::KeyCode::Char('x'),
            crossterm::event::KeyModifiers::NONE,
        );
        let result = popup.handle_key_event(key, terminal_area, &metrics, None);
        assert!(result);
        assert!(!popup.is_active());
        assert_eq!(popup.scroll.offset, 0);
    }

    #[derive(Debug)]
    struct MetricsTestCase {
        name: &'static str,
        setup_metrics: fn(&mut BTreeMap<String, u64>),
        initial_offset: usize,
        key_event: KeyEvent,
        expected_offset: Option<usize>,
        expected_popup_open: bool,
        description: &'static str,
    }

    #[test]
    fn test_metrics_popup_comprehensive() {
        let test_cases = vec![
            MetricsTestCase {
                name: "Scroll up from position 3",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=10 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(2),
                expected_popup_open: true,
                description: "Scroll up should decrease offset",
            },
            MetricsTestCase {
                name: "Scroll up from boundary",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=10 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: true,
                description: "Scroll up at top should stay at 0",
            },
            MetricsTestCase {
                name: "Scroll down from position 0",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=10 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(1),
                expected_popup_open: true,
                description: "Scroll down should increase offset",
            },
            MetricsTestCase {
                name: "PageUp closes popup",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=5 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 5,
                key_event: KeyEvent::new(KeyCode::PageUp, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: false,
                description: "PageUp should close popup and reset offset",
            },
            MetricsTestCase {
                name: "PageDown closes popup",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=5 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::PageDown, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: false,
                description: "PageDown should close popup and reset offset",
            },
            MetricsTestCase {
                name: "Home closes popup",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=5 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: false,
                description: "Home should close popup and reset offset",
            },
            MetricsTestCase {
                name: "End closes popup",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=5 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::End, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: false,
                description: "End should close popup and reset offset",
            },
            MetricsTestCase {
                name: "Escape closes popup",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=5 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: false,
                description: "Escape should close popup and reset offset",
            },
            MetricsTestCase {
                name: "F3 closes popup",
                setup_metrics: |metrics: &mut BTreeMap<String, u64>| {
                    for i in 1..=5 {
                        metrics.insert(format!("test_metric_{}", i), i);
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::F(3), crossterm::event::KeyModifiers::NONE),
                expected_offset: Some(0),
                expected_popup_open: false,
                description: "F3 should close popup and reset offset",
            },
        ];

        for test_case in test_cases {
            let mut metrics = BTreeMap::new();
            (test_case.setup_metrics)(&mut metrics);

            let mut popup = MetricsPopup::new();
            popup.active = true;
            popup.scroll.offset = test_case.initial_offset;

            let terminal_area = Rect::new(0, 0, 80, 24);
            popup.handle_key_event(test_case.key_event, terminal_area, &metrics, None);

            if let Some(expected_offset) = test_case.expected_offset {
                assert_eq!(
                    popup.scroll.offset, expected_offset,
                    "{}: {} - Expected offset {}, got {}",
                    test_case.name, test_case.description, expected_offset, popup.scroll.offset
                );
            }

            assert_eq!(
                popup.active, test_case.expected_popup_open,
                "{}: {} - Expected popup open: {}, got {}",
                test_case.name, test_case.description, test_case.expected_popup_open, popup.active
            );
        }
    }

    #[test]
    fn test_url_selection_popup_new() {
        let popup = UrlSelectionPopup::new();
        assert!(!popup.is_active());
        assert!(popup.urls.is_empty());
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_url_selection_popup_show() {
        let mut popup = UrlSelectionPopup::new();
        let urls = vec![
            "http://example.com".to_string(),
            "http://test.com".to_string(),
        ];

        popup.show(urls.clone());

        assert!(popup.is_active());
        assert_eq!(popup.urls, urls);
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_url_selection_popup_hide() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://example.com".to_string()]);
        assert!(popup.is_active());

        popup.hide();

        assert!(!popup.is_active());
        assert!(popup.urls.is_empty());
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_url_selection_popup_navigation_up() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec![
            "http://a.com".to_string(),
            "http://b.com".to_string(),
            "http://c.com".to_string(),
        ]);
        popup.selected_index = 2;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert_eq!(popup.selected_index, 1);
    }

    #[test]
    fn test_url_selection_popup_navigation_up_at_first() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://a.com".to_string(), "http://b.com".to_string()]);
        popup.selected_index = 0;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_url_selection_popup_navigation_down() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec![
            "http://a.com".to_string(),
            "http://b.com".to_string(),
            "http://c.com".to_string(),
        ]);
        popup.selected_index = 0;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert_eq!(popup.selected_index, 1);
    }

    #[test]
    fn test_url_selection_popup_navigation_down_at_last() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://a.com".to_string(), "http://b.com".to_string()]);
        popup.selected_index = 1;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert_eq!(popup.selected_index, 1);
    }

    #[test]
    fn test_url_selection_popup_enter_key() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://a.com".to_string(), "http://b.com".to_string()]);
        popup.selected_index = 1;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert!(popup.is_active());
    }

    #[test]
    fn test_url_selection_popup_escape_key() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://example.com".to_string()]);
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert!(!popup.is_active());
    }

    #[test]
    fn test_url_selection_popup_other_key_closes() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://example.com".to_string()]);
        popup.selected_index = 5;
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Char('x'), crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(result);
        assert!(!popup.is_active());
    }

    #[test]
    fn test_url_selection_popup_take_selected_url() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec![
            "http://a.com".to_string(),
            "http://b.com".to_string(),
            "http://c.com".to_string(),
        ]);
        popup.selected_index = 1;

        let terminal_area = Rect::new(0, 0, 80, 24);
        let enter_key = KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        popup.handle_key_event(enter_key, terminal_area);

        let url = popup.take_selected_url();

        assert_eq!(url, Some("http://b.com".to_string()));
        assert!(!popup.is_active());
    }

    #[test]
    fn test_url_selection_popup_take_selected_url_without_confirmation() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://a.com".to_string(), "http://b.com".to_string()]);
        popup.selected_index = 1;

        let url = popup.take_selected_url();

        assert!(url.is_none());
        assert!(popup.is_active());
    }

    #[test]
    fn test_url_selection_popup_take_selected_url_when_inactive() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://example.com".to_string()]);
        popup.active = false;

        let url = popup.take_selected_url();

        assert!(url.is_none());
    }

    #[test]
    fn test_url_selection_popup_handle_key_when_inactive() {
        let mut popup = UrlSelectionPopup::new();
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = popup.handle_key_event(key, terminal_area);

        assert!(!result);
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_url_selection_popup_single_url_navigation() {
        let mut popup = UrlSelectionPopup::new();
        popup.show(vec!["http://onlyone.com".to_string()]);
        let terminal_area = Rect::new(0, 0, 80, 24);

        let key_up = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        popup.handle_key_event(key_up, terminal_area);
        assert_eq!(popup.selected_index, 0);

        let key_down = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        popup.handle_key_event(key_down, terminal_area);
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_popup_state_url_selection_show() {
        let mut state = PopupState::new();
        let urls = vec!["http://a.com".to_string(), "http://b.com".to_string()];

        state.show_url_selection(urls.clone());

        assert!(state.url_selection_popup.is_active());
        assert_eq!(state.url_selection_popup.urls, urls);
    }

    #[test]
    fn test_popup_state_url_selection_key_event() {
        let mut state = PopupState::new();
        state.show_url_selection(vec!["http://a.com".to_string(), "http://b.com".to_string()]);
        let terminal_area = Rect::new(0, 0, 80, 24);
        let metrics = BTreeMap::new();

        let key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key, terminal_area, &metrics, None);

        assert!(result);
        assert_eq!(state.url_selection_popup.selected_index, 1);
    }

    #[test]
    fn test_popup_state_url_selection_takes_priority() {
        let mut state = PopupState::new();
        state.help_popup.active = true;
        state.show_url_selection(vec!["http://a.com".to_string(), "http://b.com".to_string()]);
        let terminal_area = Rect::new(0, 0, 80, 24);
        let metrics = BTreeMap::new();

        let key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key, terminal_area, &metrics, None);

        assert!(result);
        assert_eq!(state.url_selection_popup.selected_index, 1);
    }
}

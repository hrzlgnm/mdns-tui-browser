// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::scroll::ScrollState;

#[derive(Debug, Clone)]
pub struct HelpPopup {
    pub active: bool,
    pub scroll: ScrollState,
    cached_help_lines: usize,
}

impl HelpPopup {
    pub fn new() -> Self {
        Self {
            active: false,
            scroll: ScrollState::new(),
            cached_help_lines: 0,
        }
    }

    fn ensure_cache_valid(&mut self) {
        if self.cached_help_lines == 0 {
            self.cached_help_lines = generate_help_content().len();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    pub fn handle_key_event(&mut self, key: KeyEvent, terminal_area: Rect) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.ensure_cache_valid();

                let popup_area = create_centered_popup(terminal_area, 60, 70);
                let inner_area = Rect::new(
                    popup_area.x + 1,
                    popup_area.y + 1,
                    popup_area.width.saturating_sub(2),
                    popup_area.height.saturating_sub(2),
                );
                let max_visible_lines = inner_area.height as usize;

                self.scroll.visible_items = max_visible_lines;

                handle_popup_scroll(
                    key.code,
                    &mut self.scroll.offset,
                    self.cached_help_lines,
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

    pub fn render(&self, f: &mut Frame) {
        if self.active {
            render_help_popup(f, self.scroll.offset);
        }
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
    cached_metrics_lines: usize,
}

impl MetricsPopup {
    pub fn new() -> Self {
        Self {
            active: false,
            scroll: ScrollState::new(),
            cached_metrics_lines: 0,
        }
    }

    fn ensure_cache_valid(&mut self, metrics: &BTreeMap<String, u64>) {
        if self.cached_metrics_lines == 0 {
            self.cached_metrics_lines = generate_metrics_content(metrics).len();
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
    ) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.ensure_cache_valid(metrics);

                let popup_area = create_centered_popup(terminal_area, 60, 70);
                let inner_area = Rect::new(
                    popup_area.x + 1,
                    popup_area.y + 1,
                    popup_area.width.saturating_sub(2),
                    popup_area.height.saturating_sub(2),
                );
                let max_visible_lines = inner_area.height as usize;

                self.scroll.visible_items = max_visible_lines;

                handle_popup_scroll(
                    key.code,
                    &mut self.scroll.offset,
                    self.cached_metrics_lines,
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

    pub fn render(&self, f: &mut Frame, terminal_area: Rect, metrics: &BTreeMap<String, u64>) {
        if self.active {
            render_metrics_popup(f, terminal_area, self.scroll.offset, metrics);
        }
    }
}

impl Default for MetricsPopup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PopupState {
    pub help_popup: HelpPopup,
    pub metrics_popup: MetricsPopup,
}

impl PopupState {
    pub fn new() -> Self {
        Self {
            help_popup: HelpPopup::new(),
            metrics_popup: MetricsPopup::new(),
        }
    }

    pub fn toggle_help(&mut self) {
        self.help_popup.toggle();
    }

    pub fn toggle_metrics(&mut self) {
        self.metrics_popup.toggle();
    }

    pub fn handle_key_event(
        &mut self,
        key: KeyEvent,
        terminal_area: Rect,
        metrics: &BTreeMap<String, u64>,
    ) -> bool {
        if self.help_popup.is_active() {
            self.help_popup.handle_key_event(key, terminal_area)
        } else if self.metrics_popup.is_active() {
            self.metrics_popup
                .handle_key_event(key, terminal_area, metrics)
        } else {
            false
        }
    }

    pub fn render(&self, f: &mut Frame, terminal_area: Rect, metrics: &BTreeMap<String, u64>) {
        self.help_popup.render(f);
        if !self.help_popup.is_active() {
            self.metrics_popup.render(f, terminal_area, metrics);
        }
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
            let max_scroll_offset = total_content_lines.saturating_sub(max_visible_lines);
            *scroll_offset = std::cmp::min(*scroll_offset + 1, max_scroll_offset);
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

    let clamped_offset = if help_content.is_empty() {
        0
    } else {
        help_scroll_offset.min(help_content.len().saturating_sub(1))
    };
    let visible_help_content: Vec<Line> = help_content.into_iter().skip(clamped_offset).collect();

    let help_paragraph = Paragraph::new(visible_help_content)
        .style(ratatui::style::Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    f.render_widget(help_paragraph, inner_area);

    let border_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Key Bindings | v{}", version))
        .title_style(ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD));
    f.render_widget(border_block, popup_area);
}

pub fn render_metrics_popup(
    f: &mut Frame,
    terminal_area: Rect,
    metrics_scroll_offset: usize,
    metrics: &BTreeMap<String, u64>,
) {
    let metrics_content = generate_metrics_content(metrics);

    let popup_area = create_centered_popup(terminal_area, 60, 70);

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

    let visible_lines = inner_area.height as usize;
    let max_offset = metrics_content.len().saturating_sub(visible_lines);
    let clamped_offset = metrics_scroll_offset.min(max_offset);
    let visible_metrics_content: Vec<Line> =
        metrics_content.into_iter().skip(clamped_offset).collect();

    let metrics_paragraph = Paragraph::new(visible_metrics_content)
        .style(ratatui::style::Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    f.render_widget(metrics_paragraph, inner_area);

    let border_block = Block::default()
        .borders(Borders::ALL)
        .title("Service Metrics")
        .title_style(ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD));
    f.render_widget(border_block, popup_area);
}

pub fn generate_help_content() -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""),
        Line::from(" Help Controls:"),
        Line::from("   ↑/↓               - Scroll this help content"),
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
        Line::from("   d                 - Remove offline services"),
        Line::from("   D                 - Clear stale service types"),
        Line::from("   m                 - Show service metrics"),
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
        Line::from("   Special keywords:"),
        Line::from(
            "     'online'          - Show online services + services with 'online' in text",
        ),
        Line::from(
            "     'offline'         - Show offline services + services with 'offline' in text",
        ),
        Line::from(" "),
        Line::from("   Filter searches all service fields case-insensitively"),
        Line::from(" "),
        Line::from(" Add Service Type Mode:"),
        Line::from("   Enter             - Add service type and start browsing"),
        Line::from("   Esc               - Cancel input"),
        Line::from("   Backspace         - Delete last character"),
        Line::from(" "),
    ]);
    lines
}

pub fn generate_metrics_content(metrics: &BTreeMap<String, u64>) -> Vec<Line<'static>> {
    let mut metrics_content: Vec<Line> = vec![
        Line::from(""),
        Line::from(" Metrics Controls:"),
        Line::from("   ↑/↓               - Scroll this metrics content"),
        Line::from("   Any other key     - Close this metrics popup"),
        Line::from(" "),
        Line::from(" Service Discovery Metrics:"),
        Line::from(" "),
    ];

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
                .any(|line| line.to_string().contains("Help Controls"))
        );
    }

    #[test]
    fn test_generate_metrics_content_empty() {
        let metrics: BTreeMap<String, u64> = BTreeMap::new();
        let content = generate_metrics_content(&metrics);
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
        let content = generate_metrics_content(&metrics);
        assert!(!content.is_empty());
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
        let result = popup.handle_key_event(key, terminal_area, &metrics);
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
        let result = popup.handle_key_event(key, terminal_area, &metrics);
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
            popup.handle_key_event(test_case.key_event, terminal_area, &metrics);

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
}

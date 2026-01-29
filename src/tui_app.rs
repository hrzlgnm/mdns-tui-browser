#![forbid(unsafe_code)]

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use mdns_sd::{ServiceDaemon, ServiceEvent};
#[cfg(unix)]
use nix::sys::signal::Signal;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
struct ServiceEntry {
    fullname: String,
    host: String,
    service_type: String,
    subtype: Option<String>,
    addrs: Vec<String>,
    port: u16,
    txt: Vec<String>,
    alive: bool,
    timestamp_micros: u64,
}

impl ServiceEntry {
    fn die_at(&mut self, timestamp_micros: u64) {
        self.alive = false;
        self.timestamp_micros = timestamp_micros;
    }
}

struct AppState {
    services: Vec<ServiceEntry>,
    service_types: Vec<String>,
    selected_service: usize,
    selected_type: Option<usize>,
    types_scroll_offset: usize,
    services_scroll_offset: usize,
    visible_types: usize,
    visible_services: usize,
    cached_filtered_services: Vec<usize>,
    cache_dirty: bool,
    show_help_popup: bool,
    last_error: Option<String>,
}

impl AppState {
    fn new() -> Self {
        let mut state = Self {
            services: Vec::new(),
            service_types: Vec::new(),
            selected_service: 0,
            selected_type: None,
            types_scroll_offset: 0,
            services_scroll_offset: 0,
            visible_types: 0,
            visible_services: 0,
            cached_filtered_services: Vec::new(),
            cache_dirty: true,
            show_help_popup: false,
            last_error: None,
        };
        state.validate_selected_type();
        state
    }

    fn filter_service(&self, service: &ServiceEntry) -> bool {
        if self.selected_type.is_none() {
            return true; // "All Types" - show everything
        }

        let idx = self.selected_type.unwrap();
        if let Some(selected_type) = self.service_types.get(idx) {
            service.service_type == *selected_type
        } else {
            false
        }
    }

    fn update_filtered_cache(&mut self) {
        if self.cache_dirty {
            self.cached_filtered_services.clear();
            for (idx, service) in self.services.iter().enumerate() {
                if self.filter_service(service) {
                    self.cached_filtered_services.push(idx);
                }
            }
            self.cache_dirty = false;
        }
    }

    fn mark_cache_dirty(&mut self) {
        self.cache_dirty = true;
    }

    fn validate_selected_type(&mut self) {
        // Ensure selected_type is always valid
        if let Some(idx) = self.selected_type
            && idx >= self.service_types.len()
        {
            if self.service_types.is_empty() {
                self.selected_type = None;
            } else {
                self.selected_type = Some(self.service_types.len().saturating_sub(1));
            }
        }
    }

    fn get_filtered_services(&mut self) -> &[usize] {
        self.update_filtered_cache();
        self.cached_filtered_services.as_slice()
    }

    // Helper methods for service type management
    fn add_service_type(&mut self, service_type: &str) -> bool {
        if !self.service_types.contains(&service_type.to_string()) {
            // Capture currently selected value before mutation
            let selected_value = self
                .selected_type
                .and_then(|idx| self.service_types.get(idx).cloned());

            self.service_types.push(service_type.to_string());
            self.service_types.sort();

            // Re-anchor selection by finding the captured value's new index
            if let Some(selected_value) = selected_value {
                if let Some(new_idx) = self.service_types.iter().position(|s| s == &selected_value)
                {
                    self.selected_type = Some(new_idx);
                } else {
                    // Fallback: if somehow the value is gone, go to None (All Types)
                    self.selected_type = None;
                }
            }

            self.invalidate_cache_and_validate();
            true
        } else {
            false
        }
    }

    fn remove_service_type(&mut self, service_type: &str) -> bool {
        if self.services.iter().any(|s| s.service_type == service_type) {
            return false; // Still in use
        }
        let initial_len = self.service_types.len();

        // Capture currently selected value before mutation
        let selected_value = self
            .selected_type
            .and_then(|idx| self.service_types.get(idx).cloned());

        self.service_types.retain(|s| s != service_type);
        let removed = self.service_types.len() < initial_len;

        if removed {
            // Re-anchor selection by finding the captured value's new index
            if let Some(selected_value) = selected_value {
                if let Some(new_idx) = self.service_types.iter().position(|s| s == &selected_value)
                {
                    self.selected_type = Some(new_idx);
                } else if selected_value == service_type {
                    // The selected item was removed - pick nearest valid index
                    if self.service_types.is_empty() {
                        self.selected_type = None;
                    } else {
                        // Try to use the same index, or clamp to last valid index
                        let fallback_idx = self
                            .selected_type
                            .unwrap_or(0)
                            .min(self.service_types.len().saturating_sub(1));
                        self.selected_type = Some(fallback_idx);
                    }
                } else {
                    // Selected value is gone for some other reason
                    self.selected_type = None;
                }
            }

            self.invalidate_cache_and_validate();
        }

        removed
    }

    fn update_service_type_selection(&mut self, new_type: Option<usize>) {
        self.selected_type = new_type;
        self.selected_service = 0;
        self.services_scroll_offset = 0;
        self.invalidate_cache_and_validate();
    }

    fn remove_dead_services(&mut self) {
        // Collect service types that have dead services
        let mut service_types_to_check: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Capture initial filtered length for scroll logic
        let initial_filtered_len = self.get_filtered_services().len();

        // Remove dead services and track their types
        let initial_len = self.services.len();
        self.services.retain(|service| {
            if !service.alive {
                service_types_to_check.insert(service.service_type.clone());
                false // Remove this service
            } else {
                true // Keep this service
            }
        });

        let removed_count = initial_len - self.services.len();

        if removed_count > 0 {
            // Refresh cache immediately after retain to ensure filtered services are up-to-date
            self.invalidate_cache_and_validate();

            // Check if any service types should be removed (no active services of that type)
            let mut types_to_remove = Vec::new();
            for service_type in service_types_to_check {
                if !self
                    .services
                    .iter()
                    .any(|s| s.service_type == service_type && s.alive)
                {
                    types_to_remove.push(service_type);
                }
            }

            // Remove empty service types
            for service_type in types_to_remove {
                self.remove_service_type(&service_type);
            }

            let new_filtered_len = self.get_filtered_services().len();

            // Adjust selection indices - if user was at the end, keep them at the end
            if new_filtered_len > 0 {
                let was_near_end = initial_filtered_len > 0
                    && (self.selected_service >= initial_filtered_len.saturating_sub(2)
                        || self.selected_service >= new_filtered_len);
                if was_near_end {
                    self.selected_service = new_filtered_len.saturating_sub(1);
                } else {
                    // Otherwise, keep the same position but cap it to the new maximum
                    self.selected_service = self
                        .selected_service
                        .min(new_filtered_len.saturating_sub(1));
                }
            } else {
                self.selected_service = 0;
            }

            // Adjust scroll offset - if we're at the end, position selected item at bottom of view
            if new_filtered_len > 0 && self.selected_service >= new_filtered_len.saturating_sub(2) {
                // Position selected item at or near the bottom of the visible area
                if self.visible_services > 0 {
                    self.services_scroll_offset = self
                        .selected_service
                        .saturating_sub(self.visible_services - 1);
                }
            } else {
                // Otherwise, just ensure it's visible
                self.update_services_scroll_offset();
            }
        }
    }

    fn invalidate_cache_and_validate(&mut self) {
        self.mark_cache_dirty();
        self.validate_selected_type();
    }

    // Key handling methods
    fn handle_key_event(
        &mut self,
        key: KeyEvent,
        notification_sender: Option<flume::Sender<Notification>>,
    ) -> bool {
        if self.show_help_popup {
            self.handle_help_popup_key(key)
        } else {
            self.handle_normal_mode_key(key, notification_sender)
        }
    }

    fn handle_help_popup_key(&mut self, _key: KeyEvent) -> bool {
        // Any key just closes the help popup and returns to normal mode
        self.show_help_popup = false;
        true // Continue running
    }

    fn handle_normal_mode_key(
        &mut self,
        key: KeyEvent,
        _notification_sender: Option<flume::Sender<Notification>>,
    ) -> bool {
        match key.code {
            // Quit actions
            KeyCode::Char('q') => {
                false // Signal to quit
            }
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                false // Signal to quit
            }

            // Suspend action (Unix only)
            #[cfg(unix)]
            KeyCode::Char('z')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                // Suspend the process (resume is handled inline in suspend_process)
                let mut state_for_suspend = state.write().await;
                if let Err(_) = suspend_process(&mut state_for_suspend) {
                    // Error is already stored in state, just trigger redraw to show it
                } else {
                    // Trigger immediate complete redraw to show any error or clear previous error
                    if let Some(sender) = notification_sender {
                        let _ = sender.send(Notification::ForceRedraw);
                    }
                }
                true // Continue running after resume
            }

            // Help toggle
            KeyCode::Char('?') => {
                self.toggle_help();
                true
            }

            // Service navigation
            KeyCode::Char('k') | KeyCode::Up => {
                self.navigate_services_up();
                true
            }

            KeyCode::Char('j') | KeyCode::Down => {
                self.navigate_services_down();
                true
            }

            KeyCode::Char('h') | KeyCode::Left => {
                self.navigate_service_types_up();
                true
            }

            KeyCode::Char('l') | KeyCode::Right => {
                self.navigate_service_types_down();
                true
            }

            // Page navigation
            KeyCode::PageUp | KeyCode::Char('b') => {
                self.navigate_services_page_up();
                true
            }

            KeyCode::PageDown | KeyCode::Char('f') | KeyCode::Char(' ') => {
                self.navigate_services_page_down();
                true
            }

            KeyCode::Home => {
                self.navigate_services_to_first();
                true
            }

            KeyCode::End => {
                self.navigate_services_to_last();
                true
            }

            // Actions
            KeyCode::Char('d') => {
                self.remove_dead_services();
                true
            }
            KeyCode::Char('c') => {
                self.clear_error();
                true
            }

            _ => true,
        }
    }

    fn toggle_help(&mut self) {
        self.show_help_popup = !self.show_help_popup;
    }

    fn navigate_services_up(&mut self) {
        if self.selected_service > 0 {
            self.selected_service -= 1;
            self.update_services_scroll_offset();
        }
    }

    fn navigate_services_down(&mut self) {
        let filtered = self.get_filtered_services();
        let filtered_len = filtered.len();
        if self.selected_service < filtered_len.saturating_sub(1) {
            self.selected_service += 1;
            self.update_services_scroll_offset();
        }
    }

    fn navigate_service_types_up(&mut self) {
        let new_type = match self.selected_type {
            None => None,               // Already at "All Types", can't go further left
            Some(0) => None,            // Move from first service type to "All Types"
            Some(idx) => Some(idx - 1), // Move to previous service type
        };

        if new_type.is_none() {
            // Moving to "All Types" - ensure it's visible at visual index 0
            self.types_scroll_offset = 0;
        } else if let Some(new_idx) = new_type {
            // Update scroll offset for types list using actual visible count
            if new_idx < self.types_scroll_offset {
                self.types_scroll_offset = new_idx;
            }
        }
        self.update_service_type_selection(new_type);
    }

    fn navigate_service_types_down(&mut self) {
        let new_type = match self.selected_type {
            None => {
                // Move from "All Types" to first service type (index 0)
                if !self.service_types.is_empty() {
                    Some(0)
                } else {
                    None
                }
            }
            Some(idx) if idx < self.service_types.len().saturating_sub(1) => Some(idx + 1),
            Some(idx) => Some(idx), // Stay at last service type, don't wrap to "All Types"
        };

        if new_type.is_none() {
            // Moving to "All Types" - ensure it's visible at visual index 0
            self.types_scroll_offset = 0;
        } else if let Some(new_idx) = new_type {
            // Update scroll offset for types list using actual visible count
            if self.visible_types > 0 && new_idx >= self.types_scroll_offset + self.visible_types {
                self.types_scroll_offset = new_idx - self.visible_types + 1;
            }
        }
        self.update_service_type_selection(new_type);
    }

    fn navigate_services_page_up(&mut self) {
        let scroll_amount = self.visible_services.saturating_sub(1);
        if self.selected_service >= scroll_amount {
            self.selected_service -= scroll_amount;
        } else {
            self.selected_service = 0;
        }
        self.update_services_scroll_offset();
    }

    fn navigate_services_page_down(&mut self) {
        let filtered = self.get_filtered_services();
        let filtered_len = filtered.len();
        let scroll_amount = self.visible_services.saturating_sub(1);
        if self.selected_service + scroll_amount < filtered_len.saturating_sub(1) {
            self.selected_service += scroll_amount;
        } else {
            self.selected_service = filtered_len.saturating_sub(1);
        }
        self.update_services_scroll_offset();
    }

    fn navigate_services_to_first(&mut self) {
        self.selected_service = 0;
        self.services_scroll_offset = 0;
    }

    fn navigate_services_to_last(&mut self) {
        let filtered = self.get_filtered_services();
        let filtered_len = filtered.len();
        self.selected_service = filtered_len.saturating_sub(1);
        self.update_services_scroll_offset();
    }

    fn update_services_scroll_offset(&mut self) {
        if self.selected_service < self.services_scroll_offset {
            self.services_scroll_offset = self.selected_service;
        } else if self.visible_services > 0
            && self.selected_service >= self.services_scroll_offset + self.visible_services
        {
            self.services_scroll_offset = self.selected_service - self.visible_services + 1;
        }
    }

    fn clear_error(&mut self) {
        self.last_error = None;
    }
}

#[derive(Debug, Clone)]
enum Notification {
    UserInput,
    ServiceChanged,
    ForceRedraw,
}

fn is_valid_service_type(service_type: &str) -> bool {
    // Just ignore subtypes in enumeration, other
    // invalid types are covered by browse resulting in an error
    !service_type.contains("_sub.")
}

fn current_timestamp_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[cfg(unix)]
fn suspend_process(state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{Write, stdout};

    // Disable raw mode and restore terminal before suspending
    if let Err(e) = disable_raw_mode() {
        state.last_error = Some(format!("Failed to disable raw mode: {}", e));
        return Err(e.into());
    }

    if let Err(e) = execute!(stdout(), LeaveAlternateScreen) {
        state.last_error = Some(format!("Failed to leave alternate screen: {}", e));
        return Err(e.into());
    }

    if let Err(e) = stdout().flush() {
        state.last_error = Some(format!("Failed to flush stdout: {}", e));
        return Err(e.into());
    }

    // Send SIGTSTP to the current process using nix
    if let Err(e) = nix::sys::signal::kill(nix::unistd::getpid(), Signal::SIGTSTP) {
        state.last_error = Some(format!("Failed to send suspend signal: {}", e));
        return Err(e.into());
    }

    // This code won't execute until after resume
    if let Err(e) = resume_after_suspend() {
        state.last_error = Some(format!("Failed to resume after suspend: {}", e));
        return Err(e.into());
    }

    // Clear error on successful resume
    state.last_error = None;

    Ok(())
}

#[cfg(unix)]
fn resume_after_suspend() -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::cursor;
    use crossterm::execute;
    use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode};
    use std::io::{Write, stdout};

    // Complete terminal reset sequence
    execute!(
        stdout(),
        LeaveAlternateScreen,
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    stdout().flush()?;

    // Disable raw mode completely
    disable_raw_mode()?;

    // Restart TUI mode fresh
    execute!(
        stdout(),
        EnterAlternateScreen,
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    stdout().flush()?;

    // Re-enable raw mode
    enable_raw_mode()?;

    Ok(())
}

#[cfg(unix)]
fn recreate_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::cursor;
    use crossterm::terminal::{Clear, ClearType};
    use ratatui::layout::Rect;

    // Completely flush and clear terminal
    terminal.clear()?;
    execute!(
        terminal.backend_mut(),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    terminal.flush()?;

    // Force a resize to trigger complete redraw
    let size = terminal.size()?;
    let rect = Rect::new(0, 0, size.width, size.height);
    terminal.resize(rect)?;

    Ok(())
}

#[cfg(unix)]
fn recreate_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::cursor;
    use crossterm::terminal::{Clear, ClearType};
    use ratatui::layout::Rect;

    // Completely flush and clear terminal
    terminal.clear()?;
    execute!(
        terminal.backend_mut(),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    terminal.flush()?;

    // Force a resize to trigger complete redraw
    let size = terminal.size()?;
    let rect = Rect::new(0, 0, size.width, size.height);
    terminal.resize(rect)?;

    Ok(())
}

fn ui(f: &mut Frame, app_state: &mut AppState) {
    // Ensure state is consistent before rendering
    app_state.validate_selected_type();

    let layout = create_main_layout(f.area());
    let visible_counts = calculate_visible_counts(&layout);

    // Update state with current visible counts
    app_state.visible_types = visible_counts.types;
    app_state.visible_services = visible_counts.services;

    render_service_types_list(f, app_state, layout.left_panel, visible_counts.types);
    render_services_list(f, app_state, layout.services_area, visible_counts.services);
    render_service_details(f, app_state, layout.details_area);

    // Render help popup if active
    if app_state.show_help_popup {
        render_help_popup(f);
    }
}

struct MainLayout {
    left_panel: ratatui::layout::Rect,
    services_area: ratatui::layout::Rect,
    details_area: ratatui::layout::Rect,
}

struct VisibleCounts {
    types: usize,
    services: usize,
}

fn create_main_layout(area: ratatui::layout::Rect) -> MainLayout {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let services_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    MainLayout {
        left_panel: chunks[0],
        services_area: services_chunks[0],
        details_area: services_chunks[1],
    }
}

fn calculate_visible_counts(layout: &MainLayout) -> VisibleCounts {
    VisibleCounts {
        types: (layout.left_panel.height as usize).saturating_sub(2), // Account for borders
        services: (layout.services_area.height as usize).saturating_sub(2), // Account for borders
    }
}

fn render_service_types_list(
    f: &mut Frame,
    app_state: &mut AppState,
    area: ratatui::layout::Rect,
    visible_types: usize,
) {
    let mut type_items = vec![ListItem::new(Line::from(Span::styled(
        "All Types".to_string(),
        if app_state.selected_type.is_none() {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        },
    )))];

    type_items.extend(
        app_state
            .service_types
            .iter()
            .enumerate()
            .map(|(i, service_type)| {
                let style = if app_state.selected_type == Some(i) {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };
                let display_type = format_service_type_for_display(service_type);
                ListItem::new(Line::from(Span::styled(display_type, style)))
            }),
    );

    let visible_type_items: Vec<ListItem> = type_items
        .into_iter()
        .skip(app_state.types_scroll_offset)
        .take(visible_types)
        .collect();

    let types_list = List::new(visible_type_items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Service Types [{}] (←/→)",
            app_state.service_types.len()
        )))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ListState::default();
    let display_index = match app_state.selected_type {
        None => 0,
        Some(idx) => idx + 1,
    }
    .saturating_sub(app_state.types_scroll_offset);
    list_state.select(Some(display_index));
    f.render_stateful_widget(types_list, area, &mut list_state);
}

fn render_services_list(
    f: &mut Frame,
    app_state: &mut AppState,
    area: ratatui::layout::Rect,
    visible_services: usize,
) {
    let selected_service_idx = app_state.selected_service;
    let services_clone = app_state.services.clone();
    let filtered_indices = app_state.get_filtered_services();
    let filtered_indices_len = filtered_indices.len();

    let service_items: Vec<ListItem> = filtered_indices
        .iter()
        .enumerate()
        .map(|(i, &service_idx)| {
            let service = &services_clone[service_idx];
            let style = create_service_list_item_style(i, selected_service_idx, service);
            let display_text = format_service_for_display(service);
            ListItem::new(Line::from(Span::styled(display_text, style)))
        })
        .collect();

    let visible_service_items: Vec<ListItem> = service_items
        .into_iter()
        .skip(app_state.services_scroll_offset)
        .take(visible_services)
        .collect();

    let services_list = List::new(visible_service_items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Services [{}/{}] (↑/↓)",
            filtered_indices_len,
            services_clone.len()
        )))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut services_list_state = ListState::default();
    services_list_state.select(Some(
        app_state
            .selected_service
            .saturating_sub(app_state.services_scroll_offset),
    ));
    f.render_stateful_widget(services_list, area, &mut services_list_state);
}

fn render_service_details(f: &mut Frame, app_state: &mut AppState, area: ratatui::layout::Rect) {
    let selected_service_idx = app_state.selected_service;
    let services_clone = app_state.services.clone();
    let error_to_display = app_state.last_error.clone();

    // Check if there's an error to display
    if let Some(error) = error_to_display {
        let error_text = format!("Error: {}", error);
        let error_details = Paragraph::new(error_text)
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title("Error"))
            .wrap(Wrap { trim: true });
        f.render_widget(error_details, area);
        return;
    }

    let filtered_indices = app_state.get_filtered_services();
    let selected_service = filtered_indices
        .get(selected_service_idx)
        .map(|&idx| &services_clone[idx]);

    if let Some(service) = selected_service {
        let details_text = create_service_details_text(service);
        let details = Paragraph::new(details_text.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Service Details"),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(details, area);
    } else {
        let details = Paragraph::new("No service selected").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Service Details"),
        );
        f.render_widget(details, area);
    }
}

fn render_help_popup(f: &mut Frame) {
    let help_content = vec![
        Line::from(""),
        Line::from(" Navigation:"),
        Line::from("   ↑/↓ or j/k          - Navigate services list"),
        Line::from("   ←/→ or h/l          - Switch between service types"),
        Line::from("   PageUp/Down         - Scroll services list by page"),
        Line::from("   b/f/Space           - Scroll services list by page"),
        Line::from("   Home/End            - Jump to first/last service"),
        Line::from(" "),
        Line::from(" Actions:"),
        Line::from("   d                   - Remove dead services"),
        Line::from("   c                   - Clear error message"),
        Line::from("   ?                   - Toggle this help popup"),
        Line::from("   q or Ctrl+C         - Quit the application"),
        #[cfg(unix)]
        Line::from("   Ctrl+Z              - Suspend the application (Unix only)"),
        Line::from(" "),
        Line::from(" Press any key to close this help"),
    ];

    let popup_area = create_centered_popup(f.area(), 60, 70);

    // Clear the background first
    f.render_widget(ratatui::widgets::Clear, popup_area);

    // Create a solid background block to ensure readability
    let background_block =
        ratatui::widgets::Block::default().style(Style::default().bg(ratatui::style::Color::Black));
    f.render_widget(background_block, popup_area);

    // Create inner area with padding by reducing the popup area
    let inner_area = ratatui::layout::Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let help_paragraph = Paragraph::new(help_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    f.render_widget(help_paragraph, inner_area);

    // Render border on top
    let border_block = Block::default()
        .borders(Borders::ALL)
        .title("Key Bindings")
        .title_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(border_block, popup_area);
}

fn create_centered_popup(
    parent_area: ratatui::layout::Rect,
    width_percent: u16,
    height_percent: u16,
) -> ratatui::layout::Rect {
    let popup_width = (parent_area.width * width_percent) / 100;
    let popup_height = (parent_area.height * height_percent) / 100;

    // Add margins (at least 2 cells on each side if possible)
    let margin_x = std::cmp::min(2, parent_area.width.saturating_sub(popup_width) / 2);
    let margin_y = std::cmp::min(1, parent_area.height.saturating_sub(popup_height) / 2);

    let x = parent_area.x + (parent_area.width - popup_width) / 2 + margin_x;
    let y = parent_area.y + (parent_area.height - popup_height) / 2 + margin_y;

    // Adjust width and height to account for margins
    let adjusted_width = popup_width - (margin_x * 2);
    let adjusted_height = popup_height - (margin_y * 2);

    ratatui::layout::Rect::new(x, y, adjusted_width, adjusted_height)
}

// Helper functions for formatting
fn format_service_type_for_display(service_type: &str) -> String {
    service_type
        .trim_start_matches('_')
        .trim_end_matches(".local.")
        .trim_end_matches(".")
        .replace("._tcp", ".tcp")
        .replace("._udp", ".udp")
}

fn create_service_list_item_style(
    index: usize,
    selected_index: usize,
    service: &ServiceEntry,
) -> Style {
    let foreground = if service.alive {
        Color::White
    } else {
        Color::LightMagenta
    };

    let mut style = if index == selected_index {
        Style::default().bg(Color::DarkGray).fg(foreground)
    } else {
        Style::default().fg(foreground)
    };

    if !service.alive {
        style = style.add_modifier(Modifier::ITALIC);
    }

    style
}

fn format_service_for_display(service: &ServiceEntry) -> String {
    let display_name = service
        .fullname
        .trim_end_matches(&service.service_type)
        .trim_end_matches(".");
    let display_host = service
        .host
        .trim_end_matches(".local.")
        .trim_end_matches(".");
    let address = service
        .addrs
        .first()
        .map(|a| a.to_string())
        .unwrap_or_else(|| "<no-addr>".into());
    format!(
        "{} - {} - {}:{}",
        display_name, display_host, address, service.port
    )
}

fn format_timestamp_micros(timestamp_micros: u64) -> String {
    use chrono::{DateTime, Local, Utc};

    let seconds = timestamp_micros / 1_000_000;
    let nanoseconds = (timestamp_micros % 1_000_000) * 1000;

    let datetime = DateTime::<Utc>::from_timestamp(seconds as i64, nanoseconds as u32)
        .unwrap_or_default()
        .with_timezone(&Local);

    datetime.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}

fn create_service_details_text(service: &ServiceEntry) -> String {
    let subtype_text = service
        .subtype
        .as_ref()
        .map(|s| format!("\nSubtype: {}", s))
        .unwrap_or_default();

    let status_text = if service.alive {
        format!(
            "Alive since: {}",
            format_timestamp_micros(service.timestamp_micros)
        )
    } else {
        format!(
            "Dead since: {}",
            format_timestamp_micros(service.timestamp_micros)
        )
    };

    let addresses_text = if service.addrs.is_empty() {
        "None".to_string()
    } else {
        service.addrs.join("\n")
    };

    let txt_text = if service.txt.is_empty() {
        "None".to_string()
    } else {
        service.txt.join("\n")
    };

    format!(
        "{}\n\nFullname: {}\nHostname: {}\nType: {}{}\nPort: {}\n\nAddresses:\n{}\n\nTXT Records:\n{}",
        status_text,
        service.fullname,
        service.host,
        service.service_type,
        subtype_text,
        service.port,
        addresses_text,
        txt_text
    )
}

pub async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal for full TUI
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize app state
    let state = Arc::new(RwLock::new(AppState::new()));

    // Create notification channels
    let (notification_sender, notification_receiver) = flume::unbounded::<Notification>();

    let mdns = ServiceDaemon::new()?;

    // Browse for all service types
    let receiver = mdns.browse("_services._dns-sd._udp.local.")?;
    let state_clone = Arc::clone(&state);
    let notification_sender_clone = notification_sender.clone();

    let mdns = mdns.clone();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            match event {
                ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                    let mut state = state_clone.write().await;
                    if state.remove_service_type(&fullname) {
                        let _ = notification_sender_clone.send(Notification::ServiceChanged);
                    }
                }
                ServiceEvent::ServiceFound(_service_type, fullname) => {
                    let service_type = fullname.to_string();
                    if !is_valid_service_type(&service_type) {
                        continue; // invalid service type format
                    }
                    {
                        let mut state = state_clone.write().await;
                        if state.add_service_type(&service_type) {
                            let _ = notification_sender_clone.send(Notification::ServiceChanged);
                        }
                    }
                    match mdns.browse(&service_type) {
                        Err(_) => {
                            // if a browse fails, that usually means the service type is invalid and
                            // should be removed from the service types list
                            let mut state = state_clone.write().await;
                            if state.remove_service_type(&service_type) {
                                let _ =
                                    notification_sender_clone.send(Notification::ServiceChanged);
                            }
                        }
                        Ok(service_receiver) => {
                            let state_inner = Arc::clone(&state_clone);
                            let notification_sender_inner = notification_sender_clone.clone();

                            tokio::spawn(async move {
                                while let Ok(service_event) = service_receiver.recv_async().await {
                                    match service_event {
                                        ServiceEvent::ServiceRemoved(service_type, fullname) => {
                                            let mut state = state_inner.write().await;
                                            if let Some(entry) = state
                                                .services
                                                .iter_mut()
                                                .find(|s| s.fullname == fullname)
                                            {
                                                entry.die_at(current_timestamp_micros());
                                                state.invalidate_cache_and_validate();
                                                state.remove_service_type(&service_type);
                                                let _ = notification_sender_inner
                                                    .send(Notification::ServiceChanged);
                                            }
                                        }
                                        ServiceEvent::ServiceResolved(service_info) => {
                                            let entry = ServiceEntry {
                                                fullname: service_info.get_fullname().to_string(),
                                                host: service_info.get_hostname().to_string(),
                                                service_type: service_info.ty_domain.to_string(),
                                                subtype: service_info
                                                    .get_subtype()
                                                    .as_ref()
                                                    .map(|s| s.to_string()),
                                                addrs: {
                                                    let mut addrs: Vec<String> = service_info
                                                        .get_addresses()
                                                        .iter()
                                                        .map(|ip| ip.to_string())
                                                        .collect();
                                                    addrs.sort();
                                                    addrs
                                                },
                                                port: service_info.get_port(),
                                                txt: {
                                                    let mut txt: Vec<String> = service_info
                                                        .get_properties()
                                                        .iter()
                                                        .filter_map(|prop| {
                                                            prop.val().map(|val| {
                                                                format!(
                                                                    "{}={}",
                                                                    prop.key(),
                                                                    String::from_utf8_lossy(val)
                                                                )
                                                            })
                                                        })
                                                        .collect();
                                                    txt.sort_by(|a, b| {
                                                        let a_key =
                                                            a.split('=').next().unwrap_or(a);
                                                        let b_key =
                                                            b.split('=').next().unwrap_or(b);
                                                        a_key.cmp(b_key)
                                                    });
                                                    txt
                                                },
                                                alive: true,
                                                timestamp_micros: current_timestamp_micros(),
                                            };
                                            let mut state = state_inner.write().await;
                                            if let Some(exist) = state
                                                .services
                                                .iter_mut()
                                                .find(|s| s.fullname == entry.fullname)
                                            {
                                                *exist = entry;
                                            } else {
                                                state.services.push(entry);
                                            }
                                            state.services.sort_by(|a, b| a.host.cmp(&b.host));
                                            state.invalidate_cache_and_validate();
                                            let _ = notification_sender_inner
                                                .send(Notification::ServiceChanged);
                                        }
                                        _ => (),
                                    }
                                }
                            });
                        }
                    }
                }
                _ => (),
            }
        }
    });

    // Initial render to show the UI immediately
    {
        let mut state = state.write().await;
        terminal.draw(|f| ui(f, &mut state))?;
    }

    let result = loop {
        tokio::select! {
            // Handle user input events
            event_result = async {
                match event::poll(Duration::from_millis(50)) {
                    Ok(true) => {
                        match event::read() {
                            Ok(event) => Some(event),
                            Err(e) => {
                                eprintln!("Error reading event: {}", e);
                                None
                            }
                        }
                    }
                    Ok(false) => None,
                    Err(e) => {
                        eprintln!("Error polling for events: {}", e);
                        None
                    }
                }
            } => {
                if let Some(event) = event_result {
                    match event {
                        Event::Key(key) => {
                            #[cfg(target_os = "windows")]
                            {
                                // On Windows, ignore key release events to prevent duplicate handling
                                if key.kind == crossterm::event::KeyEventKind::Release {
                                    continue;
                                }
                            }

                            let mut state = state.write().await;
                            let should_continue = state.handle_key_event(key, Some(notification_sender.clone()));
                            if should_continue {
                                let _ = notification_sender.send(Notification::UserInput);
                            } else {
                                break Ok(());
                            }
                        }
                        Event::Resize(_, _) => {
                            // Trigger a redraw on terminal resize
                            let _ = notification_sender.send(Notification::UserInput);
                        }
                        _ => {}
                    }
                }
            }

            // Handle notifications for rendering
            _notification = notification_receiver.recv_async() => {
                // Draw UI only when there's a notification
                {
                    let mut state = state.write().await;

                    // Force complete redraw after resume
                    #[cfg(unix)]
                    if matches!(_notification, Ok(Notification::ForceRedraw)) {
                        // Recreate terminal state completely
                        recreate_terminal(&mut terminal)?;
                        terminal.draw(|f| ui(f, &mut state))?;
                    } else {
                        terminal.draw(|f| ui(f, &mut state))?;
                    }
                }
            }
        }
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    result
}

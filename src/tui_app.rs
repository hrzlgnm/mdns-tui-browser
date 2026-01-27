use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, Wrap,
    },
};

use std::sync::Arc;
use std::time::Duration;
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
}

struct AppState {
    services: Vec<ServiceEntry>,
    service_types: Vec<String>,
    selected_service: usize,
    selected_type: Option<usize>,
    types_scroll_offset: usize,
    services_scroll_offset: usize,
    details_scroll_offset: usize,
    visible_types: usize,
    visible_services: usize,
    cached_filtered_services: Vec<usize>,
    cache_dirty: bool,
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
            details_scroll_offset: 0,
            visible_types: 0,
            visible_services: 0,
            cached_filtered_services: Vec::new(),
            cache_dirty: true,
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
            self.service_types.push(service_type.to_string());
            self.service_types.sort();
            self.invalidate_cache_and_validate();
            true
        } else {
            false
        }
    }

    fn remove_service_type(&mut self, service_type: &str) -> bool {
        let initial_len = self.service_types.len();
        self.service_types.retain(|s| s != service_type);
        let removed = self.service_types.len() < initial_len;
        if removed {
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

    fn invalidate_cache_and_validate(&mut self) {
        self.mark_cache_dirty();
        self.validate_selected_type();
    }
}

fn is_valid_service_type(service_type: &str) -> bool {
    // all other cases are caught by ServiceDaemon::browse
    !service_type.starts_with("_sub.")
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
    render_help_section(f, f.area());
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
    let filtered_indices = app_state.get_filtered_services();

    let selected_service = filtered_indices
        .get(selected_service_idx)
        .map(|&idx| &services_clone[idx]);

    if let Some(service) = selected_service {
        let details_text = create_service_details_text(service);
        let content_lines = details_text.lines().count();
        let clamped_offset = app_state
            .details_scroll_offset
            .min(content_lines.saturating_sub((area.height - 2) as usize));

        let details = Paragraph::new(details_text.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Service Details"),
            )
            .wrap(Wrap { trim: true })
            .scroll((clamped_offset as u16, 0));
        f.render_widget(details, area);

        // Render scrollbar for details if content is longer than available space
        let visible_lines = (area.height - 2) as usize;
        if content_lines > visible_lines {
            render_details_scrollbar(f, area, content_lines, clamped_offset);
        }
    } else {
        let details = Paragraph::new("No service selected").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Service Details"),
        );
        f.render_widget(details, area);
    }
}

fn render_help_section(f: &mut Frame, area: ratatui::layout::Rect) {
    let help_text = "Press 'q' to quit | hjkl/Arrows to navigate | PageUp/PageDown/Ctrl-u/Ctrl-d/b/f to scroll | g/G/Home/End for details";
    let help = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(
        help,
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(95), Constraint::Percentage(5)])
            .split(area)[1],
    );
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
    format!(
        "{} - {} - {}:{}",
        display_name,
        display_host,
        service.addrs.first().unwrap(),
        service.port
    )
}

fn create_service_details_text(service: &ServiceEntry) -> String {
    let subtype_text = service
        .subtype
        .as_ref()
        .map(|s| format!("\nSubtype: {}", s))
        .unwrap_or_default();

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
        "Fullname: {}\nHostname: {}\nType: {}{}\nPort: {}\n\nAddresses:\n{}\n\nTXT Records:\n{}",
        service.fullname,
        service.host,
        service.service_type,
        subtype_text,
        service.port,
        addresses_text,
        txt_text
    )
}

fn render_details_scrollbar(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    content_lines: usize,
    position: usize,
) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("█");

    let mut scrollbar_state =
        ratatui::widgets::ScrollbarState::new(content_lines).position(position);
    f.render_stateful_widget(
        scrollbar,
        area.inner(ratatui::layout::Margin::new(0, 0)),
        &mut scrollbar_state,
    );
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

    let mdns = ServiceDaemon::new()?;

    // Browse for all service types
    let receiver = mdns.browse("_services._dns-sd._udp.local.")?;
    let state_clone = Arc::clone(&state);

    let mdns = mdns.clone();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            match event {
                ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                    let mut state = state_clone.write().await;
                    state.remove_service_type(&fullname);
                }
                ServiceEvent::ServiceFound(_service_type, fullname) => {
                    let service_type = fullname.to_string();
                    if !is_valid_service_type(&service_type) {
                        continue; // invalid service type format
                    }
                    {
                        let mut state = state_clone.write().await;
                        state.add_service_type(&service_type);
                    }
                    match mdns.browse(&service_type) {
                        Err(_) => {
                            // if a browse fails, that usually means the service type is invalid and
                            // should be removed from the service types list
                            let mut state = state_clone.write().await;
                            state.remove_service_type(&service_type);
                        }
                        Ok(service_receiver) => {
                            let state_inner = Arc::clone(&state_clone);

                            tokio::spawn(async move {
                                while let Ok(service_event) = service_receiver.recv_async().await {
                                    match service_event {
                                        ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                                            let mut state = state_inner.write().await;
                                            if let Some(entry) = state
                                                .services
                                                .iter_mut()
                                                .find(|s| s.fullname == fullname)
                                            {
                                                entry.alive = false;
                                            }
                                            state.invalidate_cache_and_validate();
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
                                            state
                                                .services
                                                .sort_by(|a, b| a.fullname.cmp(&b.fullname));
                                            state.invalidate_cache_and_validate();
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

    let mut tick = tokio::time::interval(Duration::from_millis(50));

    let result = loop {
        // Handle events
        if event::poll(Duration::from_millis(50))? {
            let event = event::read()?;

            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('q') => break Ok(()),
                    KeyCode::Char('k') | KeyCode::Up => {
                        let mut state = state.write().await;
                        if state.selected_service > 0 {
                            state.selected_service -= 1;
                            // Update scroll offset for services list
                            if state.selected_service < state.services_scroll_offset {
                                state.services_scroll_offset = state.selected_service;
                            }
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let mut state = state.write().await;
                        let filtered = state.get_filtered_services();
                        let filtered_len = filtered.len();
                        if state.selected_service < filtered_len.saturating_sub(1) {
                            state.selected_service += 1;
                            // Update scroll offset for services list using actual visible count
                            if state.visible_services > 0
                                && state.selected_service
                                    >= state.services_scroll_offset + state.visible_services
                            {
                                state.services_scroll_offset =
                                    state.selected_service - state.visible_services + 1;
                            }
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        let mut state = state.write().await;
                        let new_type = match state.selected_type {
                            None => None,               // Already at "All Types", can't go further left
                            Some(0) => None, // Move from first service type to "All Types"
                            Some(idx) => Some(idx - 1), // Move to previous service type
                        };
                        if new_type.is_none() {
                            // Moving to "All Types" - ensure it's visible at visual index 0
                            state.types_scroll_offset = 0;
                        } else if let Some(new_idx) = new_type {
                            // Update scroll offset for types list using actual visible count
                            if new_idx < state.types_scroll_offset {
                                state.types_scroll_offset = new_idx;
                            }
                        }
                        state.update_service_type_selection(new_type);
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        let mut state = state.write().await;
                        let new_type = match state.selected_type {
                            None => {
                                // Move from "All Types" to first service type (index 0)
                                if !state.service_types.is_empty() {
                                    Some(0)
                                } else {
                                    None
                                }
                            }
                            Some(idx) if idx < state.service_types.len().saturating_sub(1) => {
                                Some(idx + 1)
                            }
                            Some(idx) => Some(idx), // Stay at last service type, don't wrap to "All Types"
                        };
                        if new_type.is_none() {
                            // Moving to "All Types" - ensure it's visible at visual index 0
                            state.types_scroll_offset = 0;
                        } else if let Some(new_idx) = new_type {
                            // Update scroll offset for types list using actual visible count
                            if state.visible_types > 0
                                && new_idx >= state.types_scroll_offset + state.visible_types
                            {
                                state.types_scroll_offset = new_idx - state.visible_types + 1;
                            }
                        }
                        state.update_service_type_selection(new_type);
                    }
                    KeyCode::Char('u')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        let mut state = state.write().await;
                        if state.details_scroll_offset > 0 {
                            state.details_scroll_offset =
                                state.details_scroll_offset.saturating_sub(5);
                        }
                    }
                    KeyCode::Char('d')
                        if key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        let mut state = state.write().await;
                        state.details_scroll_offset += 5;
                    }
                    KeyCode::PageUp | KeyCode::Char('b') => {
                        let mut state = state.write().await;
                        if state.details_scroll_offset > 0 {
                            state.details_scroll_offset =
                                state.details_scroll_offset.saturating_sub(5);
                        }
                    }
                    KeyCode::PageDown | KeyCode::Char('f') | KeyCode::Char(' ') => {
                        let mut state = state.write().await;
                        state.details_scroll_offset += 5;
                    }
                    KeyCode::Char('g') => {
                        let mut state = state.write().await;
                        state.details_scroll_offset = 0;
                    }
                    KeyCode::Char('G') => {
                        let mut state = state.write().await;
                        // Set to a high value, the UI will clamp it
                        state.details_scroll_offset = 1000;
                    }
                    KeyCode::Home => {
                        let mut state = state.write().await;
                        state.details_scroll_offset = 0;
                    }
                    KeyCode::End => {
                        let mut state = state.write().await;
                        // Set to a high value, the UI will clamp it
                        state.details_scroll_offset = 1000;
                    }
                    _ => {}
                }
            }
        }

        // Draw UI
        {
            let mut state = state.write().await;
            terminal.draw(|f| ui(f, &mut state))?;
        }

        tick.tick().await;
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    result
}

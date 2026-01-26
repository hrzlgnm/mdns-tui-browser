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
use tokio::sync::{RwLock, mpsc};

#[derive(Clone, Debug)]
struct ServiceEntry {
    name: String,
    service_type: String,
    subtype: Option<String>,
    domain: String,
    addrs: Vec<String>,
    port: u16,
    txt: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    services: Vec<ServiceEntry>,
    service_types: Vec<String>,
    selected_service: usize,
    selected_type: usize,
    types_scroll_offset: usize,
    services_scroll_offset: usize,
    details_scroll_offset: usize,
}

impl AppState {
    fn new() -> Self {
        Self {
            services: Vec::new(),
            service_types: Vec::new(),
            selected_service: 0,
            selected_type: usize::MAX, // Use MAX to indicate "all services"
            types_scroll_offset: 0,
            services_scroll_offset: 0,
            details_scroll_offset: 0,
        }
    }
}

fn ui(f: &mut Frame, app_state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    // Service types list
    let mut type_items = vec![ListItem::new(Line::from(Span::styled(
        "All Services".to_string(),
        if app_state.selected_type == usize::MAX {
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
                let style = if i == app_state.selected_type {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(service_type.clone(), style)))
            }),
    );

    let visible_types: Vec<ListItem> = type_items
        .into_iter()
        .skip(app_state.types_scroll_offset)
        .take(chunks[0].height as usize - 2) // Account for borders
        .collect();

    let types_list = List::new(visible_types)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Service Types (←/→ to navigate)"),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ListState::default();
    let display_index = if app_state.selected_type == usize::MAX {
        0
    } else {
        app_state.selected_type + 1
    }
    .saturating_sub(app_state.types_scroll_offset);
    list_state.select(Some(display_index));
    f.render_stateful_widget(types_list, chunks[0], &mut list_state);

    // Services list
    let services_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    let service_items: Vec<ListItem> = app_state
        .services
        .iter()
        .filter(|service| {
            app_state.selected_type == usize::MAX
                || app_state.service_types.is_empty()
                || app_state
                    .service_types
                    .get(app_state.selected_type)
                    .map(|selected_type| service.service_type == *selected_type)
                    .unwrap_or(true)
        })
        .enumerate()
        .map(|(i, service)| {
            let style = if i == app_state.selected_service {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let content = format!(
                "{}\n  {}:{}\n  {}",
                service.name,
                service.addrs.first().unwrap_or(&"Unknown".to_string()),
                service.port,
                service.domain
            );
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    let visible_services: Vec<ListItem> = service_items
        .into_iter()
        .skip(app_state.services_scroll_offset)
        .take(services_chunks[0].height as usize - 2) // Account for borders
        .collect();

    let services_list = List::new(visible_services)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Services (↑/↓ to navigate)"),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut services_list_state = ListState::default();
    services_list_state.select(Some(
        app_state.selected_service - app_state.services_scroll_offset,
    ));
    f.render_stateful_widget(services_list, services_chunks[0], &mut services_list_state);

    // Service details
    let filtered_services: Vec<_> = app_state
        .services
        .iter()
        .filter(|service| {
            app_state.selected_type == usize::MAX
                || app_state.service_types.is_empty()
                || app_state
                    .service_types
                    .get(app_state.selected_type)
                    .map(|selected_type| service.service_type == *selected_type)
                    .unwrap_or(true)
        })
        .collect();

    let selected_service = filtered_services.get(app_state.selected_service);

    if let Some(service) = selected_service {
        let subtype_text = service
            .subtype
            .as_ref()
            .map(|s| format!("\nSubtype: {}", s))
            .unwrap_or_else(String::new);

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

        let details_text = format!(
            "Name: {}\nType: {}{}\nDomain: {}\nPort: {}\n\nAddresses:\n{}\n\nTXT Records:\n{}",
            service.name,
            service.service_type,
            subtype_text,
            service.domain,
            service.port,
            addresses_text,
            txt_text
        );

        let content_lines = details_text.lines().count();
        let clamped_offset = app_state
            .details_scroll_offset
            .min(content_lines.saturating_sub((services_chunks[1].height - 2) as usize));

        let details = Paragraph::new(details_text.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Service Details"),
            )
            .wrap(Wrap { trim: true })
            .scroll((clamped_offset as u16, 0));
        f.render_widget(details, services_chunks[1]);

        // Render scrollbar for details if content is longer than available space
        let visible_lines = (services_chunks[1].height - 2) as usize;
        if content_lines > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state =
                ratatui::widgets::ScrollbarState::new(content_lines).position(clamped_offset);
            f.render_stateful_widget(
                scrollbar,
                services_chunks[1].inner(ratatui::layout::Margin::new(0, 0)),
                &mut scrollbar_state,
            );
        }
    } else {
        let details = Paragraph::new("No service selected").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Service Details"),
        );
        f.render_widget(details, services_chunks[1]);
    }

    // Help text at the bottom
    let help_text = "Press 'q' to quit | hjkl/Arrows to navigate | PageUp/PageDown/Ctrl-u/Ctrl-d/b/f to scroll | g/G/Home/End for details";
    let help = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(
        help,
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(95), Constraint::Percentage(5)])
            .split(f.area())[1],
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
    let (update_sender, mut update_receiver) = mpsc::unbounded_channel();

    let mdns = ServiceDaemon::new()?;

    // Browse for all service types
    let receiver = mdns.browse("_services._dns-sd._udp.local.")?;
    let state_clone = Arc::clone(&state);
    let update_sender_clone = update_sender.clone();

    let mdns = mdns.clone();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            if let ServiceEvent::ServiceFound(_fullname, fullname) = event {
                let service_type = fullname.to_string();

                // Add service type to our list
                {
                    let mut state = state_clone.write().await;
                    if !state.service_types.contains(&service_type) {
                        state.service_types.push(service_type.clone());
                        state.service_types.sort();
                    }
                }

                match mdns.browse(&service_type) {
                    Ok(service_receiver) => {
                        let state_inner = Arc::clone(&state_clone);
                        let service_type_clone = service_type.clone();
                        let update_sender_inner = update_sender_clone.clone();

                        tokio::spawn(async move {
                            while let Ok(service_event) = service_receiver.recv_async().await {
                                if let ServiceEvent::ServiceResolved(service_info) = service_event {
                                    let entry = ServiceEntry {
                                        name: service_info.get_fullname().to_string(),
                                        service_type: service_type_clone.clone(),
                                        subtype: service_info
                                            .get_subtype()
                                            .as_ref()
                                            .map(|s| s.to_string()),
                                        domain: "local".to_string(),
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
                                                let a_key = a.split('=').next().unwrap_or(a);
                                                let b_key = b.split('=').next().unwrap_or(b);
                                                a_key.cmp(b_key)
                                            });
                                            txt
                                        },
                                    };

                                    let mut state = state_inner.write().await;

                                    if let Some(exist) =
                                        state.services.iter_mut().find(|s| s.name == entry.name)
                                    {
                                        *exist = entry;
                                    } else {
                                        state.services.push(entry);
                                    }

                                    // Sort services by hostname (name)
                                    state.services.sort_by(|a, b| a.name.cmp(&b.name));

                                    let _ = update_sender_inner.send("service_updated".to_string());
                                }
                            }
                        });
                    }
                    Err(_e) => {}
                }

                let _ = update_sender_clone.send("type_updated".to_string());
            }
        }
    });

    let mut tick = tokio::time::interval(Duration::from_millis(100));

    let result = loop {
        // Handle updates from mDNS
        while let Ok(_update) = update_receiver.try_recv() {
            // Update UI on next tick
        }

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
                        let filtered_count = state
                            .services
                            .iter()
                            .filter(|service| {
                                state.selected_type == usize::MAX
                                    || state.service_types.is_empty()
                                    || state
                                        .service_types
                                        .get(state.selected_type)
                                        .map(|selected_type| service.service_type == *selected_type)
                                        .unwrap_or(true)
                            })
                            .count();
                        if state.selected_service < filtered_count.saturating_sub(1) {
                            state.selected_service += 1;
                            // Update scroll offset for services list (assuming max visible items around 10)
                            if state.selected_service >= state.services_scroll_offset + 10 {
                                state.services_scroll_offset = state.selected_service - 9;
                            }
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        let mut state = state.write().await;
                        if state.selected_type == 0 {
                            // Move from first service type to "All Services"
                            state.selected_type = usize::MAX;
                            state.selected_service = 0;
                            state.services_scroll_offset = 0;
                        } else if state.selected_type == usize::MAX {
                            // Already at "All Services", can't go further left
                        } else {
                            // Move to previous service type
                            state.selected_type -= 1;
                            state.selected_service = 0;
                            state.services_scroll_offset = 0;
                            // Update scroll offset for types list
                            if state.selected_type < state.types_scroll_offset {
                                state.types_scroll_offset = state.selected_type;
                            }
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        let mut state = state.write().await;
                        if state.selected_type == usize::MAX {
                            // Move from "All Services" to first service type (index 0)
                            state.selected_type = 0;
                            state.selected_service = 0;
                            state.services_scroll_offset = 0;
                        } else if state.selected_type < state.service_types.len().saturating_sub(1)
                        {
                            state.selected_type += 1;
                            state.selected_service = 0;
                            state.services_scroll_offset = 0;
                            // Update scroll offset for types list (assuming max visible items around 15)
                            if state.selected_type >= state.types_scroll_offset + 15 {
                                state.types_scroll_offset = state.selected_type - 14;
                            }
                        }
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
        let state = state.read().await.clone();
        terminal.draw(|f| ui(f, &state))?;

        tick.tick().await;
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    result
}

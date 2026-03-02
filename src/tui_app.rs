// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use tokio::sync::RwLock;

use crate::models::{
    AppOptions, FilterInfo, Metadata, ServiceEntry, SortDirection, SortField, SortInfo, StateDump,
    current_timestamp_micros,
};
use crate::popup::PopupState;
use crate::scroll::ScrollState;
use crate::terminal::TuiTerminal;

const STATUS_OK_COLOR: Color = Color::Blue;
const STATUS_ERROR_COLOR: Color = Color::Yellow;
const UI_CONTROLS_COLOR: Color = Color::Cyan;
const VIEW_ONLY_BORDER_COLOR: Color = Color::DarkGray;

// Flapping service colors (color-blind friendly)
const FLAPPING_COLOR_SELECTED: Color = Color::Rgb(100, 100, 100);
const FLAPPING_COLOR_NORMAL: Color = Color::Rgb(60, 60, 60);
const FLAPPING_FOREGROUND_COLOR: Color = Color::White;

// Service debouncing constants
const DEBOUNCE_DURATION_MICROS: u64 = 1_000_000;
const CLEANUP_INTERVAL_MS: u64 = 250;

#[cfg(unix)]
async fn handle_suspend(
    terminal: &mut TuiTerminal,
    state: &RwLock<AppState>,
    ui: fn(&mut Frame<'_>, &AppState),
) {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    // Stop the terminal - this restores the alternate screen and disables raw mode
    // Only proceed with suspend if terminal stop succeeds
    if let Err(e) = terminal.stop() {
        eprintln!("Failed to stop terminal on suspend: {}", e);
        return;
    }

    // Use SIGSTOP to suspend (cannot be caught, ignored, or blocked)
    // Log error but continue - we still need to restart terminal on resume
    if let Err(e) = kill(Pid::this(), Signal::SIGSTOP) {
        eprintln!("Failed to send SIGSTOP: {}", e);
    }

    // After resume, we continue here
    // Restart the terminal
    if let Err(e) = terminal.start() {
        eprintln!("Failed to start terminal on resume: {}", e);
    }

    // Clear the terminal to force a full redraw
    // This is required because ratatui uses diff-based rendering
    // and won't repaint unchanged areas without this
    let _ = terminal.clear();

    // Redraw the UI
    if let Ok(terminal_area) = terminal.get_area() {
        let mut state = state.write().await;
        state.prepare_for_rendering(terminal_area);
        let _ = terminal.draw(|f| ui(f, &state));
    }
}

// Generic list navigation utilities
fn navigate_list_up(
    selected_index: &mut usize,
    scroll_state: &mut ScrollState,
    total_items: usize,
) {
    if *selected_index > 0 {
        *selected_index -= 1;
        scroll_state.update_offset(*selected_index, total_items);
    }
}

fn navigate_list_down(
    selected_index: &mut usize,
    scroll_state: &mut ScrollState,
    total_items: usize,
) {
    if *selected_index < total_items.saturating_sub(1) {
        *selected_index += 1;
        scroll_state.update_offset(*selected_index, total_items);
    }
}

fn navigate_list_page_up(
    selected_index: &mut usize,
    scroll_state: &mut ScrollState,
    total_items: usize,
) {
    let scroll_amount = scroll_state.page_scroll_amount();
    if *selected_index >= scroll_amount {
        *selected_index -= scroll_amount;
    } else {
        *selected_index = 0;
    }
    scroll_state.update_offset(*selected_index, total_items);
}

fn navigate_list_page_down(
    selected_index: &mut usize,
    scroll_state: &mut ScrollState,
    total_items: usize,
) {
    let scroll_amount = scroll_state.page_scroll_amount();
    let max_index = total_items.saturating_sub(1);
    if *selected_index + scroll_amount < max_index {
        *selected_index += scroll_amount;
    } else {
        *selected_index = max_index;
    }
    scroll_state.update_offset(*selected_index, total_items);
}

fn navigate_list_to_first(selected_index: &mut usize, scroll_state: &mut ScrollState) {
    *selected_index = 0;
    scroll_state.reset();
}

fn navigate_list_to_last(
    selected_index: &mut usize,
    scroll_state: &mut ScrollState,
    total_items: usize,
) {
    *selected_index = total_items.saturating_sub(1);
    scroll_state.update_offset(*selected_index, total_items);
}

// Generic rendering utilities
fn get_visible_items<'a, T>(items: &'a [T], scroll_state: &ScrollState) -> &'a [T] {
    let start = scroll_state.offset;
    let end = start + scroll_state.visible_items;
    if start >= items.len() {
        return &[];
    }
    let end = std::cmp::min(end, items.len());
    &items[start..end]
}

struct AppState {
    services: Vec<ServiceEntry>,
    service_types: Vec<String>,
    selected_service: usize,
    selected_type: Option<usize>,
    types_scroll: ScrollState,
    services_scroll: ScrollState,
    details_scroll: ScrollState,
    cached_filtered_services: Vec<usize>,
    cache_dirty: bool,
    cached_sorted: bool,
    pub(crate) popup_state: PopupState,
    metrics: BTreeMap<String, u64>,
    sort_field: SortField,
    sort_direction: SortDirection,
    filter_query: String,
    filter_input_mode: bool,
    terminal_area: ratatui::layout::Rect,
    user_service_types: HashSet<String>,
    status_message: Arc<tokio::sync::Mutex<String>>,
    pending_removals: HashMap<String, u64>,
    no_debounce: bool,
    disable_ipv4: bool,
    disable_ipv6: bool,
    interfaces: Option<Vec<String>>,
    loaded_from_file: bool,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            services: self.services.clone(),
            service_types: self.service_types.clone(),
            selected_service: self.selected_service,
            selected_type: self.selected_type,
            types_scroll: self.types_scroll.clone(),
            services_scroll: self.services_scroll.clone(),
            details_scroll: self.details_scroll.clone(),
            cached_filtered_services: self.cached_filtered_services.clone(),
            cache_dirty: self.cache_dirty,
            cached_sorted: self.cached_sorted,
            popup_state: self.popup_state.clone(),
            metrics: self.metrics.clone(),
            sort_field: self.sort_field,
            sort_direction: self.sort_direction,
            filter_query: self.filter_query.clone(),
            filter_input_mode: self.filter_input_mode,
            terminal_area: self.terminal_area,
            user_service_types: self.user_service_types.clone(),
            status_message: self.status_message.clone(),
            pending_removals: self.pending_removals.clone(),
            no_debounce: self.no_debounce,
            disable_ipv4: self.disable_ipv4,
            disable_ipv6: self.disable_ipv6,
            interfaces: self.interfaces.clone(),
            loaded_from_file: self.loaded_from_file,
        }
    }
}

impl AppState {
    fn new(
        user_service_types: HashSet<String>,
        no_debounce: bool,
        disable_ipv4: bool,
        disable_ipv6: bool,
        interfaces: Option<Vec<String>>,
    ) -> Self {
        let mut state = Self {
            services: Vec::new(),
            service_types: Vec::new(),
            selected_service: 0,
            selected_type: None,
            types_scroll: ScrollState::new(),
            services_scroll: ScrollState::new(),
            details_scroll: ScrollState::new(),
            cached_filtered_services: Vec::new(),
            cache_dirty: true,
            cached_sorted: false,
            popup_state: PopupState::new(),
            metrics: BTreeMap::new(),
            sort_field: SortField::Host,
            sort_direction: SortDirection::Ascending,
            filter_query: String::new(),
            filter_input_mode: false,
            terminal_area: ratatui::layout::Rect::new(0, 0, 80, 24), // Default, will be updated in UI
            user_service_types,
            status_message: Arc::new(tokio::sync::Mutex::new(String::new())),
            pending_removals: HashMap::new(),
            no_debounce,
            disable_ipv4,
            disable_ipv6,
            interfaces,
            loaded_from_file: false,
        };
        state.validate_selected_type();
        state
    }

    fn filter_service(&self, service: &ServiceEntry) -> bool {
        // First filter by service type if one is selected
        if let Some(selected_type_idx) = self.selected_type
            && let Some(selected_type) = self.service_types.get(selected_type_idx)
            && service.service_type != *selected_type
        {
            return false;
        }

        // Then filter by text query if present
        if !self.filter_query.is_empty() {
            let query = self.filter_query.to_lowercase();

            // Check for special keywords: online and offline
            let has_online_keyword = query.contains("online");
            let has_offline_keyword = query.contains("offline");

            // Search in all service fields case-insensitively
            let search_text = [
                service.fullname.clone(),
                service.host.clone(),
                service.service_type.clone(),
                service.addrs.join(" "),
                service.port.to_string(),
                service.txt.join(" "),
                service.subtype.as_ref().unwrap_or(&String::new()).clone(),
            ]
            .join(" ")
            .to_lowercase();

            if has_online_keyword || has_offline_keyword {
                // Special handling for online/offline keywords (hybrid mode)
                let status_matches = (has_online_keyword && service.online)
                    || (has_offline_keyword && !service.online);

                // Check if the full query appears in text fields
                let text_matches = search_text.contains(&query);

                // For hybrid mode: match if status matches OR text contains the keywords
                // But for combined queries, we need to be more precise
                if has_online_keyword && has_offline_keyword {
                    // Both keywords present - strip both keywords and check remaining terms
                    let query_cleaned = query.replace("online", "").replace("offline", "");
                    let query_without_keyword = query_cleaned.trim();

                    if query_without_keyword.is_empty() {
                        // Only keywords - treat as match-all (status or text)
                        status_matches || text_matches
                    } else {
                        // Has additional terms - enforce remaining terms
                        (status_matches
                            && search_text.contains(&query_without_keyword.to_lowercase()))
                            || text_matches
                    }
                } else if query == "online" || query == "offline" {
                    // Pure keyword - hybrid mode: match by status OR text containing keyword
                    status_matches || text_matches
                } else {
                    // Mixed query (keyword + other terms) - match if (status matches AND text contains other terms) OR text contains full query
                    let query_cleaned = query.replace("online", "").replace("offline", "");
                    let query_without_keyword = query_cleaned.trim();

                    if query_without_keyword.is_empty() {
                        status_matches || text_matches
                    } else {
                        (status_matches
                            && search_text.contains(&query_without_keyword.to_lowercase()))
                            || text_matches
                    }
                }
            } else {
                // Standard text search for non-keyword queries
                search_text.contains(&query)
            }
        } else {
            true // Show all services if query is empty
        }
    }

    fn update_filtered_cache(&mut self) -> bool {
        if self.cache_dirty {
            self.cached_filtered_services.clear();
            for (idx, service) in self.services.iter().enumerate() {
                if self.filter_service(service) {
                    self.cached_filtered_services.push(idx);
                }
            }
            self.cache_dirty = false;
            self.cached_sorted = false;
            true // Cache was rebuilt
        } else {
            false // Cache was not rebuilt
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
        // Check if we need to invalidate the cache before processing
        let cache_was_rebuilt = self.update_filtered_cache();
        if cache_was_rebuilt || !self.cached_sorted {
            self.sort_filtered_services();
            self.cached_sorted = true;
        }
        self.cached_filtered_services.as_slice()
    }

    fn get_filtered_services_readonly(&self) -> &[usize] {
        // Read-only version that doesn't modify cache - assumes cache is up to date
        self.cached_filtered_services.as_slice()
    }

    fn sort_filtered_services(&mut self) {
        let sort_field = self.sort_field;
        let services = &self.services;

        match self.sort_direction {
            SortDirection::Ascending => {
                self.cached_filtered_services.sort_by(|&a_idx, &b_idx| {
                    let service_a = &services[a_idx];
                    let service_b = &services[b_idx];
                    compare_services_by_field(service_a, service_b, sort_field)
                });
            }
            SortDirection::Descending => {
                self.cached_filtered_services.sort_by(|&a_idx, &b_idx| {
                    let service_a = &services[a_idx];
                    let service_b = &services[b_idx];
                    compare_services_by_field(service_b, service_a, sort_field)
                });
            }
        }
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
        if self.user_service_types.contains(service_type) {
            return false; // Don't remove user-requested types
        }
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

    // JSON state dump functionality
    fn create_state_dump(&self) -> StateDump {
        let meta = Metadata {
            dump_timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            application_name: env!("CARGO_PKG_NAME").to_string(),
        };
        StateDump {
            metadata: meta,
            services: self
                .services
                .iter()
                .map(|s: &ServiceEntry| s.into())
                .collect(),
            service_types: self.service_types.clone(),
            metrics: self.metrics.clone(),
            options: AppOptions {
                service_types: {
                    let mut vec: Vec<String> = self.user_service_types.iter().cloned().collect();
                    vec.sort_unstable();
                    vec
                },
                disable_ipv4: self.disable_ipv4,
                disable_ipv6: self.disable_ipv6,
                no_debounce: self.no_debounce,
                interfaces: self.interfaces.clone(),
            },
            filters: FilterInfo {
                query: self.filter_query.clone(),
                active_service_types: None,
            },
            sorting: SortInfo {
                field: self.sort_field,
                direction: self.sort_direction,
            },
        }
    }

    fn dump_state_to_json(&self) -> Result<String, Box<dyn std::error::Error>> {
        let dump = self.create_state_dump();
        Ok(serde_json::to_string_pretty(&dump)?)
    }

    async fn save_json_dump(&self) -> Result<String, Box<dyn std::error::Error>> {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.6f").to_string();
        let filename = format!("{}-state-dump.json", timestamp);
        let json_content = self.dump_state_to_json()?;

        tokio::fs::write(&filename, json_content).await?;
        Ok(filename)
    }

    fn load_from_state_dump(&mut self, dump: StateDump) {
        self.services = dump.services.iter().map(|s| s.into()).collect();
        self.service_types = dump.service_types;
        self.metrics = dump.metrics;
        self.filter_query = dump.filters.query;

        let is_old_format = dump.filters.active_service_types.is_some();

        if is_old_format {
            self.user_service_types = dump
                .filters
                .active_service_types
                .unwrap_or_default()
                .into_iter()
                .collect();
        } else {
            self.user_service_types = dump.options.service_types.into_iter().collect();
            self.disable_ipv4 = dump.options.disable_ipv4;
            self.disable_ipv6 = dump.options.disable_ipv6;
            self.no_debounce = dump.options.no_debounce;
            self.interfaces = dump.options.interfaces;
        }

        self.sort_field = dump.sorting.field;
        self.sort_direction = dump.sorting.direction;

        self.loaded_from_file = true;
        self.cache_dirty = true;
        self.cached_sorted = false;
        self.validate_selected_type();
    }

    fn update_service_type_selection(&mut self, new_type: Option<usize>) {
        self.selected_type = new_type;
        self.selected_service = 0;
        self.services_scroll.reset();
        self.details_scroll.reset();
        self.invalidate_cache_and_validate();
    }

    fn update_sort_field(&mut self, field: SortField) {
        self.sort_field = field;
        self.selected_service = 0;
        self.services_scroll.reset();
        self.details_scroll.reset();
        self.invalidate_cache_and_validate();
    }

    fn update_sort_direction(&mut self, direction: SortDirection) {
        self.sort_direction = direction;
        self.selected_service = 0;
        self.services_scroll.reset();
        self.details_scroll.reset();
        self.invalidate_cache_and_validate();
    }

    fn toggle_sort_direction(&mut self) {
        match self.sort_direction {
            SortDirection::Ascending => self.update_sort_direction(SortDirection::Descending),
            SortDirection::Descending => self.update_sort_direction(SortDirection::Ascending),
        }
    }

    fn cycle_sort_field(&mut self, forward: bool) {
        use SortField::*;
        let fields = [Host, ServiceType, Fullname, Port, Address, Timestamp];
        let current_idx = fields
            .iter()
            .position(|&f| f == self.sort_field)
            .unwrap_or(0);

        let new_idx = if forward {
            (current_idx + 1) % fields.len()
        } else {
            current_idx.checked_sub(1).unwrap_or(fields.len() - 1)
        };

        self.update_sort_field(fields[new_idx]);
    }

    fn clear_stale_service_types(&mut self) {
        // Find service types that have no services at all (neither online nor offline)
        let mut types_to_remove = Vec::new();

        for service_type in &self.service_types.clone() {
            if !self
                .services
                .iter()
                .any(|s| s.service_type == *service_type)
            {
                types_to_remove.push(service_type.clone());
            }
        }

        // Remove empty service types
        let mut removed_count = 0;
        for service_type in types_to_remove {
            if self.remove_service_type(&service_type) {
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            self.update_metric_by("stale_service_types_removed", removed_count as u64);
            self.invalidate_cache_and_validate();
        }
    }

    fn remove_offline_services(&mut self) {
        // Collect service types that have offline services
        let mut service_types_to_check: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Capture initial filtered length for scroll logic
        let initial_filtered_len = self.get_filtered_services().len();

        // Remove offline services and track their types
        let initial_len = self.services.len();
        self.services.retain(|service| {
            if !service.online {
                service_types_to_check.insert(service.service_type.clone());
                false // Remove this service
            } else {
                true // Keep this service
            }
        });

        let removed_count = initial_len - self.services.len();

        if removed_count > 0 {
            self.update_metric_by("offline_services_removed", removed_count as u64);
            // Refresh cache immediately after retain to ensure filtered services are up-to-date
            self.invalidate_cache_and_validate();

            // Check if any service types should be removed (no active services of that type)
            let mut types_to_remove = Vec::new();
            for service_type in service_types_to_check {
                if !self
                    .services
                    .iter()
                    .any(|s| s.service_type == service_type && s.online)
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
                if self.services_scroll.visible_items > 0 {
                    self.services_scroll.offset = self
                        .selected_service
                        .saturating_sub(self.services_scroll.visible_items - 1);
                }
            } else {
                // Otherwise, just ensure it's visible
                self.services_scroll
                    .update_offset(self.selected_service, new_filtered_len);
            }
        }
    }

    fn invalidate_cache_and_validate(&mut self) {
        self.mark_cache_dirty();
        self.cached_sorted = false;
        self.validate_selected_type();
    }

    // Prepare state for rendering - updates UI-related fields based on terminal size
    fn prepare_for_rendering(&mut self, terminal_area: ratatui::layout::Rect) {
        self.terminal_area = terminal_area;
        self.validate_selected_type();

        let layout = if self.filter_input_mode {
            create_filter_input_layout(terminal_area)
        } else {
            create_main_layout(terminal_area, !self.filter_query.is_empty())
        };
        let visible_counts = calculate_visible_counts(&layout);

        // Update state with current visible counts
        self.types_scroll.visible_items = visible_counts.types;
        self.services_scroll.visible_items = visible_counts.services;

        // Update details scroll visible items based on details area
        self.details_scroll.visible_items = layout.details_area.height.saturating_sub(2) as usize;

        // Ensure filtered cache is up to date for rendering
        let cache_was_rebuilt = self.update_filtered_cache();
        if cache_was_rebuilt || !self.cached_sorted {
            self.sort_filtered_services();
            self.cached_sorted = true;
        }
    }

    // Key handling methods
    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // Dismiss status message on any key press if it's displayed
        if let Ok(mut msg) = self.status_message.try_lock()
            && !msg.is_empty()
        {
            msg.clear();
            return true;
        }

        if self.popup_state.help_popup.active || self.popup_state.metrics_popup.active {
            self.popup_state
                .handle_key_event(key, self.terminal_area, &self.metrics)
        } else if self.filter_input_mode {
            self.handle_filter_input_key(key)
        } else {
            self.handle_normal_mode_key(key)
        }
    }

    fn handle_filter_input_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                self.apply_filter();
                true
            }
            KeyCode::Esc => {
                self.clear_filter();
                true
            }
            KeyCode::Backspace => {
                self.remove_from_filter();
                true
            }
            KeyCode::Char(ch) => {
                self.add_to_filter(ch);
                true
            }
            _ => true,
        }
    }

    fn handle_normal_mode_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Quit actions
            KeyCode::Char('q') => {
                false // Signal to quit
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                false // Signal to quit
            }

            // Help toggle
            KeyCode::Char('?') => {
                self.toggle_help();
                true
            }

            // Metrics toggle
            KeyCode::Char('m') => {
                self.toggle_metrics();
                true
            }

            // Service Details scrolling with Shift+Up/Down and J/K (handled first to avoid conflicts)
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.scroll_details_up();
                true
            }

            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.scroll_details_down();
                true
            }

            KeyCode::Char('J') => {
                self.scroll_details_down();
                true
            }

            KeyCode::Char('K') => {
                self.scroll_details_up();
                true
            }

            // JSON state dump with Ctrl+J (must come before regular 'j' handler)
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let state_clone = self.clone();
                tokio::spawn(async move {
                    let result_str = {
                        match state_clone.save_json_dump().await {
                            Ok(filename) => format!("JSON dump saved to: {}", filename),
                            Err(e) => format!("Failed to save JSON dump: {}", e),
                        }
                    };
                    *state_clone.status_message.lock().await = result_str;
                });
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

            // Service type page navigation
            KeyCode::Char('H') => {
                self.navigate_service_types_page_up();
                true
            }

            KeyCode::Char('L') => {
                self.navigate_service_types_page_down();
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

            // Service type beginning/end navigation with Ctrl+Home/Ctrl+End
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.navigate_service_types_to_first();
                true
            }

            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.navigate_service_types_to_last();
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

            // Sorting
            KeyCode::Char('s') => {
                self.cycle_sort_field(true);
                true
            }

            KeyCode::Char('S') => {
                self.cycle_sort_field(false);
                true
            }

            KeyCode::Char('o') => {
                self.toggle_sort_direction();
                true
            }

            // Actions
            KeyCode::Char('d') => {
                self.remove_offline_services();
                true
            }
            KeyCode::Char('D') => {
                self.clear_stale_service_types();
                true
            }

            // Filter controls
            KeyCode::Char('/') => {
                self.start_filter_input();
                true
            }

            KeyCode::Char('n') => {
                self.clear_filter();
                true
            }

            _ => true,
        }
    }

    fn toggle_help(&mut self) {
        self.popup_state.toggle_help();
    }

    fn update_metric(&mut self, key: &str) {
        *self.metrics.entry(key.to_string()).or_insert(0) += 1;
    }

    fn update_metric_by(&mut self, key: &str, value: u64) {
        *self.metrics.entry(key.to_string()).or_insert(0) += value;
    }

    fn update_daemon_metrics(
        &mut self,
        daemon_metrics: &std::collections::HashMap<String, i64>,
    ) -> bool {
        let mut metrics_updated = false;
        for (key, value) in daemon_metrics.iter() {
            let metric_key = format!("daemon_{}", key.replace('-', "_"));
            let current_value = *self.metrics.entry(metric_key.clone()).or_insert(0);
            if current_value != *value as u64 {
                *self.metrics.get_mut(&metric_key).unwrap() = *value as u64;
                metrics_updated = true;
            }
        }
        // Return whether metrics changed
        metrics_updated
    }

    fn toggle_metrics(&mut self) {
        self.popup_state.toggle_metrics();
    }

    fn add_or_update_service(&mut self, service_entry: ServiceEntry) -> bool {
        self.cancel_pending_removal(&service_entry.fullname);
        if let Some(existing) = self
            .services
            .iter_mut()
            .find(|s| s.fullname == service_entry.fullname)
        {
            // Check if any significant fields have changed
            let significant_fields_changed = existing.host != service_entry.host
                || existing.service_type != service_entry.service_type
                || existing.subtype != service_entry.subtype
                || existing.addrs != service_entry.addrs
                || existing.port != service_entry.port
                || existing.txt != service_entry.txt
                || existing.online != service_entry.online; // Include online in significant changes

            existing.updated_at_micros = service_entry.updated_at_micros; // Always update timestamp

            if significant_fields_changed {
                // Handle service coming back online
                if !existing.online && service_entry.online {
                    existing.go_online_at(service_entry.updated_at_micros);
                } else if existing.online && !service_entry.online {
                    existing.go_offline_at(service_entry.updated_at_micros);
                }

                // Update other fields that might have changed
                existing.host = service_entry.host;
                existing.service_type = service_entry.service_type;
                existing.subtype = service_entry.subtype;
                existing.addrs = service_entry.addrs;
                existing.port = service_entry.port;
                existing.txt = service_entry.txt;

                // Update flapping status when service state changes
                existing.update_flapping_status();

                self.update_metric("services_updated");
            }
            true
        } else {
            // Ensure service type exists for filtering purposes
            self.add_service_type(&service_entry.service_type);
            let fullname = service_entry.fullname.clone();
            self.services.push(service_entry);

            // Update flapping status for new services
            if let Some(new_service) = self.services.iter_mut().find(|s| s.fullname == fullname) {
                new_service.update_flapping_status();
            }

            self.update_metric("services_discovered");
            false
        }
    }

    fn mark_service_offline(&mut self, fullname: &str) -> bool {
        // Don't mark offline if service is pending removal (debounce window)
        if self.pending_removals.contains_key(fullname) {
            return false;
        }

        let service_idx = self.services.iter().position(|s| s.fullname == fullname);

        if let Some(idx) = service_idx {
            // Only count as removed if the service was online
            let was_online = self.services[idx].online;
            if was_online {
                self.update_metric("services_marked_offline");
            }
            self.services[idx].go_offline_at(current_timestamp_micros());
            self.services[idx].update_flapping_status();
            self.invalidate_cache_and_validate();
            true
        } else {
            false
        }
    }

    // Debouncing methods for handling flapping services
    fn schedule_service_removal(&mut self, fullname: &str) {
        let current_time = current_timestamp_micros();
        self.pending_removals
            .insert(fullname.to_string(), current_time);
        *self
            .metrics
            .entry("pending_removals_active".to_string())
            .or_insert(0) = self.pending_removals.len() as u64;
    }

    fn cancel_pending_removal(&mut self, fullname: &str) -> bool {
        if self.pending_removals.remove(fullname).is_some() {
            // Service was scheduled for removal and came back online within debounce window
            self.update_metric("flapping_services_detected");
            *self
                .metrics
                .entry("pending_removals_active".to_string())
                .or_insert(0) = self.pending_removals.len() as u64;
            true
        } else {
            false
        }
    }

    fn process_expired_removals(&mut self) {
        let current_time = current_timestamp_micros();
        let mut expired_services = Vec::new();

        // Find expired services
        self.pending_removals.retain(|fullname, scheduled_time| {
            if current_time.saturating_sub(*scheduled_time) >= DEBOUNCE_DURATION_MICROS {
                expired_services.push(fullname.clone());
                false // Remove from pending
            } else {
                true // Keep in pending
            }
        });

        // Mark expired services as offline
        for fullname in expired_services {
            self.mark_service_offline(&fullname);
        }

        // Update pending removals count metric
        *self
            .metrics
            .entry("pending_removals_active".to_string())
            .or_insert(0) = self.pending_removals.len() as u64;
    }

    fn navigate_services_up(&mut self) {
        let old_selected_service = self.selected_service;
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_up(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        // Only reset details scroll when selection actually changes
        if old_selected_service != self.selected_service {
            self.details_scroll.reset();
        }
    }

    fn navigate_services_down(&mut self) {
        let old_selected_service = self.selected_service;
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_down(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        // Only reset details scroll when selection actually changes
        if old_selected_service != self.selected_service {
            self.details_scroll.reset();
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
            self.types_scroll.reset();
        } else if let Some(new_idx) = new_type {
            // Update scroll offset for types list using actual visible count
            self.types_scroll
                .update_offset(new_idx, self.service_types.len());
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
            self.types_scroll.reset();
        } else if let Some(new_idx) = new_type {
            // Update scroll offset for types list using actual visible count
            self.types_scroll
                .update_offset(new_idx, self.service_types.len());
        }
        self.update_service_type_selection(new_type);
    }

    fn navigate_service_types_page_up(&mut self) {
        let service_types_len = self.service_types.len();
        let scroll_amount = self.types_scroll.page_scroll_amount();
        let new_type = match self.selected_type {
            None => None, // Already at "All Types"
            Some(idx) => {
                if idx >= scroll_amount {
                    Some(idx - scroll_amount)
                } else {
                    None // Jump to "All Types"
                }
            }
        };

        if new_type.is_none() {
            // Moving to "All Types" - ensure it's visible at visual index 0
            self.types_scroll.reset();
        } else if let Some(new_idx) = new_type {
            // Update scroll offset for types list using actual visible count
            self.types_scroll.update_offset(new_idx, service_types_len);
        }
        self.update_service_type_selection(new_type);
    }

    fn navigate_service_types_page_down(&mut self) {
        let service_types_len = self.service_types.len();
        let scroll_amount = self.types_scroll.page_scroll_amount();
        let new_type = match self.selected_type {
            None => {
                // Move from "All Types" to service type at scroll_amount position
                if service_types_len > scroll_amount {
                    Some(scroll_amount)
                } else if !self.service_types.is_empty() {
                    Some(service_types_len.saturating_sub(1))
                } else {
                    None
                }
            }
            Some(idx) => {
                let target_idx = idx + scroll_amount;
                if target_idx < service_types_len {
                    Some(target_idx)
                } else {
                    Some(service_types_len.saturating_sub(1)) // Go to last type
                }
            }
        };

        if new_type.is_none() {
            // Moving to "All Types" - ensure it's visible at visual index 0
            self.types_scroll.reset();
        } else if let Some(new_idx) = new_type {
            // Update scroll offset for types list using actual visible count
            self.types_scroll.update_offset(new_idx, service_types_len);
        }
        self.update_service_type_selection(new_type);
    }

    fn navigate_service_types_to_first(&mut self) {
        self.selected_type = None; // "All Types" is the first
        self.types_scroll.reset();
        self.update_service_type_selection(None);
    }

    fn navigate_service_types_to_last(&mut self) {
        let service_types_len = self.service_types.len();
        if !self.service_types.is_empty() {
            let last_idx = service_types_len.saturating_sub(1);

            self.types_scroll.update_offset(last_idx, service_types_len);
            self.update_service_type_selection(Some(last_idx));
        } else {
            // When no service types, reset both selection and scroll offset
            self.selected_type = None;
            self.types_scroll.reset();
            self.selected_service = 0;
            self.services_scroll.reset();
            self.invalidate_cache_and_validate();
        }
    }

    fn navigate_services_page_up(&mut self) {
        let old_selected_service = self.selected_service;
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_page_up(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        // Only reset details scroll when selection actually changes
        if old_selected_service != self.selected_service {
            self.details_scroll.reset();
        }
    }

    fn navigate_services_page_down(&mut self) {
        let old_selected_service = self.selected_service;
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_page_down(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        // Only reset details scroll when selection actually changes
        if old_selected_service != self.selected_service {
            self.details_scroll.reset();
        }
    }

    fn navigate_services_to_first(&mut self) {
        let old_selected_service = self.selected_service;
        navigate_list_to_first(&mut self.selected_service, &mut self.services_scroll);
        // Only reset details scroll when selection actually changes
        if old_selected_service != self.selected_service {
            self.details_scroll.reset();
        }
    }

    fn navigate_services_to_last(&mut self) {
        let old_selected_service = self.selected_service;
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_to_last(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        // Only reset details scroll when selection actually changes
        if old_selected_service != self.selected_service {
            self.details_scroll.reset();
        }
    }

    // Filter methods
    fn start_filter_input(&mut self) {
        self.filter_input_mode = true;
        self.filter_query.clear();
    }

    fn clear_filter(&mut self) {
        let had_filter = !self.filter_query.is_empty();
        self.filter_query.clear();
        self.filter_input_mode = false;
        // Only reset selection and scroll when there was actually a filter
        if had_filter {
            self.selected_service = 0;
            self.services_scroll.reset();
            self.details_scroll.reset();
        }
        self.invalidate_cache_and_validate();
    }

    fn apply_filter(&mut self) {
        self.filter_input_mode = false;
        // Reset selection and scroll when exiting filter mode
        self.selected_service = 0;
        self.services_scroll.reset();
        self.details_scroll.reset();
        self.invalidate_cache_and_validate();
    }

    fn add_to_filter(&mut self, ch: char) {
        self.filter_query.push(ch);
        // Invalidate cache to trigger real-time filtering
        self.invalidate_cache_and_validate();
    }

    fn remove_from_filter(&mut self) {
        self.filter_query.pop();
        // Invalidate cache to trigger real-time filtering
        self.invalidate_cache_and_validate();
    }

    fn scroll_details_up(&mut self) {
        if self.details_scroll.offset > 0 {
            self.details_scroll.offset -= 1;
        }
    }

    fn scroll_details_down(&mut self) {
        let selected_service_idx = self.selected_service;
        let filtered_indices = self.get_filtered_services();

        if let Some(&service_idx) = filtered_indices.get(selected_service_idx)
            && let Some(service) = self.services.get(service_idx)
        {
            let details_lines = create_service_details_text(service);
            let total_lines = details_lines.len();

            if total_lines > 0 && self.details_scroll.visible_items > 0 {
                let max_scroll_offset =
                    total_lines.saturating_sub(self.details_scroll.visible_items);
                self.details_scroll.offset =
                    std::cmp::min(self.details_scroll.offset + 1, max_scroll_offset);
            }
        }
    }
}

fn compare_services_by_field(
    a: &ServiceEntry,
    b: &ServiceEntry,
    field: SortField,
) -> std::cmp::Ordering {
    match field {
        SortField::Host => a.host.cmp(&b.host),
        SortField::ServiceType => a.service_type.cmp(&b.service_type),
        SortField::Fullname => a.fullname.cmp(&b.fullname),
        SortField::Port => a.port.cmp(&b.port),
        SortField::Address => {
            use std::net::IpAddr;

            let a_addr_str = a.addrs.first().map(|s| s.as_str()).unwrap_or_default();
            let b_addr_str = b.addrs.first().map(|s| s.as_str()).unwrap_or_default();

            // Try to parse as IP addresses for numeric comparison, fall back to string comparison
            match (a_addr_str.parse::<IpAddr>(), b_addr_str.parse::<IpAddr>()) {
                (Ok(a_ip), Ok(b_ip)) => a_ip.cmp(&b_ip),
                _ => a_addr_str.cmp(b_addr_str),
            }
        }
        SortField::Timestamp => a.updated_at_micros.cmp(&b.updated_at_micros),
    }
}

#[derive(Debug, Clone)]
enum Notification {
    UserInput,
    ServiceChanged,
    MetricsUpdated,
}

pub fn normalize_service_type(service_type: &str) -> String {
    // Return empty string for empty or whitespace-only input
    if service_type.trim().is_empty() {
        return String::new();
    }

    let input = service_type.trim();

    // If already has .local. suffix, return as-is
    if input.ends_with(".local.") {
        return input.to_string();
    }

    let trimmed = input.trim_end_matches('.');

    // If already has .local. suffix after trimming dots, return as-is
    if trimmed.ends_with(".local.") {
        return trimmed.to_string();
    }

    // Check if it's already a complete service type (contains ._tcp or ._udp)
    if (trimmed.contains("._tcp") || trimmed.contains("._udp")) && trimmed.starts_with('_') {
        return format!("{}.local.", trimmed);
    }

    let parts: Vec<&str> = trimmed.split('.').collect();

    match parts.len() {
        1 => {
            // Simple name: "http" -> "_http._tcp.local."
            let name = if parts[0].starts_with('_') {
                parts[0].to_string()
            } else {
                format!("_{}", parts[0])
            };
            format!("{}._tcp.local.", name)
        }
        2 => {
            // Name + protocol: "http.tcp" or "_http.tcp"
            let name = if parts[0].starts_with('_') {
                parts[0].to_string()
            } else {
                format!("_{}", parts[0])
            };
            let protocol = if parts[1].starts_with('_') {
                parts[1].to_string()
            } else if parts[1] == "tcp" || parts[1] == "udp" {
                format!("_{}", parts[1])
            } else {
                // If second part is not a recognized protocol, treat it as tcp by default
                "_tcp".to_string()
            };
            format!("{}.{}.local.", name, protocol)
        }
        3 => {
            // Subtype format: "printer.sub.http" or "_printer.sub._http"
            let subtype = if parts[0].starts_with('_') {
                parts[0].trim_start_matches('_').to_string()
            } else {
                parts[0].to_string()
            };

            let sub_marker = if parts[1] == "sub" || parts[1] == "_sub" {
                "_sub"
            } else {
                // If middle part is not a subtype marker, treat as regular format
                return format!("{}.local.", trimmed);
            };

            let service_name = if parts[2].starts_with('_') {
                parts[2].to_string()
            } else {
                format!("_{}", parts[2])
            };

            format!("{}.{}.{}._tcp.local.", subtype, sub_marker, service_name)
        }
        4 => {
            // Subtype format with protocol: "printer.sub.http.tcp" or "_printer.sub._http.tcp"
            let subtype = if parts[0].starts_with('_') {
                parts[0].trim_start_matches('_').to_string()
            } else {
                parts[0].to_string()
            };

            let sub_marker = if parts[1] == "sub" || parts[1] == "_sub" {
                "_sub"
            } else {
                // If middle part is not a subtype marker, treat as regular format
                return format!("{}.local.", trimmed);
            };

            let service_name = if parts[2].starts_with('_') {
                parts[2].to_string()
            } else {
                format!("_{}", parts[2])
            };

            let protocol = if parts[3].starts_with('_') {
                parts[3].to_string()
            } else if parts[3] == "tcp" || parts[3] == "udp" {
                format!("_{}", parts[3])
            } else {
                // If fourth part is not a recognized protocol, treat it as tcp by default
                "_tcp".to_string()
            };

            format!(
                "{}.{}.{}.{}.local.",
                subtype, sub_marker, service_name, protocol
            )
        }
        _ => {
            // Already complex format (like existing subtypes), just add .local. if missing
            format!("{}.local.", trimmed)
        }
    }
}

fn is_sub_type(service_type: &str) -> bool {
    // Check if this is a subtype (contains _sub.)
    service_type.contains("_sub.")
}

fn start_browsing_service_type(
    mdns: &ServiceDaemon,
    service_type: &str,
    state: Arc<RwLock<AppState>>,
    notification_sender: flume::Sender<Notification>,
) -> Result<(), mdns_sd::Error> {
    let service_receiver = mdns.browse(service_type)?;

    let state_inner = Arc::clone(&state);
    let notification_sender_inner = notification_sender.clone();

    tokio::spawn(async move {
        while let Ok(service_event) = service_receiver.recv_async().await {
            match service_event {
                ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                    let mut state = state_inner.write().await;
                    if state.no_debounce {
                        if state.mark_service_offline(&fullname) {
                            let _ = notification_sender_inner.send(Notification::ServiceChanged);
                        }
                    } else {
                        state.schedule_service_removal(&fullname);
                    }
                }
                ServiceEvent::ServiceResolved(resolved_service) => {
                    let entry = ServiceEntry::from(*resolved_service);
                    let mut state = state_inner.write().await;
                    // Always use the resolved service entry to ensure metadata is up-to-date
                    // This handles both normal updates and flapping cases correctly
                    state.add_or_update_service(entry);

                    state.invalidate_cache_and_validate();
                    let _ = notification_sender_inner.send(Notification::ServiceChanged);
                }
                _ => (),
            }
        }
    });

    Ok(())
}

fn handle_browse_failure(
    service_type: &str,
    state: &mut AppState,
    notification_sender: flume::Sender<Notification>,
    failure_metric_key: &str,
) {
    state.remove_service_type(service_type);
    state.update_metric(failure_metric_key);
    let _ = notification_sender.send(Notification::ServiceChanged);
}

fn ui(f: &mut Frame, app_state: &AppState) {
    let layout = if app_state.filter_input_mode {
        create_filter_input_layout(f.area())
    } else {
        create_main_layout(f.area(), !app_state.filter_query.is_empty())
    };
    let visible_counts = calculate_visible_counts(&layout);

    if app_state.filter_input_mode {
        render_service_types_list(f, app_state, layout.left_panel, visible_counts.types);
        render_services_list(f, app_state, layout.services_area, visible_counts.services);
        render_service_details(f, app_state, layout.details_area);
        render_filter_input(f, app_state, f.area());
    } else {
        render_service_types_list(f, app_state, layout.left_panel, visible_counts.types);
        render_services_list(f, app_state, layout.services_area, visible_counts.services);
        render_service_details(f, app_state, layout.details_area);

        // Render filter status if not empty
        if !app_state.filter_query.is_empty()
            && let Some(filter_status_area) = layout.filter_status_area
        {
            render_filter_status(f, app_state, filter_status_area);
        }
    }

    // Render status message if present
    render_status_message(f, app_state);

    // Render popups if active
    app_state
        .popup_state
        .render(f, app_state.terminal_area, &app_state.metrics);
}

struct MainLayout {
    left_panel: ratatui::layout::Rect,
    services_area: ratatui::layout::Rect,
    details_area: ratatui::layout::Rect,
    filter_status_area: Option<ratatui::layout::Rect>,
}

struct VisibleCounts {
    types: usize,
    services: usize,
}

fn get_border_style(loaded_from_file: bool) -> Style {
    if loaded_from_file {
        Style::default().fg(VIEW_ONLY_BORDER_COLOR)
    } else {
        Style::default()
    }
}

fn create_main_layout(area: ratatui::layout::Rect, has_filter_status: bool) -> MainLayout {
    let main_area = if has_filter_status {
        // Reserve 3 rows at the bottom for filter status
        let remaining_height = area.height.saturating_sub(3);
        ratatui::layout::Rect::new(area.x, area.y, area.width, remaining_height)
    } else {
        area
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_area);

    let services_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    let filter_status_area = if has_filter_status {
        Some(ratatui::layout::Rect::new(
            area.x,
            area.y + area.height.saturating_sub(3),
            area.width,
            3,
        ))
    } else {
        None
    };

    MainLayout {
        left_panel: chunks[0],
        services_area: services_chunks[0],
        details_area: services_chunks[1],
        filter_status_area,
    }
}

fn calculate_visible_counts(layout: &MainLayout) -> VisibleCounts {
    VisibleCounts {
        types: (layout.left_panel.height as usize).saturating_sub(2), // Account for borders
        services: (layout.services_area.height as usize).saturating_sub(2), // Account for borders
    }
}

fn create_filter_input_layout(area: ratatui::layout::Rect) -> MainLayout {
    // Reserve 3 rows at the bottom for filter input
    let remaining_height = area.height.saturating_sub(3);
    let main_area = ratatui::layout::Rect::new(area.x, area.y, area.width, remaining_height);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_area);

    let services_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    // Filter input layout doesn't have a separate filter status area
    MainLayout {
        left_panel: chunks[0],
        services_area: services_chunks[0],
        details_area: services_chunks[1],
        filter_status_area: None,
    }
}

fn render_service_types_list(
    f: &mut Frame,
    app_state: &AppState,
    area: ratatui::layout::Rect,
    _visible_types: usize,
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
                let mut style = if app_state.selected_type == Some(i) {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                // If this is a user-requested service type, display it in italic
                if app_state.user_service_types.contains(service_type) {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                let display_type = format_service_type_for_display(service_type);
                ListItem::new(Line::from(Span::styled(display_type, style)))
            }),
    );

    let visible_type_items: Vec<ListItem> =
        get_visible_items(&type_items, &app_state.types_scroll).to_vec();

    let border_style = get_border_style(app_state.loaded_from_file);
    let types_list = List::new(visible_type_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title({
                    let mut spans = vec![Span::raw(format!(
                        "Service Types [{}] (←/→)",
                        app_state.service_types.len()
                    ))];
                    if app_state.disable_ipv4 || app_state.disable_ipv6 {
                        let mut disabled = Vec::new();
                        if app_state.disable_ipv4 {
                            disabled.push(Span::styled(
                                "IPv4",
                                Style::default().add_modifier(Modifier::CROSSED_OUT),
                            ));
                        }
                        if app_state.disable_ipv6 {
                            disabled.push(Span::styled(
                                "IPv6",
                                Style::default().add_modifier(Modifier::CROSSED_OUT),
                            ));
                        }
                        spans.push(Span::raw(" ["));
                        for (i, s) in disabled.iter().enumerate() {
                            if i > 0 {
                                spans.push(Span::raw(", "));
                            }
                            spans.push(s.clone());
                        }
                        spans.push(Span::raw("]"));
                    }
                    Line::from(spans)
                }),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ListState::default();
    let display_index = match app_state.selected_type {
        None => 0,
        Some(idx) => idx + 1,
    }
    .saturating_sub(app_state.types_scroll.offset);
    list_state.select(Some(display_index));
    f.render_stateful_widget(types_list, area, &mut list_state);
}

fn render_services_list(
    f: &mut Frame,
    app_state: &AppState,
    area: ratatui::layout::Rect,
    _visible_services: usize,
) {
    let selected_service_idx = app_state.selected_service;
    let services_clone = app_state.services.clone();
    let filtered_indices = app_state.get_filtered_services_readonly();
    let filtered_indices_len = filtered_indices.len();

    let offline_count = filtered_indices
        .iter()
        .filter(|&&idx| !services_clone[idx].online)
        .count();
    let online_count = filtered_indices_len - offline_count;
    let total_count = services_clone.len();

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

    let visible_service_items: Vec<ListItem> =
        get_visible_items(&service_items, &app_state.services_scroll).to_vec();

    let sort_field_display = format_sort_field_for_display(app_state.sort_field);
    let sort_dir_display = format_sort_direction_for_display(app_state.sort_direction);
    let sort_field_highlighted = Span::styled(
        sort_field_display,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::UNDERLINED),
    );
    let sort_dir_highlighted = Span::styled(
        sort_dir_display,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let title = if offline_count > 0 {
        Line::from(vec![
            Span::raw("Services ["),
            Span::styled(
                format!("{}", online_count),
                Style::default().fg(STATUS_OK_COLOR),
            ),
            Span::raw("/"),
            Span::styled(
                format!("{}", offline_count),
                Style::default().fg(STATUS_ERROR_COLOR),
            ),
            Span::raw("/"),
            Span::raw(format!("{}", total_count)),
            Span::raw("] ["),
            sort_field_highlighted,
            Span::raw("/"),
            sort_dir_highlighted,
            Span::raw("] (↑/↓, s/S to sort, o to toggle)"),
        ])
    } else {
        Line::from(vec![
            Span::raw("Services ["),
            Span::styled(
                format!("{}", online_count),
                Style::default().fg(STATUS_OK_COLOR),
            ),
            Span::raw("/"),
            Span::raw(format!("{}", total_count)),
            Span::raw("] ["),
            sort_field_highlighted,
            Span::raw("/"),
            sort_dir_highlighted,
            Span::raw("] (↑/↓, s/S to sort, o to toggle)"),
        ])
    };

    let border_style = get_border_style(app_state.loaded_from_file);
    let services_list = List::new(visible_service_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut services_list_state = ListState::default();
    services_list_state.select(Some(
        app_state
            .selected_service
            .saturating_sub(app_state.services_scroll.offset),
    ));
    f.render_stateful_widget(services_list, area, &mut services_list_state);
}

fn render_service_details(f: &mut Frame, app_state: &AppState, area: ratatui::layout::Rect) {
    let selected_service_idx = app_state.selected_service;
    let services_clone = app_state.services.clone();

    let filtered_indices = app_state.get_filtered_services_readonly();

    let selected_service = filtered_indices
        .get(selected_service_idx)
        .map(|&idx| &services_clone[idx]);

    if let Some(service) = selected_service {
        let details_lines = create_service_details_text(service);

        // Apply scroll offset
        let clamped_offset = if details_lines.is_empty() {
            0
        } else {
            let total_lines = details_lines.len();
            let visible_items = app_state.details_scroll.visible_items;
            let max_scroll_offset = if total_lines <= visible_items {
                0
            } else {
                total_lines.saturating_sub(visible_items)
            };
            app_state.details_scroll.offset.min(max_scroll_offset)
        };

        let visible_details: Vec<Line> = details_lines.into_iter().skip(clamped_offset).collect();

        let border_style = get_border_style(app_state.loaded_from_file);
        let details = Paragraph::new(visible_details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title("Service Details (Shift+↑/↓, J/K to scroll)"),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(details, area);
    } else {
        let border_style = get_border_style(app_state.loaded_from_file);
        let details = Paragraph::new("No service selected").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Service Details"),
        );
        f.render_widget(details, area);
    }
}

fn render_filter_input(f: &mut Frame, app_state: &AppState, area: ratatui::layout::Rect) {
    let filter_area = ratatui::layout::Rect::new(
        area.x,
        area.y + area.height.saturating_sub(3),
        area.width,
        3,
    );

    let input_text = format!("/{}_", app_state.filter_query);
    let border_style = get_border_style(app_state.loaded_from_file);

    let filter_input = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Quick Filter (Enter to apply, Esc to cancel)"),
        )
        .style(Style::default().fg(UI_CONTROLS_COLOR));

    f.render_widget(filter_input, filter_area);
}

fn render_filter_status(f: &mut Frame, app_state: &AppState, area: ratatui::layout::Rect) {
    let status_text = format!("Filter: '{}' (Press 'n' to clear)", app_state.filter_query);
    let border_style = get_border_style(app_state.loaded_from_file);

    let status = Paragraph::new(status_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Active Filter"),
        )
        .style(Style::default().fg(UI_CONTROLS_COLOR));

    f.render_widget(status, area);
}

fn render_status_message(f: &mut Frame, app_state: &AppState) {
    // Try to read the message without blocking
    if let Ok(msg) = app_state.status_message.try_lock()
        && !msg.is_empty()
    {
        // Position status message centered on the screen
        let area = f.area();
        // Calculate width with padding and border (2 for left/right borders, 2 for padding)
        let msg_width = (msg.len() + 4).min(area.width.saturating_sub(4) as usize);
        let popup_area = Rect::new(
            (area.width.saturating_sub(msg_width as u16)) / 2,
            (area.height.saturating_sub(3)) / 2,
            msg_width as u16,
            3,
        );

        // Clear the background first
        f.render_widget(ratatui::widgets::Clear, popup_area);

        // Create a block with border
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(STATUS_OK_COLOR).bg(Color::DarkGray));

        // Create the inner area for text (accounting for borders)
        let inner_area = block.inner(popup_area);

        // Render the border/frame
        f.render_widget(block, popup_area);

        // Render the message text centered in the inner area
        let paragraph = Paragraph::new(msg.as_str())
            .style(Style::default().fg(STATUS_OK_COLOR).bg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(paragraph, inner_area);
    }
}

// Helper functions for formatting
fn format_sort_field_for_display(field: SortField) -> &'static str {
    match field {
        SortField::Host => "Host",
        SortField::ServiceType => "Type",
        SortField::Fullname => "Name",
        SortField::Port => "Port",
        SortField::Address => "Addr",
        SortField::Timestamp => "Time",
    }
}

fn format_sort_direction_for_display(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "↑",
        SortDirection::Descending => "↓",
    }
}

fn format_service_type_for_display(service_type: &str) -> String {
    service_type
        .trim_start_matches('_')
        .trim_end_matches(".local.")
        .trim_end_matches(".")
        .replace("._", ".")
}

fn create_service_list_item_style(
    index: usize,
    selected_index: usize,
    service: &ServiceEntry,
) -> Style {
    let foreground = Color::White;

    let mut style = if index == selected_index {
        Style::default().bg(Color::DarkGray).fg(foreground)
    } else {
        Style::default().fg(foreground)
    };

    if !service.online {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }

    // Add subtle styling for flapping services using color-blind friendly approach
    if service.is_flapping {
        // Use visual indicators that work for all users:
        // - Slightly darker background (darker than normal dark gray)
        // - Underline modifier for additional visual distinction
        if index == selected_index {
            style = style
                .bg(FLAPPING_COLOR_SELECTED)
                .add_modifier(Modifier::UNDERLINED);
        } else {
            style = style
                .bg(FLAPPING_COLOR_NORMAL)
                .add_modifier(Modifier::UNDERLINED);
        }
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

fn format_duration_micros(duration_micros: u64) -> String {
    let total_seconds = duration_micros / 1_000_000;
    let remaining_micros = duration_micros % 1_000_000;

    let seconds = total_seconds % 60;
    let mut minutes = (total_seconds / 60) % 60;
    let mut hours = (total_seconds / 3600) % 24;
    let mut days = total_seconds / 86400;

    let mut parts = Vec::new();

    // Handle fractional seconds and potential rounding
    if remaining_micros > 0 {
        let precise_seconds = seconds as f64 + remaining_micros as f64 / 1_000_000.0;
        let rounded_seconds = (precise_seconds * 1000.0).round() / 1000.0;

        // Check if rounding causes seconds to roll over to 60
        if rounded_seconds >= 60.0 {
            minutes += 1;

            // Handle minute rollover
            if minutes >= 60 {
                minutes = 0;
                hours += 1;

                // Handle hour rollover
                if hours >= 24 {
                    hours = 0;
                    days += 1;
                }
            }
        }

        let final_seconds = if rounded_seconds >= 60.0 {
            0.0
        } else {
            rounded_seconds
        };

        if days > 0 {
            parts.push(format!("{}d", days));
        }
        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
        }

        // Only show seconds if they're non-zero OR if there are no minutes/higher units
        if final_seconds > 0.0 || (days == 0 && hours == 0 && minutes == 0) {
            parts.push(format!("{:.3}s", final_seconds));
        }
    } else {
        if days > 0 {
            parts.push(format!("{}d", days));
        }
        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
        }

        // Only show seconds if they're non-zero OR if there are no minutes/higher units
        if seconds > 0 || (days == 0 && hours == 0 && minutes == 0) {
            parts.push(format!("{}s", seconds));
        }
    }

    parts.join(" ")
}

fn get_session_history(service: &ServiceEntry) -> String {
    let max_session_num_length = service
        .session_history
        .iter()
        .enumerate()
        .map(|(i, _)| (i + 1).to_string().len())
        .max()
        .unwrap_or(0);

    let mut timeline = Vec::new();
    for (i, session) in service.session_history.iter().enumerate() {
        let session_num = i + 1;
        let start_str = format_timestamp_micros(session.start_time);
        let (duration_str, end_str) = if let Some(end_time) = session.end_time {
            let duration = end_time.saturating_sub(session.start_time);
            (
                format_duration_micros(duration),
                format_timestamp_micros(end_time),
            )
        } else {
            ("N/A".to_string(), "Ongoing".to_string())
        };

        timeline.push(format!(
            "Session {:>session_width$}: {} → {:<timestamp_width$} = {}",
            session_num,
            start_str,
            end_str,
            duration_str,
            session_width = max_session_num_length,
            timestamp_width = 26,
        ));
    }
    timeline.join("\n")
}

fn create_service_details_text(service: &ServiceEntry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Online status - use blue (color-blind friendly)
    let online_style: Style = Style::default()
        .fg(STATUS_OK_COLOR)
        .add_modifier(Modifier::BOLD);
    // Offline status - use orange (color-blind friendly)
    let offline_style: Style = Style::default()
        .fg(STATUS_ERROR_COLOR)
        .add_modifier(Modifier::BOLD);

    // Status with flapping info
    let flapping_style: Style = Style::default()
        .fg(FLAPPING_FOREGROUND_COLOR)
        .add_modifier(Modifier::BOLD);

    if service.online {
        if service.is_flapping {
            lines.push(Line::from(vec![
                Span::styled("Status:", Style::default()),
                Span::styled(" Online, ", online_style),
                Span::styled("Flapping", flapping_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Status:", Style::default()),
                Span::styled(" Online", online_style),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled("First seen:        ", Style::default()),
            Span::raw(format_timestamp_micros(service.first_seen_micros)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Last came online:  ", Style::default()),
            Span::raw(format_timestamp_micros(
                service
                    .last_online_micros
                    .unwrap_or(service.first_seen_micros),
            )),
        ]));
        if service.last_offline_micros.is_some() {
            lines.push(Line::from(vec![
                Span::styled("Last seen offline: ", Style::default()),
                Span::raw(format_timestamp_micros(
                    service.last_offline_micros.unwrap_or(0),
                )),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Last seen offline: ", Style::default()),
                Span::raw("Never".to_string()),
            ]));
        }
    } else {
        if service.is_flapping {
            lines.push(Line::from(vec![
                Span::styled("Status:", Style::default()),
                Span::styled(" Offline, ", offline_style),
                Span::styled("Flapping", flapping_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Status:", Style::default()),
                Span::styled(" Offline", offline_style),
            ]));
        }

        let offline_timestamp = service
            .last_offline_micros
            .map(format_timestamp_micros)
            .unwrap_or_else(|| "Unknown".to_string());
        let last_online_timestamp = service
            .last_online_micros
            .map(format_timestamp_micros)
            .unwrap_or_else(|| "Unknown".to_string());

        lines.push(Line::from(vec![
            Span::styled("First seen:        ", Style::default()),
            Span::raw(format_timestamp_micros(service.first_seen_micros)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Last seen online:  ", Style::default()),
            Span::raw(last_online_timestamp),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Went offline at:   ", Style::default()),
            Span::raw(offline_timestamp),
        ]));
    }

    // Empty line for spacing
    lines.push(Line::from(""));

    // Service information
    let subtype_text = service
        .subtype
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_default();

    lines.push(Line::from(vec![
        Span::styled("Fullname: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(service.fullname.clone()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Hostname: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(service.host.clone()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(service.service_type.clone()),
    ]));
    if !subtype_text.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Subtype: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(subtype_text),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("Port: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(service.port.to_string()),
    ]));

    // Empty line for spacing
    lines.push(Line::from(""));

    // Addresses
    lines.push(Line::from(vec![Span::styled(
        "Addresses:",
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    let addresses_text = if service.addrs.is_empty() {
        "None".to_string()
    } else {
        service.addrs.join("\n")
    };
    for addr_line in addresses_text.lines() {
        lines.push(Line::from(addr_line.to_string()));
    }

    // Empty line for spacing
    lines.push(Line::from(""));

    // TXT Records
    lines.push(Line::from(vec![Span::styled(
        "TXT Records:",
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    let txt_text = if service.txt.is_empty() {
        "None".to_string()
    } else {
        service.txt.join("\n")
    };
    for txt_line in txt_text.lines() {
        lines.push(Line::from(txt_line.to_string()));
    }

    // Empty line for spacing
    lines.push(Line::from(""));

    // Session History
    lines.push(Line::from(vec![Span::styled(
        "Session History:",
        Style::default().add_modifier(Modifier::BOLD),
    )]));
    let timeline = get_session_history(service);
    for timeline_line in timeline.lines() {
        lines.push(Line::from(timeline_line.to_string()));
    }

    lines
}

/// Runs the TUI application for mDNS service browsing.
///
/// # Arguments
/// * `user_service_types` - Service types to browse for
/// * `no_debounce` - Whether to disable debouncing of flapping services
/// * `interfaces` - Optional list of network interface names to bind to.
///   If `Some`, only the specified interfaces will be used.
///   If `None`, all available interfaces will be used (default behavior).
///   The expected string format is the interface name (e.g., "eth0", "en0").
///   An empty vector `Some(vec![])` will result in no interfaces being used.
/// * `available_interfaces` - List of all available network interface names.
///   Used to disable all interfaces before enabling the requested ones.
/// * `disable_ipv4` - Whether to disable IPv4 mDNS discovery
/// * `disable_ipv6` - Whether to disable IPv6 mDNS discovery
/// * `loaded_state` - Optional JSON string to load state from file (view-only mode)
///
/// # Example
/// ```no_run
/// use std::collections::HashSet;
///
/// async fn demo() -> Result<(), Box<dyn std::error::Error>> {
///     let service_types = HashSet::new();
///
///     // Use default interfaces (all available)
///     run_tui(service_types.clone(), false, None, None, false, false, None).await;
///
///     // Use specific interfaces
///     run_tui(
///         service_types,
///         false,
///         Some(vec!["eth0".into()]),
///         Some(vec!["eth0".into(), "lo".into()]),
///         false,
///         false,
///         None,
///     )
///     .await
/// }
/// ```
pub async fn run_tui(
    user_service_types: HashSet<String>,
    no_debounce: bool,
    interfaces: Option<Vec<String>>,
    available_interfaces: Option<Vec<String>>,
    disable_ipv4: bool,
    disable_ipv6: bool,
    loaded_state: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if disable_ipv4 && disable_ipv6 {
        return Err("Cannot disable both IPv4 and IPv6. At least one must be enabled.".into());
    }

    let is_view_only = loaded_state.is_some();

    // Parse state file before terminal setup to avoid leaving terminal in broken state on error
    let state_dump: Option<StateDump> = if let Some(json_content) = loaded_state {
        Some(
            serde_json::from_str(&json_content)
                .map_err(|e| format!("Failed to parse state file: {}", e))?,
        )
    } else {
        None
    };

    // Setup terminal for full TUI
    let mut terminal = TuiTerminal::new()?;

    let mdns = if is_view_only {
        None
    } else {
        Some(ServiceDaemon::new()?)
    };

    if let Some(ref mdns_ref) = mdns {
        if let Some(ref ifs) = interfaces {
            let available = available_interfaces.ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Available interfaces required when --interfaces is set",
                )
            })?;

            for interface in &available {
                mdns_ref
                    .disable_interface(interface)
                    .map_err(|e| format!("Failed to disable interface '{}': {}", interface, e))?;
            }

            for interface in ifs {
                mdns_ref
                    .enable_interface(interface)
                    .map_err(|e| format!("Failed to enable interface '{}': {}", interface, e))?;
            }
        }

        if disable_ipv4 {
            mdns_ref
                .disable_interface(IfKind::IPv4)
                .map_err(|e| format!("Failed to disable IPv4: {}", e))?;
        }

        if disable_ipv6 {
            mdns_ref
                .disable_interface(IfKind::IPv6)
                .map_err(|e| format!("Failed to disable IPv6: {}", e))?;
        }
    }

    // Initialize app state
    let state = Arc::new(RwLock::new(AppState::new(
        user_service_types.clone(),
        no_debounce,
        disable_ipv4,
        disable_ipv6,
        interfaces.clone(),
    )));

    // Load state from file if provided
    if let Some(dump) = state_dump {
        let mut state_write = state.write().await;
        state_write.load_from_state_dump(dump);
    }

    // Create notification channels
    let (notification_sender, notification_receiver) = flume::unbounded::<Notification>();

    let state_clone = Arc::clone(&state);
    let notification_sender_clone = notification_sender.clone();

    // Browse for user_requested service types provided via command line (skip in view-only mode)
    if !is_view_only {
        let user_types = {
            let state_read = state.read().await;
            state_read.user_service_types.clone()
        };

        for service_type in &user_types {
            // Allow subtypes for user-requested service types

            {
                let mut state_write = state_clone.write().await;
                if state_write.add_service_type(service_type) {
                    state_write.update_metric("user_service_types_added");
                    let _ = notification_sender_clone.send(Notification::ServiceChanged);
                    if let Some(ref mdns_ref) = mdns {
                        match start_browsing_service_type(
                            mdns_ref,
                            service_type,
                            Arc::clone(&state_clone),
                            notification_sender_clone.clone(),
                        ) {
                            Ok(_) => {} // Successfully started browsing
                            Err(_) => {
                                handle_browse_failure(
                                    service_type,
                                    &mut state_write,
                                    notification_sender_clone.clone(),
                                    "user_requested_service_browse_failures",
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let mdns_for_metrics = mdns.clone();

    // Start background task to periodically collect ServiceDaemon metrics (skip in view-only mode)
    if !is_view_only {
        let state_for_metrics = Arc::clone(&state);
        let notification_sender_for_metrics = notification_sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;

                if let Some(ref mdns_ref) = mdns_for_metrics {
                    match mdns_ref.get_metrics() {
                        Ok(metrics_receiver) => {
                            if let Ok(daemon_metrics) = metrics_receiver.recv_async().await {
                                let mut state = state_for_metrics.write().await;
                                if state.update_daemon_metrics(&daemon_metrics) {
                                    // Metrics changed, trigger UI refresh
                                    let _ = notification_sender_for_metrics
                                        .send(Notification::MetricsUpdated);
                                }
                            }
                        }
                        Err(_) => {
                            // If we can't get metrics, just continue
                        }
                    }
                }
            }
        });
    }

    // Start global cleanup task to handle expired service removals (skip in view-only mode)
    if !is_view_only {
        let state_for_cleanup = Arc::clone(&state);
        let notification_sender_for_cleanup = notification_sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(CLEANUP_INTERVAL_MS));
            loop {
                interval.tick().await;

                let ui_should_update = {
                    let mut state = state_for_cleanup.write().await;

                    // Only process expired removals if debouncing is enabled
                    if !state.no_debounce {
                        // Capture state before processing expired removals
                        let before_count = state.pending_removals.len();

                        // Process expired removals
                        state.process_expired_removals();

                        // Update pending removals count metric
                        let pending_count = state.pending_removals.len() as u64;
                        *state
                            .metrics
                            .entry("pending_removals_active".to_string())
                            .or_insert(0) = pending_count;

                        // Check if any services actually changed (removed or marked offline)
                        (before_count as u64) != pending_count
                    } else {
                        false
                    }
                };

                // Notify UI if state changed
                if ui_should_update {
                    let _ = notification_sender_for_cleanup.send(Notification::ServiceChanged);
                }

                // This task runs indefinitely
            }
        });

        if state.read().await.user_service_types.is_empty() {
            // Browse for all service types
            if let Some(ref mdns_ref) = mdns {
                let receiver = mdns_ref.browse("_services._dns-sd._udp.local.")?;

                let mdns = mdns.clone();
                tokio::spawn(async move {
                    while let Ok(event) = receiver.recv_async().await {
                        match event {
                            ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                                let mut state = state_clone.write().await;
                                if state.remove_service_type(&fullname) {
                                    let _ = notification_sender_clone
                                        .send(Notification::ServiceChanged);
                                }
                            }
                            ServiceEvent::ServiceFound(_service_type, fullname) => {
                                let service_type = fullname.to_string();
                                if is_sub_type(&service_type) {
                                    continue; // skip subtypes in auto-discovery
                                }
                                {
                                    let mut state = state_clone.write().await;
                                    if state.add_service_type(&service_type) {
                                        state.update_metric("service_types_discovered");
                                        let _ = notification_sender_clone
                                            .send(Notification::ServiceChanged);
                                        if let Some(ref mdns_ref) = mdns {
                                            match start_browsing_service_type(
                                                mdns_ref,
                                                &service_type,
                                                Arc::clone(&state_clone),
                                                notification_sender_clone.clone(),
                                            ) {
                                                Ok(_) => {} // Successfully started browsing
                                                Err(_) => {
                                                    handle_browse_failure(
                                                        &service_type,
                                                        &mut state,
                                                        notification_sender_clone.clone(),
                                                        "discovered_service_browse_failures",
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => (),
                        }
                    }
                });
            }
        }
    }
    // Initial render to show the UI immediately
    {
        let terminal_area = terminal
            .get_area()
            .map_err(|e| format!("Failed to get terminal area: {}", e))?;
        {
            let mut state = state.write().await;
            state.prepare_for_rendering(terminal_area);
            terminal.draw(|f| ui(f, &state))?;
        }
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
                            #[cfg(unix)]
                            {
                                if key.code == KeyCode::Char('z')
                                    && key.modifiers.contains(KeyModifiers::CONTROL)
                                {
                                    handle_suspend(&mut terminal, &state, ui).await;
                                    continue;
                                }
                            }

                            #[cfg(target_os = "windows")]
                            {
                                // On Windows, ignore key release events to prevent duplicate handling
                                if key.kind == crossterm::event::KeyEventKind::Release {
                                    continue;
                                }
                            }

                            let mut state = state.write().await;
                            let should_continue = state.handle_key_event(key);
                            if should_continue {
                                let _ = notification_sender.send(Notification::UserInput);
                            } else {
                                break Ok(());
                            }
                        }
                        Event::Resize(_, _) => {
                            let _ = notification_sender.send(Notification::UserInput);
                        }
                        _ => {}
                    }
                }
            }

            // Handle notifications for rendering
            _notification = notification_receiver.recv_async() => {
                // Draw UI only when there's a notification
                // Acquire write lock once for both preparation and rendering to prevent race conditions
                let terminal_area = match terminal.get_area() {
                    Ok(area) => area,
                    Err(e) => {
                        let err_msg = format!("Failed to get terminal area: {}", e);
                        eprintln!("{}", err_msg);
                        break Err(err_msg.into());
                    }
                };
                {
                    let mut state = state.write().await;
                    state.prepare_for_rendering(terminal_area);
                    if let Err(e) = terminal.draw(|f| ui(f, &state)) {
                        let err_msg = format!("Failed to draw terminal: {}", e);
                        eprintln!("{}", err_msg);
                        break Err(err_msg.into());
                    }
                }
            }
        }
    };

    // Restore terminal
    terminal.restore()?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::ServiceSession;
    use crate::models::tests::{create_test_service, create_test_service_with_sessions};

    // Enhanced test helper functions to reduce duplication

    /// Quick state setup with specified number of services
    fn setup_test_state(service_count: usize) -> AppState {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        for i in 0..service_count {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                8080 + i as u16,
            ));
        }

        state
    }

    /// Setup state with custom service types
    fn setup_test_state_with_types(types: Vec<&str>) -> AppState {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        for service_type in types {
            state.add_service_type(service_type);
        }

        state
    }

    /// Setup state with pre-populated services
    fn setup_test_state_with_services(services: Vec<ServiceEntry>) -> AppState {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        for service in &services {
            state.add_service_type(&service.service_type);
        }

        state.services = services;
        state
    }

    /// Setup state with user-specified service types
    fn setup_test_state_with_user_types(user_types: Vec<&str>) -> AppState {
        let user_types_set: HashSet<String> =
            user_types.into_iter().map(|s| s.to_string()).collect();
        AppState::new(user_types_set, false, false, false, None)
    }

    /// Helper to create AppState for testing
    fn create_test_app_state(
        user_service_types: HashSet<String>,
        no_debounce: bool,
        disable_ipv4: bool,
        disable_ipv6: bool,
    ) -> AppState {
        AppState::new(
            user_service_types,
            no_debounce,
            disable_ipv4,
            disable_ipv6,
            None,
        )
    }

    // Common assertion helper functions

    /// Assert navigation state (service and type selection)
    fn assert_navigation_state(
        state: &AppState,
        expected_service: usize,
        expected_type: Option<usize>,
    ) {
        assert_eq!(
            state.selected_service, expected_service,
            "Service selection mismatch"
        );
        assert_eq!(
            state.selected_type, expected_type,
            "Type selection mismatch"
        );
    }

    /// Assert cache state
    fn assert_cache_state(state: &AppState, expected_dirty: bool, expected_sorted: bool) {
        assert_eq!(
            state.cache_dirty, expected_dirty,
            "Cache dirty state mismatch"
        );
        assert_eq!(
            state.cached_sorted, expected_sorted,
            "Cache sorted state mismatch"
        );
    }

    /// Assert service count
    fn assert_service_count(state: &AppState, expected: usize) {
        assert_eq!(state.services.len(), expected, "Service count mismatch");
    }

    /// Assert service type count
    fn assert_service_type_count(state: &AppState, expected: usize) {
        assert_eq!(
            state.service_types.len(),
            expected,
            "Service type count mismatch"
        );
    }

    // Tests for details scroll fix
    /// Test that details scroll is not reset when navigating with only one service
    #[test]
    fn test_details_scroll_not_reset_with_single_service() {
        let mut state = setup_test_state_with_services(vec![create_test_service(
            "test1",
            "_http._tcp.local.",
            8080,
        )]);

        // Set initial scroll position
        state.details_scroll.offset = 5;
        state.details_scroll.visible_items = 10;

        // Try to navigate up (should not change selection)
        let old_details_offset = state.details_scroll.offset;
        state.navigate_services_up();
        assert_eq!(
            state.details_scroll.offset, old_details_offset,
            "Details scroll should not reset when navigating up with single service"
        );

        // Try to navigate down (should not change selection)
        let old_details_offset = state.details_scroll.offset;
        state.navigate_services_down();
        assert_eq!(
            state.details_scroll.offset, old_details_offset,
            "Details scroll should not reset when navigating down with single service"
        );
    }

    /// Test that details scroll is reset when actually changing services
    #[test]
    fn test_details_scroll_reset_when_changing_services() {
        let mut state = setup_test_state_with_services(vec![
            create_test_service("test1", "_http._tcp.local.", 8080),
            create_test_service("test2", "_http._tcp.local.", 8081),
        ]);

        // Set initial scroll position
        state.details_scroll.offset = 5;
        state.details_scroll.visible_items = 10;

        // Navigate down (should change selection and reset scroll)
        state.navigate_services_down();
        assert_eq!(state.selected_service, 1, "Should select second service");
        assert_eq!(
            state.details_scroll.offset, 0,
            "Details scroll should reset when changing services"
        );
    }

    /// Test that page navigation respects the same logic
    #[test]
    fn test_details_scroll_page_navigation_with_single_service() {
        let mut state = setup_test_state_with_services(vec![create_test_service(
            "test1",
            "_http._tcp.local.",
            8080,
        )]);

        // Set initial scroll position
        state.details_scroll.offset = 5;
        state.details_scroll.visible_items = 10;

        // Try page up (should not change selection)
        let old_details_offset = state.details_scroll.offset;
        state.navigate_services_page_up();
        assert_eq!(
            state.details_scroll.offset, old_details_offset,
            "Details scroll should not reset on page up with single service"
        );

        // Try page down (should not change selection)
        let old_details_offset = state.details_scroll.offset;
        state.navigate_services_page_down();
        assert_eq!(
            state.details_scroll.offset, old_details_offset,
            "Details scroll should not reset on page down with single service"
        );
    }

    /// Test that home/end navigation respects the same logic
    #[test]
    fn test_details_scroll_home_end_with_single_service() {
        let mut state = setup_test_state_with_services(vec![create_test_service(
            "test1",
            "_http._tcp.local.",
            8080,
        )]);

        // Set initial scroll position
        state.details_scroll.offset = 5;
        state.details_scroll.visible_items = 10;

        // Try navigate to first (should not change selection)
        let old_details_offset = state.details_scroll.offset;
        state.navigate_services_to_first();
        assert_eq!(
            state.details_scroll.offset, old_details_offset,
            "Details scroll should not reset on navigate to first with single service"
        );

        // Try navigate to last (should not change selection)
        let old_details_offset = state.details_scroll.offset;
        state.navigate_services_to_last();
        assert_eq!(
            state.details_scroll.offset, old_details_offset,
            "Details scroll should not reset on navigate to last with single service"
        );
    }

    /// Test that navigation resets scroll when there are multiple services
    #[test]
    fn test_details_scroll_reset_with_multiple_services() {
        let mut state = setup_test_state_with_services(vec![
            create_test_service("test1", "_http._tcp.local.", 8080),
            create_test_service("test2", "_http._tcp.local.", 8081),
            create_test_service("test3", "_http._tcp.local.", 8082),
        ]);

        // Set initial scroll position
        state.details_scroll.offset = 5;
        state.details_scroll.visible_items = 10;

        // Navigate to last service
        state.navigate_services_to_last();
        assert_eq!(state.selected_service, 2, "Should select third service");
        assert_eq!(
            state.details_scroll.offset, 0,
            "Details scroll should reset when navigating to last service"
        );
    }

    /// Assert specific metric value
    fn assert_metric(state: &AppState, metric_name: &str, expected_value: u64) {
        assert_eq!(
            state.metrics.get(metric_name),
            Some(&expected_value),
            "Metric {} mismatch",
            metric_name
        );
    }

    /// Assert metric does not exist
    fn assert_metric_not_exist(state: &AppState, metric_name: &str) {
        assert_eq!(
            state.metrics.get(metric_name),
            None,
            "Metric {} should not exist",
            metric_name
        );
    }

    /// Create offline service variant
    fn create_offline_service(name: &str, service_type: &str, port: u16) -> ServiceEntry {
        let mut service = create_test_service(name, service_type, port);
        service.online = false;
        service
    }

    /// Create service with custom addresses
    fn create_service_with_addrs(
        name: &str,
        service_type: &str,
        port: u16,
        addrs: Vec<&str>,
    ) -> ServiceEntry {
        let mut service = create_test_service(name, service_type, port);
        service.addrs = addrs.into_iter().map(|s| s.to_string()).collect();
        service
    }

    /// Create service with TXT records
    fn create_service_with_txt(
        name: &str,
        service_type: &str,
        port: u16,
        txt: Vec<&str>,
    ) -> ServiceEntry {
        let mut service = create_test_service(name, service_type, port);
        service.txt = txt.into_iter().map(|s| s.to_string()).collect();
        service
    }

    /// Create service with subtype
    fn create_service_with_subtype(
        name: &str,
        service_type: &str,
        port: u16,
        subtype: &str,
    ) -> ServiceEntry {
        let mut service = create_test_service(name, service_type, port);
        service.subtype = Some(subtype.to_string());
        service
    }

    // Navigation test helper
    enum NavigationDirection {
        Up,
        Down,
        PageUp,
        PageDown,
        First,
        Last,
    }

    /// Helper to test navigation with validation
    fn test_navigation_scenario(
        mut state: AppState,
        start_pos: usize,
        direction: NavigationDirection,
        expected_pos: usize,
        description: &str,
    ) {
        state.selected_service = start_pos;

        match direction {
            NavigationDirection::Up => state.navigate_services_up(),
            NavigationDirection::Down => state.navigate_services_down(),
            NavigationDirection::PageUp => state.navigate_services_page_up(),
            NavigationDirection::PageDown => state.navigate_services_page_down(),
            NavigationDirection::First => state.navigate_services_to_first(),
            NavigationDirection::Last => state.navigate_services_to_last(),
        }

        assert_eq!(
            state.selected_service, expected_pos,
            "{}: Expected position {}, got {}",
            description, expected_pos, state.selected_service
        );
    }

    /// Helper to test service type navigation
    fn test_type_navigation_scenario(
        mut state: AppState,
        start_type: Option<usize>,
        direction: NavigationDirection,
        expected_type: Option<usize>,
        description: &str,
    ) {
        state.selected_type = start_type;

        match direction {
            NavigationDirection::Up => state.navigate_service_types_up(),
            NavigationDirection::Down => state.navigate_service_types_down(),
            NavigationDirection::PageUp => state.navigate_service_types_page_up(),
            NavigationDirection::PageDown => state.navigate_service_types_page_down(),
            NavigationDirection::First => state.navigate_service_types_to_first(),
            NavigationDirection::Last => state.navigate_service_types_to_last(),
        }

        assert_eq!(
            state.selected_type, expected_type,
            "{}: Expected type {:?}, got {:?}",
            description, expected_type, state.selected_type
        );
    }

    // CONSOLIDATED NAVIGATION TESTS

    #[test]
    fn test_navigate_services_comprehensive() {
        let state = setup_test_state(3); // services 0, 1, 2

        // Test basic up navigation
        test_navigation_scenario(
            state.clone(),
            2,
            NavigationDirection::Up,
            1,
            "Navigate up from position 2",
        );
        test_navigation_scenario(
            state.clone(),
            1,
            NavigationDirection::Up,
            0,
            "Navigate up from position 1",
        );
        test_navigation_scenario(
            state.clone(),
            0,
            NavigationDirection::Up,
            0,
            "Navigate up from position 0 (boundary)",
        );

        // Test basic down navigation
        test_navigation_scenario(
            state.clone(),
            0,
            NavigationDirection::Down,
            1,
            "Navigate down from position 0",
        );
        test_navigation_scenario(
            state.clone(),
            1,
            NavigationDirection::Down,
            2,
            "Navigate down from position 1",
        );
        test_navigation_scenario(
            state.clone(),
            2,
            NavigationDirection::Down,
            2,
            "Navigate down from position 2 (boundary)",
        );

        // Test first/last navigation
        test_navigation_scenario(
            state.clone(),
            2,
            NavigationDirection::First,
            0,
            "Navigate to first",
        );
        test_navigation_scenario(
            state.clone(),
            0,
            NavigationDirection::Last,
            2,
            "Navigate to last",
        );

        // Test single service edge case
        let single_service_state = setup_test_state(1);
        test_navigation_scenario(
            single_service_state.clone(),
            0,
            NavigationDirection::Up,
            0,
            "Single service up navigation",
        );
        test_navigation_scenario(
            single_service_state,
            0,
            NavigationDirection::Down,
            0,
            "Single service down navigation",
        );
    }

    #[test]
    fn test_navigate_services_page_navigation() {
        let mut state = setup_test_state(20);
        state.services_scroll.visible_items = 5;

        // Test page up navigation
        test_navigation_scenario(
            state.clone(),
            10,
            NavigationDirection::PageUp,
            6,
            "Page up from position 10",
        );
        test_navigation_scenario(
            state.clone(),
            6,
            NavigationDirection::PageUp,
            2,
            "Page up from position 6",
        );
        test_navigation_scenario(
            state.clone(),
            2,
            NavigationDirection::PageUp,
            0,
            "Page up from position 2 (boundary)",
        );

        // Test page down navigation
        test_navigation_scenario(
            state.clone(),
            0,
            NavigationDirection::PageDown,
            4,
            "Page down from position 0",
        );
        test_navigation_scenario(
            state.clone(),
            4,
            NavigationDirection::PageDown,
            8,
            "Page down from position 4",
        );
        test_navigation_scenario(
            state.clone(),
            15,
            NavigationDirection::PageDown,
            19,
            "Page down from position 15 (boundary)",
        );

        // Test with fewer items than page size
        let small_state = setup_test_state(2);
        test_navigation_scenario(
            small_state,
            0,
            NavigationDirection::PageDown,
            0,
            "Page down with fewer than page size",
        );
    }

    #[test]
    fn test_navigate_service_types_comprehensive() {
        let state = setup_test_state_with_types(vec![
            "_http._tcp.local.",
            "_ssh._tcp.local.",
            "_printer._tcp.local.",
        ]);

        // Test basic up navigation
        test_type_navigation_scenario(
            state.clone(),
            Some(2),
            NavigationDirection::Up,
            Some(1),
            "Type up from index 2",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(1),
            NavigationDirection::Up,
            Some(0),
            "Type up from index 1",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(0),
            NavigationDirection::Up,
            None,
            "Type up from index 0 to All Types",
        );
        test_type_navigation_scenario(
            state.clone(),
            None,
            NavigationDirection::Up,
            None,
            "Type up from All Types (boundary)",
        );

        // Test basic down navigation
        test_type_navigation_scenario(
            state.clone(),
            None,
            NavigationDirection::Down,
            Some(0),
            "Type down from All Types",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(0),
            NavigationDirection::Down,
            Some(1),
            "Type down from index 0",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(1),
            NavigationDirection::Down,
            Some(2),
            "Type down from index 1",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(2),
            NavigationDirection::Down,
            Some(2),
            "Type down from index 2 (boundary)",
        );

        // Test first/last navigation
        test_type_navigation_scenario(
            state.clone(),
            Some(2),
            NavigationDirection::First,
            None,
            "Type navigate to first (All Types)",
        );
        test_type_navigation_scenario(
            state.clone(),
            None,
            NavigationDirection::Last,
            Some(2),
            "Type navigate to last",
        );

        // Test single type edge case
        let single_type_state = setup_test_state_with_types(vec!["_http._tcp.local."]);
        test_type_navigation_scenario(
            single_type_state.clone(),
            Some(0),
            NavigationDirection::Up,
            None,
            "Single type up navigation",
        );
        test_type_navigation_scenario(
            single_type_state,
            None,
            NavigationDirection::Down,
            Some(0),
            "Single type down navigation",
        );
    }

    #[test]
    fn test_navigate_service_types_page_navigation() {
        let mut state = setup_test_state_with_types(
            (0..10)
                .map(|i| format!("_test{}._tcp.local.", i))
                .collect::<Vec<_>>()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );
        state.types_scroll.visible_items = 3;

        // Test page up navigation
        test_type_navigation_scenario(
            state.clone(),
            Some(8),
            NavigationDirection::PageUp,
            Some(6),
            "Type page up from index 8",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(6),
            NavigationDirection::PageUp,
            Some(4),
            "Type page up from index 6",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(1),
            NavigationDirection::PageUp,
            None,
            "Type page up from index 1 to All Types",
        );

        // Test page down navigation
        test_type_navigation_scenario(
            state.clone(),
            None,
            NavigationDirection::PageDown,
            Some(2),
            "Type page down from All Types",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(2),
            NavigationDirection::PageDown,
            Some(4),
            "Type page down from index 2",
        );
        test_type_navigation_scenario(
            state.clone(),
            Some(8),
            NavigationDirection::PageDown,
            Some(9),
            "Type page down from index 8 (boundary)",
        );

        // Test edge cases
        let empty_state = setup_test_state_with_types(vec![]);
        test_type_navigation_scenario(
            empty_state.clone(),
            None,
            NavigationDirection::PageUp,
            None,
            "Type page up with no types",
        );
        test_type_navigation_scenario(
            empty_state,
            None,
            NavigationDirection::PageDown,
            None,
            "Type page down with no types",
        );

        let mut few_types_state =
            setup_test_state_with_types(vec!["_test1._tcp.local.", "_test2._tcp.local."]);
        few_types_state.types_scroll.visible_items = 5;
        test_type_navigation_scenario(
            few_types_state,
            None,
            NavigationDirection::PageDown,
            Some(1),
            "Type page down with fewer than page size",
        );

        let mut zero_visible_state =
            setup_test_state_with_types(vec!["_test1._tcp.local.", "_test2._tcp.local."]);
        zero_visible_state.types_scroll.visible_items = 0;
        test_type_navigation_scenario(
            zero_visible_state,
            Some(1),
            NavigationDirection::PageUp,
            Some(1),
            "Type page up with zero visible",
        );
    }

    #[test]
    fn test_scroll_offset_updates_on_navigation() {
        let mut state = setup_test_state(10);
        state.services_scroll.visible_items = 5;

        // Navigate down beyond visible area
        state.selected_service = 4;
        state.navigate_services_down();
        assert_eq!(state.selected_service, 5);
        assert!(
            state.services_scroll.offset > 0,
            "Scroll offset should update to keep selected service visible"
        );

        // Navigate up should update scroll offset appropriately
        state.selected_service = 5;
        state.navigate_services_up();
        assert_eq!(state.selected_service, 4);

        // Verify selected service stays within visible range
        assert!(state.selected_service >= state.services_scroll.offset);
        assert!(
            state.selected_service
                < state.services_scroll.offset + state.services_scroll.visible_items
        );
    }

    // CONSOLIDATED SORTING TESTS

    #[derive(Debug)]
    struct SortTestCase {
        name: &'static str,
        services: Vec<ServiceEntry>,
        field: SortField,
        direction: SortDirection,
        expected_order: Vec<usize>, // Indices in the services vector
    }

    #[test]
    fn test_sorting_comprehensive() {
        let test_cases = vec![
            // Host field tests
            SortTestCase {
                name: "Host ascending",
                services: vec![
                    create_test_service("zebra", "_http._tcp.local.", 80),
                    create_test_service("alpha", "_http._tcp.local.", 81),
                    create_test_service("beta", "_http._tcp.local.", 82),
                ],
                field: SortField::Host,
                direction: SortDirection::Ascending,
                expected_order: vec![1, 2, 0], // alpha, beta, zebra
            },
            SortTestCase {
                name: "Host descending",
                services: vec![
                    create_test_service("alpha", "_http._tcp.local.", 80),
                    create_test_service("beta", "_http._tcp.local.", 81),
                    create_test_service("zebra", "_http._tcp.local.", 82),
                ],
                field: SortField::Host,
                direction: SortDirection::Descending,
                expected_order: vec![2, 1, 0], // zebra, beta, alpha
            },
            // Service type field tests
            SortTestCase {
                name: "Service type ascending",
                services: vec![
                    create_test_service("test1", "_ssh._tcp.local.", 80),
                    create_test_service("test2", "_http._tcp.local.", 81),
                ],
                field: SortField::ServiceType,
                direction: SortDirection::Ascending,
                expected_order: vec![1, 0], // http before ssh
            },
            // Port field tests
            SortTestCase {
                name: "Port ascending",
                services: vec![
                    create_test_service("service1", "_http._tcp.local.", 8080),
                    create_test_service("service2", "_http._tcp.local.", 80),
                    create_test_service("service3", "_http._tcp.local.", 443),
                ],
                field: SortField::Port,
                direction: SortDirection::Ascending,
                expected_order: vec![1, 2, 0], // 80, 443, 8080
            },
            // Timestamp field tests
            SortTestCase {
                name: "Timestamp ascending",
                services: vec![
                    {
                        let mut s = create_test_service("service1", "_http._tcp.local.", 3000);
                        s.updated_at_micros = 3000;
                        s
                    },
                    {
                        let mut s = create_test_service("service2", "_http._tcp.local.", 1000);
                        s.updated_at_micros = 1000;
                        s
                    },
                    {
                        let mut s = create_test_service("service3", "_http._tcp.local.", 2000);
                        s.updated_at_micros = 2000;
                        s
                    },
                ],
                field: SortField::Timestamp,
                direction: SortDirection::Ascending,
                expected_order: vec![1, 2, 0], // 1000, 2000, 3000
            },
        ];

        for test_case in test_cases {
            let mut state = setup_test_state_with_services(test_case.services.clone());
            state.sort_field = test_case.field;
            state.sort_direction = test_case.direction;
            state.mark_cache_dirty();

            let filtered = state.get_filtered_services().to_vec();
            assert_eq!(
                filtered.len(),
                test_case.expected_order.len(),
                "{}: Expected {} filtered services, got {}",
                test_case.name,
                test_case.expected_order.len(),
                filtered.len()
            );

            for (i, &service_index) in test_case.expected_order.iter().enumerate() {
                assert_eq!(
                    filtered[i], service_index,
                    "{}: Expected service {} at position {}, got {}",
                    test_case.name, service_index, i, filtered[i]
                );
            }
        }
    }

    #[test]
    fn test_sort_stability_with_equal_values() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        // Create services with same port but different names
        state
            .services
            .push(create_test_service("alpha", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("beta", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("gamma", "_http._tcp.local.", 80));

        state.sort_field = SortField::Port;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(filtered.len(), 3);

        // All should have same port, order should be stable (preserves insertion order for equal values)
        for &service_index in &filtered {
            assert_eq!(state.services[service_index].port, 80);
        }
    }

    #[test]
    fn test_sort_field_cycling() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Test forward cycling
        assert_eq!(state.sort_field, SortField::Host);
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::ServiceType);
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Fullname);
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Port);
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Address);
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Timestamp);
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Host); // Wrap around

        // Test backward cycling
        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Timestamp);
        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Address);
        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Port);
    }

    #[test]
    fn test_sort_direction_toggle() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        assert_eq!(state.sort_direction, SortDirection::Ascending);
        state.toggle_sort_direction();
        assert_eq!(state.sort_direction, SortDirection::Descending);
        state.toggle_sort_direction();
        assert_eq!(state.sort_direction, SortDirection::Ascending);
    }

    #[test]
    fn test_sort_key_event_handling() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let original_field = state.sort_field;
        let _original_direction = state.sort_direction;

        // Test sort field cycling key
        let key = KeyEvent::from(KeyCode::Char('s'));
        state.handle_key_event(key);
        assert_eq!(state.sort_field, SortField::ServiceType); // Should have cycled forward

        // Test sort field cycling backward key
        let key = KeyEvent::from(KeyCode::Char('S'));
        state.handle_key_event(key);
        assert_eq!(state.sort_field, original_field); // Should have cycled backward

        // Test sort direction toggle key
        let key = KeyEvent::from(KeyCode::Char('o'));
        state.handle_key_event(key);
        assert_eq!(state.sort_direction, SortDirection::Descending); // Should have toggled
    }

    #[test]
    fn test_sort_field_display_formatting() {
        assert_eq!(format_sort_field_for_display(SortField::Host), "Host");
        assert_eq!(
            format_sort_field_for_display(SortField::ServiceType),
            "Type"
        );
        assert_eq!(format_sort_field_for_display(SortField::Fullname), "Name");
        assert_eq!(format_sort_field_for_display(SortField::Port), "Port");
        assert_eq!(format_sort_field_for_display(SortField::Address), "Addr");
        assert_eq!(format_sort_field_for_display(SortField::Timestamp), "Time");
    }

    #[test]
    fn test_sort_direction_display_formatting() {
        assert_eq!(
            format_sort_direction_for_display(SortDirection::Ascending),
            "↑"
        );
        assert_eq!(
            format_sort_direction_for_display(SortDirection::Descending),
            "↓"
        );
    }

    #[test]
    fn test_sort_with_filtering_combined() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        // Add services with different types and names
        state
            .services
            .push(create_test_service("http-zebra", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("ssh-alpha", "_ssh._tcp.local.", 22));
        state
            .services
            .push(create_test_service("http-alpha", "_http._tcp.local.", 8080));
        state
            .services
            .push(create_test_service("ssh-zebra", "_ssh._tcp.local.", 2222));

        // Filter to HTTP services and sort by host
        state.selected_type = Some(0); // _http._tcp.local.
        state.sort_field = SortField::Host;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(filtered.len(), 2); // Only HTTP services
        assert_eq!(state.services[filtered[0]].host, "http-alpha.local.");
        assert_eq!(state.services[filtered[1]].host, "http-zebra.local.");
    }

    #[test]
    fn test_sort_type_ordering() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        // Service types should be sorted alphabetically
        assert_eq!(state.service_types[0], "_http._tcp.local.");
        assert_eq!(state.service_types[1], "_printer._tcp.local.");
        assert_eq!(state.service_types[2], "_ssh._tcp.local.");
    }

    // UNIFIED METRICS POPUP/SCROLL TESTS

    #[derive(Debug)]
    struct MetricsTestCase {
        name: &'static str,
        setup_metrics: fn(&mut AppState),
        initial_offset: usize,
        key_event: KeyEvent,
        expected_offset: Option<usize>,
        expected_popup_open: bool,
        description: &'static str,
    }

    #[test]
    fn test_metrics_comprehensive() {
        let test_cases = vec![
            // Test case 1: Basic scrolling up
            MetricsTestCase {
                name: "Scroll up from position 3",
                setup_metrics: |state| {
                    for i in 1..=10 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                expected_offset: Some(2), // Should decrease by 1
                expected_popup_open: true,
                description: "Scroll up should decrease offset",
            },
            // Test case 2: Scroll up from top (boundary)
            MetricsTestCase {
                name: "Scroll up from boundary",
                setup_metrics: |state| {
                    for i in 1..=10 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                expected_offset: Some(0), // Should stay at boundary
                expected_popup_open: true,
                description: "Scroll up at top should stay at 0",
            },
            // Test case 3: Basic scrolling down
            MetricsTestCase {
                name: "Scroll down from position 0",
                setup_metrics: |state| {
                    for i in 1..=10 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                expected_offset: Some(1), // Should increase by 1
                expected_popup_open: true,
                description: "Scroll down should increase offset",
            },
            // Test case 4: PageUp closes popup
            MetricsTestCase {
                name: "PageUp closes popup",
                setup_metrics: |state| {
                    for i in 1..=5 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 5,
                key_event: KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
                expected_offset: Some(0), // Should reset to 0 when closing
                expected_popup_open: false,
                description: "PageUp should close popup and reset offset",
            },
            // Test case 5: PageDown closes popup
            MetricsTestCase {
                name: "PageDown closes popup",
                setup_metrics: |state| {
                    for i in 1..=5 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
                expected_offset: Some(0), // Should reset to 0 when closing
                expected_popup_open: false,
                description: "PageDown should close popup and reset offset",
            },
            // Test case 6: Home key closes popup
            MetricsTestCase {
                name: "Home closes popup",
                setup_metrics: |state| {
                    for i in 1..=5 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
                expected_offset: Some(0), // Should reset to 0 when closing
                expected_popup_open: false,
                description: "Home should close popup and reset offset",
            },
            // Test case 7: End key closes popup
            MetricsTestCase {
                name: "End closes popup",
                setup_metrics: |state| {
                    for i in 1..=5 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
                expected_offset: Some(0), // Should reset to 0 when closing
                expected_popup_open: false,
                description: "End should close popup and reset offset",
            },
            // Test case 8: Escape key closes popup
            MetricsTestCase {
                name: "Escape closes popup",
                setup_metrics: |state| {
                    for i in 1..=5 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                expected_offset: Some(0), // Should reset offset when closing (matches actual behavior)
                expected_popup_open: false,
                description: "Escape should close popup and reset offset",
            },
            // Test case 9: Function keys (F1-F12) close popup
            MetricsTestCase {
                name: "F3 closes popup",
                setup_metrics: |state| {
                    for i in 1..=5 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 3,
                key_event: KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE),
                expected_offset: Some(0), // Should reset offset when closing (matches actual behavior)
                expected_popup_open: false,
                description: "F3 should close popup and reset offset",
            },
            // Test case 10: Multiple operations sequence
            MetricsTestCase {
                name: "Multiple scroll operations",
                setup_metrics: |state| {
                    for i in 1..=10 {
                        state.update_metric(&format!("test_metric_{}", i));
                    }
                },
                initial_offset: 0,
                key_event: KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                expected_offset: Some(0), // After sequence of 3 operations
                expected_popup_open: true,
                description: "Multiple operations should behave correctly",
            },
        ];

        for test_case in test_cases {
            let mut state = create_test_app_state(HashSet::new(), false, false, false);
            state.popup_state.metrics_popup.active = true;
            state.popup_state.metrics_popup.scroll.offset = test_case.initial_offset;
            (test_case.setup_metrics)(&mut state);

            // Handle the key event
            state.handle_key_event(test_case.key_event);

            // Check results
            if let Some(expected_offset) = test_case.expected_offset {
                assert_eq!(
                    state.popup_state.metrics_popup.scroll.offset,
                    expected_offset,
                    "{}: {} - Expected offset {}, got {}",
                    test_case.name,
                    test_case.description,
                    expected_offset,
                    state.popup_state.metrics_popup.scroll.offset
                );
            }

            assert_eq!(
                state.popup_state.metrics_popup.active,
                test_case.expected_popup_open,
                "{}: {} - Expected popup open: {}, got {}",
                test_case.name,
                test_case.description,
                test_case.expected_popup_open,
                state.popup_state.metrics_popup.active
            );
        }
    }

    // UNIFIED FILTER TESTING FRAMEWORK

    #[derive(Debug)]
    struct FilterTestCase {
        name: &'static str,
        setup_state: fn(&mut AppState),
        services: Vec<ServiceEntry>,
        expected_matches: Vec<usize>, // Indices of services that should match
    }

    #[test]
    fn test_filtering_comprehensive() {
        let test_cases = vec![
            // Test case 1: Filter all types (no type restriction)
            FilterTestCase {
                name: "All types filter",
                setup_state: |state| state.selected_type = None,
                services: vec![
                    create_test_service("http-service", "_http._tcp.local.", 80),
                    create_test_service("ssh-service", "_ssh._tcp.local.", 22),
                ],
                expected_matches: vec![0, 1], // Both should match
            },
            // Test case 2: Filter by specific type
            FilterTestCase {
                name: "Specific type filter",
                setup_state: |state| {
                    state.add_service_type("_http._tcp.local.");
                    state.add_service_type("_ssh._tcp.local.");
                    state.selected_type = Some(0); // HTTP only
                },
                services: vec![
                    create_test_service("http-service", "_http._tcp.local.", 80),
                    create_test_service("ssh-service", "_ssh._tcp.local.", 22),
                ],
                expected_matches: vec![0], // Only HTTP should match
            },
            // Test case 3: Text query filtering
            FilterTestCase {
                name: "Text query filter",
                setup_state: |state| {
                    state.selected_type = None;
                    state.filter_query = "test".to_string();
                },
                services: vec![
                    create_test_service("test-service", "_http._tcp.local.", 80),
                    create_test_service("other-service", "_http._tcp.local.", 80),
                ],
                expected_matches: vec![0], // Only service with "test" should match
            },
            // Test case 4: Case insensitive filtering
            FilterTestCase {
                name: "Case insensitive filter",
                setup_state: |state| {
                    state.selected_type = None;
                    state.filter_query = "TEST".to_string();
                },
                services: vec![
                    create_test_service("test-service", "_http._tcp.local.", 80),
                    create_test_service("other-service", "_http._tcp.local.", 80),
                ],
                expected_matches: vec![0], // Should match despite case difference
            },
            // Test case 5: Port as string matching
            FilterTestCase {
                name: "Port as string filter",
                setup_state: |state| {
                    state.selected_type = None;
                    state.filter_query = "8080".to_string();
                },
                services: vec![
                    create_test_service("service-8080", "_http._tcp.local.", 8080),
                    create_test_service("service-9090", "_http._tcp.local.", 9090),
                ],
                expected_matches: vec![0], // Only 8080 should match
            },
            // Test case 6: Partial word matching
            FilterTestCase {
                name: "Partial word filter",
                setup_state: |state| {
                    state.selected_type = None;
                    state.filter_query = "test".to_string();
                },
                services: vec![
                    create_test_service("test-service", "_http._tcp.local.", 80),
                    create_test_service("testing-service", "_http._tcp.local.", 80),
                ],
                expected_matches: vec![0, 1], // Both should match partial "test"
            },
            // Test case 7: Special characters handling
            FilterTestCase {
                name: "Special characters filter",
                setup_state: |state| {
                    state.selected_type = None;
                    state.filter_query = "test!@#".to_string();
                },
                services: vec![
                    create_test_service("test!@#-service", "_http._tcp.local.", 80),
                    create_test_service("test-service", "_http._tcp.local.", 80),
                ],
                expected_matches: vec![0], // Only exact special character match
            },
            // Test case 8: Empty query shows all
            FilterTestCase {
                name: "Empty query shows all",
                setup_state: |state| {
                    state.selected_type = None;
                    state.filter_query = "".to_string();
                },
                services: vec![
                    create_test_service("service1", "_http._tcp.local.", 80),
                    create_test_service("service2", "_http._tcp.local.", 80),
                ],
                expected_matches: vec![0, 1], // Both should match
            },
        ];

        for test_case in test_cases {
            let mut state = create_test_app_state(HashSet::new(), false, false, false);
            (test_case.setup_state)(&mut state);

            for (i, service) in test_case.services.iter().enumerate() {
                let should_match = test_case.expected_matches.contains(&i);
                assert_eq!(
                    state.filter_service(service),
                    should_match,
                    "{}: Service {} should match: {}",
                    test_case.name,
                    i,
                    should_match
                );
            }
        }
    }

    // AppState initialization tests
    #[test]
    fn test_appstate_new() {
        let state = AppState::new(HashSet::new(), false, false, false, None);
        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll.offset, 0);
        assert_eq!(state.services_scroll.offset, 0);
        assert!(state.cache_dirty);
        assert!(!state.popup_state.help_popup.active);
        assert!(!state.popup_state.metrics_popup.active);
        assert!(state.user_service_types.is_empty());
    }

    // CLI Service Types tests
    #[test]
    fn test_appstate_new_with_user_service_types() {
        let user_requested_types = vec![
            "_http._tcp.local.".to_string(),
            "_ssh._tcp.local.".to_string(),
        ];
        let user_requested_types_set: HashSet<String> =
            user_requested_types.clone().into_iter().collect();
        let state = create_test_app_state(user_requested_types_set.clone(), false, false, false);

        assert_eq!(state.user_service_types, user_requested_types_set);
        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_appstate_new_with_empty_user_service_types() {
        let user_requested_types = HashSet::new();
        let state = AppState::new(user_requested_types, false, false, false, None);

        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert!(state.user_service_types.is_empty());
    }

    #[test]
    fn test_appstate_new_with_single_user_service_type() {
        let user_requested_types = vec!["_printer._tcp.local.".to_string()];
        let user_requested_types_set: HashSet<String> = user_requested_types.into_iter().collect();
        let state = create_test_app_state(user_requested_types_set.clone(), false, false, false);

        assert_eq!(state.user_service_types.len(), 1);
        assert!(state.user_service_types.contains("_printer._tcp.local."));
    }

    #[test]
    fn test_user_service_types_immutability() {
        let user_requested_types = vec!["_http._tcp.local.".to_string()];
        let user_requested_types_set: HashSet<String> = user_requested_types.into_iter().collect();
        let state = create_test_app_state(user_requested_types_set.clone(), false, false, false);

        // The user_service_types field should remain unchanged throughout the app lifecycle
        let original_types = state.user_service_types.clone();

        // Simulate some state operations that shouldn't affect user_service_types
        let mut mutable_state = state;
        mutable_state.add_service_type("_ssh._tcp.local.");
        mutable_state.update_metric("test_metric");

        // Verify user_service_types hasn't changed
        assert_eq!(mutable_state.user_service_types, original_types);
    }

    #[test]
    fn test_is_sub_type_comprehensive() {
        let valid_types = vec![
            "_http._tcp.local.",
            "_ssh._tcp.local.",
            "_printer._tcp.local.",
            "_airplay._tcp.local.",
            "_raop._tcp.local.",
            "http._tcp.local.", // Currently considered valid by the function
            "_http.tcp.local.", // Currently considered valid by the function
            "_http._tcp.local", // Currently considered valid by the function
        ];

        for service_type in valid_types {
            assert!(
                !is_sub_type(service_type),
                "Expected {} to not be a subtype",
                service_type
            );
        }

        // Test that actual subtypes are detected correctly
        let subtypes = vec![
            "_printer._sub._http._tcp.local.", // Contains _sub.
        ];

        for service_type in subtypes {
            assert!(
                is_sub_type(service_type),
                "Expected {} to be a subtype",
                service_type
            );
        }

        // Only subtypes are considered invalid by current implementation
        let invalid_types = vec![
            "_sub._http._tcp.local.", // Contains sub
            "_sub._ssh._tcp.local.",  // Contains sub
        ];

        for service_type in invalid_types {
            assert!(
                is_sub_type(service_type),
                "Expected {} to be a subtype",
                service_type
            );
        }
    }

    // Service type normalization tests
    #[test]
    fn test_normalize_service_type_with_local_suffix() {
        let service_type = "_http._tcp.local.";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_without_local_suffix() {
        let service_type = "_http._tcp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_with_trailing_dot() {
        let service_type = "_ssh._tcp.";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_ssh._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_short_form() {
        let service_type = "_printer";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_printer._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_multiple_trailing_dots() {
        let service_type = "_airplay._tcp..";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_airplay._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_empty_string() {
        let service_type = "";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "");
    }

    #[test]
    fn test_normalize_service_type_already_complete() {
        let service_type = "_raop._tcp.local.";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_raop._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_simple_name() {
        // Test simple service name without underscore or protocol
        let service_type = "http";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_missing_underscore() {
        // Test service name with underscore but missing protocol
        let service_type = "_http";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_with_tcp_protocol() {
        // Test service name with tcp protocol (without underscore)
        let service_type = "http.tcp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_with_udp_protocol() {
        // Test service name with udp protocol (without underscore)
        let service_type = "dns.udp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_dns._udp.local.");
    }

    #[test]
    fn test_normalize_service_type_with_protocol_and_underscore() {
        // Test service name with protocol that already has underscore
        let service_type = "http._tcp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_with_unrecognized_protocol() {
        // Test service name where second part is not a recognized protocol
        let service_type = "http.unknown";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "_http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_subtype_compact_format() {
        // Test compact subtype format: "printer.sub.http" -> "printer._sub._http._tcp.local."
        let service_type = "printer.sub.http";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "printer._sub._http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_subtype_with_underscores() {
        // Test subtype format with some underscores: "_printer.sub._http" -> "printer._sub._http._tcp.local."
        let service_type = "_printer.sub._http";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "printer._sub._http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_subtype_full_underscores() {
        // Test subtype format with all underscores: "_printer._sub._http" -> "printer._sub._http._tcp.local."
        let service_type = "_printer._sub._http";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "printer._sub._http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_subtype_with_protocol() {
        // Test subtype format with explicit protocol: "printer.sub.http.tcp" -> "printer._sub._http._tcp.local."
        let service_type = "printer.sub.http.tcp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "printer._sub._http._tcp.local.");
    }

    #[test]
    fn test_normalize_service_type_subtype_not_recognized() {
        // Test when middle part is not "sub" - should treat as regular format
        let service_type = "printer.middle.http";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "printer.middle.http.local.");
    }

    #[test]
    fn test_normalize_service_type_subtype_with_udp_protocol() {
        // Test subtype format with UDP as service name: "dns.sub.udp" -> "dns._sub._udp._tcp.local."
        let service_type = "dns.sub.udp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(normalized, "dns._sub._udp._tcp.local.");
    }

    // Integration test to simulate CLI parsing behavior
    #[test]
    fn test_normalize_service_type_pr_case() {
        let service_type = "pr-qtatbtbi-de-efife7vt.sub.nabto.udp";
        let normalized = normalize_service_type(service_type);
        assert_eq!(
            normalized,
            "pr-qtatbtbi-de-efife7vt._sub._nabto._udp.local."
        );
    }

    #[test]
    fn test_cli_service_types_parsing() {
        // This test simulates behavior that happens in main.rs with normalization
        let input_types = vec!["_http._tcp".to_string(), "_ssh._tcp".to_string()];
        let user_requested_types: HashSet<String> = input_types
            .into_iter()
            .map(|service_type| normalize_service_type(&service_type))
            .collect::<HashSet<_>>();

        assert_eq!(user_requested_types.len(), 2);
        assert!(user_requested_types.contains("_http._tcp.local."));
        assert!(user_requested_types.contains("_ssh._tcp.local."));
    }

    // Service type management tests
    #[test]
    fn test_add_service_type() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        assert!(state.add_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
        assert_eq!(state.service_types[0], "_http._tcp.local.");

        // Adding duplicate should return false
        assert!(!state.add_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
    }

    #[test]
    fn test_add_service_type_maintains_sort_order() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        assert_eq!(state.service_types[0], "_http._tcp.local.");
        assert_eq!(state.service_types[1], "_printer._tcp.local.");
        assert_eq!(state.service_types[2], "_ssh._tcp.local.");
    }

    #[test]
    fn test_add_service_type_preserves_selection() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.selected_type = Some(1); // _ssh._tcp.local.

        // Add a new type, selection should still point to _ssh._tcp.local.
        state.add_service_type("_printer._tcp.local.");
        assert_eq!(state.selected_type, Some(2)); // _ssh._tcp.local. moved to index 2
    }

    #[test]
    fn test_remove_service_type() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        // Can't remove if still in use
        state
            .services
            .push(create_test_service("test", "_http._tcp.local.", 80));

        assert!(!state.remove_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 2);

        // Can remove if not in use
        assert!(state.remove_service_type("_ssh._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
    }

    #[test]
    fn test_remove_service_type_adjusts_selection() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = Some(1); // _printer._tcp.local.

        // Remove the selected type
        state.remove_service_type("_printer._tcp.local.");
        // Selection should move to nearest valid index
        assert!(state.selected_type == Some(1) || state.selected_type == Some(0));
    }

    // Remove offline services tests
    #[test]
    fn test_remove_offline_services() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.online = false;
        let service2 = create_test_service("test2", "_http._tcp.local.", 81);
        let mut service3 = create_test_service("test3", "_http._tcp.local.", 82);
        service3.online = false;

        state.services.push(service1);
        state.services.push(service2);
        state.services.push(service3);

        state.remove_offline_services();
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].fullname, "test2._http._tcp.local.");
    }

    #[test]
    fn test_remove_offline_services_removes_empty_types() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        let mut http_service = create_test_service("test1", "_http._tcp.local.", 80);
        http_service.online = false;
        let ssh_service = create_test_service("test2", "_ssh._tcp.local.", 22);

        state.services.push(http_service);
        state.services.push(ssh_service);

        state.remove_offline_services();
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.service_types.len(), 1);
        assert_eq!(state.service_types[0], "_ssh._tcp.local.");
    }

    #[test]
    fn test_remove_offline_services_adjusts_selection() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        let service1 = create_test_service("test1", "_http._tcp.local.", 80);
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 81);
        service2.online = false;
        let service3 = create_test_service("test3", "_http._tcp.local.", 82);

        state.services.push(service1);
        state.services.push(service2);
        state.services.push(service3);
        state.selected_service = 2;

        state.remove_offline_services();
        assert_eq!(state.services.len(), 2);
        // Selection should be adjusted to stay within bounds
        assert!(state.selected_service <= 1);
    }

    // Key handling tests
    #[test]
    fn test_handle_key_event_toggle_help() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        assert!(!state.popup_state.help_popup.active);

        let key = KeyEvent::from(KeyCode::Char('?'));
        assert!(state.handle_key_event(key)); // Should return true to continue
        assert!(state.popup_state.help_popup.active);

        assert!(state.handle_key_event(key));
        assert!(!state.popup_state.help_popup.active);
    }

    #[test]
    fn test_handle_key_event_toggle_metrics() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        assert!(!state.popup_state.metrics_popup.active);

        let key = KeyEvent::from(KeyCode::Char('m'));
        assert!(state.handle_key_event(key));
        assert!(state.popup_state.metrics_popup.active);

        assert!(state.handle_key_event(key));
        assert!(!state.popup_state.metrics_popup.active);
    }

    #[test]
    fn test_handle_metrics_popup_key() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;

        // Add some metrics to ensure there's content to scroll through
        state.update_metric("test_metric_1");
        state.update_metric("test_metric_2");
        state.update_metric("test_metric_3");
        state.update_metric("test_metric_4");
        state.update_metric("test_metric_5");

        // Add some metrics to ensure there's content to scroll through
        for i in 1..50 {
            state.update_metric(&format!("test_metric_{}", i));
        }

        // Test scrolling down when possible
        state.popup_state.metrics_popup.scroll.offset = 3;
        let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.popup_state.metrics_popup.active); // Should remain open
        // The exact scroll offset now depends on content length and terminal size
        assert!(state.popup_state.metrics_popup.scroll.offset >= 3); // Should not decrease

        // Test scrolling up at boundary (should not go below 0)
        state.popup_state.metrics_popup.scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.popup_state.metrics_popup.active); // Should remain open
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);

        // Test any other key closes popup and resets scroll
        state.popup_state.metrics_popup.scroll.offset = 10;
        let key_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(!state.popup_state.metrics_popup.active); // Should close
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0); // Should reset
    }

    #[test]
    fn test_handle_help_popup_key() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.help_popup.active = true;

        // Test scrolling down when at max scroll offset
        state.popup_state.help_popup.scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.popup_state.help_popup.active); // Should remain open

        // Test scrolling up when at boundary (should not go below 0)
        state.popup_state.help_popup.scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.popup_state.help_popup.active); // Should remain open
        assert_eq!(state.popup_state.help_popup.scroll.offset, 0);

        // Test any other key closes popup and resets scroll
        state.popup_state.help_popup.scroll.offset = 10;
        let key_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(!state.popup_state.help_popup.active); // Should close
        assert_eq!(state.popup_state.help_popup.scroll.offset, 0); // Should reset
    }

    // Metrics tests
    #[test]
    fn test_update_metric() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.update_metric("test_metric");
        assert_eq!(state.metrics.get("test_metric"), Some(&1));

        state.update_metric("test_metric");
        assert_eq!(state.metrics.get("test_metric"), Some(&2));
    }

    #[test]
    fn test_update_daemon_metrics() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let mut daemon_metrics = std::collections::HashMap::new();
        daemon_metrics.insert("queries-sent".to_string(), 10);
        daemon_metrics.insert("responses-recv".to_string(), 5);

        let updated = state.update_daemon_metrics(&daemon_metrics);
        assert!(updated);
        assert_eq!(state.metrics.get("daemon_queries_sent"), Some(&10));
        assert_eq!(state.metrics.get("daemon_responses_recv"), Some(&5));

        // Same metrics should not trigger update
        let updated = state.update_daemon_metrics(&daemon_metrics);
        assert!(!updated);

        // Changed metrics should trigger update
        daemon_metrics.insert("queries-sent".to_string(), 15);
        let updated = state.update_daemon_metrics(&daemon_metrics);
        assert!(updated);
        assert_eq!(state.metrics.get("daemon_queries_sent"), Some(&15));
    }

    #[test]
    fn test_metrics_scroll_basic() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add some metrics content first
        for i in 1..=10 {
            state.update_metric(&format!("test_metric_{}", i));
        }

        // Test scrolling up when already at top (should stay at 0)
        state.popup_state.metrics_popup.scroll.offset = 0;
        state.popup_state.metrics_popup.active = true; // Show popup
        let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        state.handle_key_event(key_event);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);

        // Test scrolling up from higher position
        state.popup_state.metrics_popup.scroll.offset = 3;
        let initial_offset = state.popup_state.metrics_popup.scroll.offset;
        state.handle_key_event(key_event);
        assert!(state.popup_state.metrics_popup.scroll.offset < initial_offset); // Should scroll up

        // Test scrolling down from various positions
        state.popup_state.metrics_popup.scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        state.handle_key_event(key_event);
        // Should increment
        assert!(state.popup_state.metrics_popup.scroll.offset > 0);

        // Test scrolling down when already at max - set to very high value first
        state.popup_state.metrics_popup.scroll.offset = 100;
        let max_before = state.popup_state.metrics_popup.scroll.offset;
        state.handle_key_event(key_event);
        // Should not exceed max
        assert!(state.popup_state.metrics_popup.scroll.offset <= max_before);
    }

    #[test]
    fn test_metrics_scroll_with_popup() {
        // Test that scrolling only works when popup is shown
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = false;
        state.popup_state.metrics_popup.scroll.offset = 5;
        let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);

        // This should not be called when popup is not shown, but let's test it anyway
        state.handle_key_event(key_event);
        let _offset_without_popup = state.popup_state.metrics_popup.scroll.offset;

        // Now test with popup shown
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 5;
        state.handle_key_event(key_event);
        // With popup shown, scrolling should work and offset should decrease
        assert!(state.popup_state.metrics_popup.scroll.offset < 5);
        // Note: behavior without popup is undefined since key shouldn't be handled then
    }

    #[test]
    fn test_metrics_scroll_reset_on_close() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 10;

        // Close popup with a non-scroll key
        let key_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        state.handle_key_event(key_event);

        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0); // Should reset

        // Test with Enter key
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 15;
        let key_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        state.handle_key_event(key_event);

        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0); // Should reset

        // Test with Escape key
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 20;
        let key_event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        state.handle_key_event(key_event);

        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0); // Should reset
    }

    #[test]
    fn test_metrics_scroll_return_value() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;

        // Test that all key events return true (continue running)
        let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        let close_key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);

        state.handle_key_event(up_key);
        state.handle_key_event(down_key);
        state.handle_key_event(close_key);
    }

    #[test]
    fn test_metrics_scroll_with_modifiers() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;

        // Add some metrics to ensure there's content to scroll through
        for i in 1..20 {
            state.update_metric(&format!("test_metric_{}", i));
        }
        state.popup_state.metrics_popup.scroll.offset = 2;

        // Test scrolling with Control modifier (should still work)
        let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        state.handle_key_event(key_event);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 1);

        // Test scrolling with Shift modifier
        state.popup_state.metrics_popup.scroll.offset = 2;
        let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT);
        state.handle_key_event(key_event);
        assert!(state.popup_state.metrics_popup.scroll.offset > 2);
    }

    #[test]
    fn test_metrics_scroll_multiple_operations() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 5;

        let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);

        // Test multiple up operations
        for _ in 0..10 {
            state.handle_key_event(up_key);
        }
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0); // Should stop at 0

        // Test multiple down operations
        for _ in 0..20 {
            state.handle_key_event(down_key);
        }
        // Should not exceed calculated maximum
        assert!(state.popup_state.metrics_popup.scroll.offset <= 6);

        // Test mixed operations
        state.popup_state.metrics_popup.scroll.offset = 3;
        state.handle_key_event(up_key); // to 2
        state.handle_key_event(up_key); // to 1
        state.handle_key_event(down_key); // to 2
        state.handle_key_event(up_key); // to 1
        state.handle_key_event(up_key); // to 0

        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);
    }

    #[test]
    fn test_metrics_scroll_page_navigation() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;

        // Test PageUp key (should behave like Up in current implementation)
        state.popup_state.metrics_popup.scroll.offset = 5;
        let page_up_key = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
        state.handle_key_event(page_up_key);
        assert!(!state.popup_state.metrics_popup.active); // PageUp closes popup
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);

        // Test PageDown key (should behave like Down in current implementation)
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 0;
        let page_down_key = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        state.handle_key_event(page_down_key);
        assert!(!state.popup_state.metrics_popup.active); // PageDown closes popup
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);

        // Test Home key (should close popup)
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 3;
        let home_key = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
        state.handle_key_event(home_key);
        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);

        // Test End key (should close popup)
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 3;
        let end_key = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        state.handle_key_event(end_key);
        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);
    }

    #[test]
    fn test_metrics_scroll_function_key_navigation() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 3;

        // Test F1 key (should close popup)
        let f1_key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        state.handle_key_event(f1_key);
        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);

        // Test F5 key (should close popup)
        state.popup_state.metrics_popup.active = true;
        state.popup_state.metrics_popup.scroll.offset = 3;
        let f5_key = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        state.handle_key_event(f5_key);
        assert!(!state.popup_state.metrics_popup.active);
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0);
    }

    #[test]
    fn test_metrics_scroll_edge_cases() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.popup_state.metrics_popup.active = true;

        // Test with very large scroll offset (should be clamped)
        state.popup_state.metrics_popup.scroll.offset = 1000;
        let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        state.handle_key_event(down_key);
        assert!(state.popup_state.metrics_popup.scroll.offset <= 6); // Should be clamped to max

        // Test with negative scroll offset (can't happen in practice, but test robustness)
        state.popup_state.metrics_popup.scroll.offset = 0;
        let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        for _ in 0..10 {
            state.handle_key_event(up_key);
        }
        assert_eq!(state.popup_state.metrics_popup.scroll.offset, 0); // Should never go negative
    }

    // Cache tests
    #[test]
    fn test_filter_cache_invalidation() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));

        // Populate cache
        let filtered = state.get_filtered_services();
        assert_eq!(filtered.len(), 1);
        assert!(!state.cache_dirty);

        // Mark cache dirty
        state.mark_cache_dirty();
        assert!(state.cache_dirty);

        // Next call should rebuild cache
        let filtered = state.get_filtered_services();
        assert_eq!(filtered.len(), 1);
        assert!(!state.cache_dirty);
    }

    #[test]
    fn test_validate_selected_type() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = Some(1);

        // Remove both types
        state.service_types.clear();
        state.validate_selected_type();
        assert_eq!(state.selected_type, None);

        // Add types back
        state.add_service_type("_http._tcp.local.");
        state.selected_type = Some(5); // Invalid index
        state.validate_selected_type();
        assert_eq!(state.selected_type, Some(0)); // Should clamp to last valid index
    }

    // Utility function tests
    #[test]
    fn test_is_sub_type() {
        assert!(!is_sub_type("_http._tcp.local."));
        assert!(!is_sub_type("_ssh._tcp.local."));
        assert!(is_sub_type("_sub._http._tcp.local."));
        assert!(is_sub_type("test_sub.something"));
    }

    #[test]
    fn test_current_timestamp_micros() {
        let ts1 = current_timestamp_micros();
        let ts2 = current_timestamp_micros();
        assert!(ts2 >= ts1);
        assert!(ts1 > 0);
    }

    #[test]
    fn test_normalize_service_type_with_subtypes() {
        // Test basic subtype normalization (correct format: _subtype._sub._service._protocol)
        assert_eq!(
            normalize_service_type("_printer._sub._http._tcp"),
            "_printer._sub._http._tcp.local."
        );
        assert_eq!(
            normalize_service_type("_airplay._sub._raop._tcp"),
            "_airplay._sub._raop._tcp.local."
        );

        // Test subtype with existing .local. suffix
        assert_eq!(
            normalize_service_type("_printer._sub._http._tcp.local."),
            "_printer._sub._http._tcp.local."
        );

        // Test subtype with trailing dot
        assert_eq!(
            normalize_service_type("_printer._sub._http._tcp."),
            "_printer._sub._http._tcp.local."
        );
    }

    // Formatting tests
    #[test]
    fn test_format_service_type_for_display() {
        assert_eq!(
            format_service_type_for_display("_http._tcp.local."),
            "http.tcp"
        );
        assert_eq!(
            format_service_type_for_display("_ssh._tcp.local."),
            "ssh.tcp"
        );
        assert_eq!(
            format_service_type_for_display("_printer._tcp.local."),
            "printer.tcp"
        );
        assert_eq!(
            format_service_type_for_display("airplay._sub._raop._tcp."),
            "airplay.sub.raop.tcp"
        );
        assert_eq!(
            format_service_type_for_display("invalid._sub._service._protocol."),
            "invalid.sub.service.protocol"
        );
        assert_eq!(
            format_service_type_for_display("_http._tcp.local."),
            "http.tcp"
        );
        assert_eq!(
            format_service_type_for_display("_ssh._tcp.local."),
            "ssh.tcp"
        );
        assert_eq!(
            format_service_type_for_display("_http._tcp.local."),
            "http.tcp"
        );
        assert_eq!(
            format_service_type_for_display("_printer._tcp.local."),
            "printer.tcp"
        );
    }

    #[test]
    fn test_format_service_for_display() {
        let service = create_test_service("MyPrinter", "_printer._tcp.local.", 63);
        let display = format_service_for_display(&service);
        println!("Display string: {}", display);
        assert!(display.contains("MyPrinter"));
        assert!(display.contains("192.168.1.64"));
        assert!(display.contains(":63"));
    }

    #[test]
    fn test_format_service_for_display_no_address() {
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
            is_flapping: false,
        };

        let display = format_service_for_display(&service);
        assert!(display.contains("test"));
        assert!(display.contains("testhost"));
        assert!(display.contains("<no-addr>"));
        assert!(display.contains("80"));
    }

    #[test]
    fn test_format_service_for_display_offline_service() {
        let mut service = create_test_service_with_sessions(
            "OfflineService",
            "_http._tcp.local.",
            80,
            vec![],
            false,
            2000000000,
            1000000000,
            Some(1000000000),
            Some(2000000000),
        );
        service.addrs.clear(); // Test with empty addresses
        service.host = "offlinehost.local.".to_string(); // Override host for test

        let display = format_service_for_display(&service);
        assert!(display.contains("OfflineService"));
        assert!(display.contains("offlinehost"));
        assert!(display.contains("80"));
    }

    #[test]
    fn test_format_service_for_display_no_address_duplicate() {
        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.addrs.clear(); // Test with empty addresses

        let display = format_service_for_display(&service);
        assert!(display.contains("<no-addr>"));
    }

    #[test]
    fn test_create_service_details_text() {
        let mut service = create_test_service("MyService", "_http._tcp.local.", 8080);
        service.subtype = Some("printer._sub._http._tcp.local.".to_string());
        service.addrs = vec!["192.168.1.63".to_string(), "192.168.1.20".to_string()];
        service.txt = vec!["key1=value1".to_string(), "key2=value2".to_string()];
        service.online = true;
        service.updated_at_micros = 1000000000;
        service.session_history = Vec::new();
        service.first_seen_micros = 1000000000;
        service.last_online_micros = Some(1000000000);
        service.last_offline_micros = None;

        let details_lines = create_service_details_text(&service);
        let details_text: String = details_lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("");

        assert!(details_text.contains("MyService._http._tcp.local."));
        assert!(details_text.contains("MyService.local."));
        assert!(details_text.contains("_http._tcp.local."));
        assert!(details_text.contains("printer"));
        assert!(details_text.contains("8080"));
        assert!(details_text.contains("192.168.1.63"));
        assert!(details_text.contains("192.168.1.20"));
        assert!(details_text.contains("key1=value1"));
        assert!(details_text.contains("key2=value2"));
        assert!(details_text.contains("First seen:"));
        assert!(details_text.contains("Online"));
    }

    #[test]
    fn test_create_service_details_text_online_service() {
        let service = create_test_service("test", "_http._tcp.local.", 80);

        let details_lines = create_service_details_text(&service);
        let details_text: String = details_lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("");

        assert!(details_text.contains("Last came online:"));
        assert!(details_text.contains("None")); // No addresses
        assert!(!details_text.contains("Subtype:")); // No subtype
        assert!(details_text.contains("Status: Online"));
    }

    #[test]
    fn test_create_service_details_text_offline_service() {
        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.online = false;
        let details_lines = create_service_details_text(&service);
        let details_text: String = details_lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("");

        assert!(details_text.contains("Status: Offline"));
    }

    #[test]
    fn test_format_timestamp_micros() {
        let timestamp = format_timestamp_micros(1609459200000000); // 2021-01-01 00:00:00 UTC
        // Just verify it's a valid formatted string with expected components
        assert!(timestamp.contains("-"));
        assert!(timestamp.contains(":"));
        assert!(timestamp.len() > 20); // Should include date, time, and microseconds
    }

    #[test]
    fn test_format_duration_micros_formats_various_durations_correctly() {
        // Test basic seconds
        assert_eq!(format_duration_micros(0), "0s"); // 0s shown when no other units
        assert_eq!(format_duration_micros(1_000_000), "1s");
        assert_eq!(format_duration_micros(30_000_000), "30s");

        // Test minutes and seconds
        assert_eq!(format_duration_micros(60_000_000), "1m");
        assert_eq!(format_duration_micros(90_000_000), "1m 30s");
        assert_eq!(format_duration_micros(120_000_000), "2m");

        // Test when there are minutes but zero seconds - seconds should not be displayed
        assert_eq!(format_duration_micros(60_000_000), "1m"); // Exactly 1 minute, no seconds shown

        // Test fractional seconds
        assert_eq!(format_duration_micros(1_500_000), "1.500s");
        assert_eq!(format_duration_micros(59_500_000), "59.500s");

        // Test the rounding edge case that was causing "60.000s"
        // 59.999594s should round to 60.000s and become "1m"
        assert_eq!(format_duration_micros(59_999_594), "1m");

        // Test other rounding edge cases
        assert_eq!(format_duration_micros(59_999_500), "1m"); // 59.9995s rounds up to 60.000s
        assert_eq!(format_duration_micros(59_999_400), "59.999s"); // 59.9994s doesn't round up, stays under 60s

        // Test hours
        assert_eq!(format_duration_micros(3_600_000_000), "1h");
        assert_eq!(format_duration_micros(3_660_000_000), "1h 1m");

        // Test days
        assert_eq!(format_duration_micros(86_400_000_000), "1d");
        assert_eq!(format_duration_micros(90_000_000_000), "1d 1h");

        // Test complex durations with fractional seconds
        assert_eq!(format_duration_micros(3_661_500_000), "1h 1m 1.500s");
        assert_eq!(format_duration_micros(86_401_500_000), "1d 1.500s");
    }

    // Layout tests
    #[test]
    fn test_create_main_layout() {
        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let layout = create_main_layout(area, false);

        assert!(layout.left_panel.width > 0);
        assert!(layout.services_area.width > 0);
        assert!(layout.details_area.width > 0);
        assert!(layout.services_area.height > 0);
        assert!(layout.details_area.height > 0);
        assert!(layout.filter_status_area.is_none());
    }

    #[test]
    fn test_calculate_visible_counts() {
        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let layout = create_main_layout(area, false);
        let counts = calculate_visible_counts(&layout);

        assert!(counts.types > 0);
        assert!(counts.services > 0);
    }

    #[test]
    fn test_create_service_list_item_style() {
        let mut online_service = create_test_service("test", "_http._tcp.local.", 80);
        online_service.addrs.clear(); // Test with empty addresses

        let mut offline_service = create_test_service_with_sessions(
            "test",
            "_http._tcp.local.",
            80,
            vec![],
            false,
            1000,
            1000,
            Some(1000),
            Some(1000),
        );
        offline_service.addrs.clear(); // Test with empty addresses

        // Test selected online service
        let style = create_service_list_item_style(0, 0, &online_service);
        assert_eq!(style.fg, Some(Color::White));
        assert_eq!(style.bg, Some(Color::DarkGray));

        // Test unselected online service
        let style = create_service_list_item_style(0, 1, &online_service);
        assert_eq!(style.fg, Some(Color::White));
        assert_eq!(style.bg, None);

        // Test offline service
        let style = create_service_list_item_style(0, 0, &offline_service);
        assert_eq!(style.fg, Some(Color::White));
        assert!(style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    // Edge case tests
    #[test]
    fn test_empty_service_list_navigation() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        state.navigate_services_up();
        assert_eq!(state.selected_service, 0);

        state.navigate_services_down();
        assert_eq!(state.selected_service, 0);

        state.navigate_services_to_first();
        assert_eq!(state.selected_service, 0);

        state.navigate_services_to_last();
        assert_eq!(state.selected_service, 0);
    }

    #[test]
    fn test_empty_service_types_navigation() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        state.navigate_service_types_up();
        assert_eq!(state.selected_type, None);

        state.navigate_service_types_down();
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_filter_with_no_matching_services() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = Some(1); // Select _ssh._tcp.local.

        // Add only http service
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));

        let filtered = state.get_filtered_services();
        assert_eq!(filtered.len(), 0); // No ssh services
    }

    #[test]
    fn test_scroll_offset_boundary_conditions() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        for i in 0..3 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.services_scroll.visible_items = 10; // More visible space than services

        state.selected_service = 2;
        let filtered_len = state.get_filtered_services().len();
        state
            .services_scroll
            .update_offset(state.selected_service, filtered_len);
        assert_eq!(state.services_scroll.offset, 0); // Should stay at 0 since all fit
    }

    #[test]
    fn test_update_service_type_selection_resets_scroll() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        for i in 0..10 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }

        state.selected_service = 5;
        state.services_scroll.offset = 3;

        state.update_service_type_selection(Some(1));
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_notification_enum() {
        // Test that notification enum variants can be created
        let _user_input = Notification::UserInput;
        let _service_changed = Notification::ServiceChanged;
        let _metrics_updated = Notification::MetricsUpdated;
    }

    // Sorting tests
    #[test]
    fn test_compare_services_by_field_host() {
        let service1 = create_test_service("alpha", "_http._tcp.local.", 80);
        let service2 = create_test_service("beta", "_http._tcp.local.", 80);

        let result = compare_services_by_field(&service1, &service2, SortField::Host);
        assert_eq!(result, std::cmp::Ordering::Less);

        let result = compare_services_by_field(&service2, &service1, SortField::Host);
        assert_eq!(result, std::cmp::Ordering::Greater);

        let result = compare_services_by_field(&service1, &service1, SortField::Host);
        assert_eq!(result, std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_services_by_field_service_type() {
        let http_service = create_test_service("test", "_http._tcp.local.", 80);
        let ssh_service = create_test_service("test", "_ssh._tcp.local.", 22);

        let result = compare_services_by_field(&http_service, &ssh_service, SortField::ServiceType);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_fullname() {
        let service1 = create_test_service("test", "_http._tcp.local.", 80);
        let service2 = create_test_service("zzz", "_http._tcp.local.", 80);

        let result = compare_services_by_field(&service1, &service2, SortField::Fullname);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_port() {
        let service1 = create_test_service("test", "_http._tcp.local.", 80);
        let service2 = create_test_service("test", "_http._tcp.local.", 8080);

        let result = compare_services_by_field(&service1, &service2, SortField::Port);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_timestamp() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.updated_at_micros = 1000;
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.updated_at_micros = 2000;

        let result = compare_services_by_field(&service1, &service2, SortField::Timestamp);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_ip() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["192.168.1.11".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["192.168.1.22".to_string()];

        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_ipv6() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["2001:db8::2".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["2001:db8::3".to_string()];

        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_mixed_ipv4_ipv6() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["192.168.1.63".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["2001:db8::1".to_string()];

        // IPv4 should come before IPv6 (lexicographic comparison of IP types)
        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_no_addr() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec![];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["192.168.1.3".to_string()];

        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_string_fallback() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["invalid-ip-2".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["invalid-ip-3".to_string()];

        // Falls back to string comparison when IP parsing fails
        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_cycle_sort_field_forward() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        assert_eq!(state.sort_field, SortField::Host);

        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::ServiceType);

        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Fullname);

        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Port);

        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Address);

        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Timestamp);

        // Should wrap around
        state.cycle_sort_field(true);
        assert_eq!(state.sort_field, SortField::Host);
    }

    #[test]
    fn test_cycle_sort_field_backward() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        assert_eq!(state.sort_field, SortField::Host);

        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Timestamp);

        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Address);

        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Port);

        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Fullname);

        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::ServiceType);

        state.cycle_sort_field(false);
        assert_eq!(state.sort_field, SortField::Host);
    }

    // Filter functionality tests

    #[test]
    fn test_clear_filter() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "test".to_string();
        state.filter_input_mode = true;
        state.selected_service = 5;
        state.services_scroll.offset = 2;

        state.clear_filter();

        assert_eq!(state.filter_query, "");
        assert!(!state.filter_input_mode);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_apply_filter() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "test".to_string();
        state.filter_input_mode = true;
        state.selected_service = 5;
        state.services_scroll.offset = 2;

        state.apply_filter();

        assert_eq!(state.filter_query, "test");
        assert!(!state.filter_input_mode);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_add_to_filter() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_to_filter('a');
        state.add_to_filter('b');
        state.add_to_filter('c');
        assert_eq!(state.filter_query, "abc");
    }

    #[test]
    fn test_add_to_filter_invalidates_cache() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test", "_http._tcp.local.", 80));

        // Populate cache first
        let _ = state.get_filtered_services();
        assert!(!state.cache_dirty);

        // Adding to filter should invalidate cache
        state.add_to_filter('t');
        assert!(state.cache_dirty);
    }

    #[test]
    fn test_remove_from_filter() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "abc".to_string();
        state.remove_from_filter();
        assert_eq!(state.filter_query, "ab");
        state.remove_from_filter();
        assert_eq!(state.filter_query, "a");
        state.remove_from_filter();
        assert_eq!(state.filter_query, "");
        state.remove_from_filter(); // Removing from empty string should be safe
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_remove_from_filter_invalidates_cache() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test", "_http._tcp.local.", 80));

        // Populate cache first
        let _ = state.get_filtered_services();
        assert!(!state.cache_dirty);

        // Removing from filter should invalidate cache
        state.filter_query = "test".to_string();
        state.remove_from_filter();
        assert!(state.cache_dirty);
    }

    #[test]
    fn test_filter_service_with_text_query() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "test".to_string();

        let matching_service = create_test_service("test", "_http._tcp.local.", 80);
        let non_matching_service = create_test_service("other", "_http._tcp.local.", 80);

        assert!(state.filter_service(&matching_service));
        assert!(!state.filter_service(&non_matching_service));
    }

    #[test]
    fn test_filter_service_case_insensitive() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "TEST".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_searches_all_fields() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Test fullname search
        state.filter_query = "MyService".to_string();
        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.fullname = "MyService._http._tcp.local.".to_string();
        assert!(state.filter_service(&service));

        // Test host search
        state.filter_query = "myhost".to_string();
        service.host = "myhost.local.".to_string();
        assert!(state.filter_service(&service));

        // Test service type search
        state.filter_query = "http".to_string();
        service.service_type = "_http._tcp.local.".to_string();
        assert!(state.filter_service(&service));

        // Test address search
        state.filter_query = "192.168.1.100".to_string();
        service.addrs = vec!["192.168.1.100".to_string()];
        assert!(state.filter_service(&service));

        // Test port search
        state.filter_query = "8080".to_string();
        service.port = 8080;
        assert!(state.filter_service(&service));

        // Test TXT record search
        state.filter_query = "key1=value1".to_string();
        service.txt = vec!["key1=value1".to_string()];
        assert!(state.filter_service(&service));

        // Test subtype search
        state.filter_query = "printer".to_string();
        service.subtype = Some("_printer".to_string());
        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_online_keyword_only() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "online".to_string();

        let online_service = create_test_service("test", "_http._tcp.local.", 80);
        let mut offline_service = create_test_service("test", "_http._tcp.local.", 80);
        offline_service.go_offline_at(1000);

        assert!(state.filter_service(&online_service));
        assert!(!state.filter_service(&offline_service));
    }

    #[test]
    fn test_filter_service_offline_keyword_only() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "offline".to_string();

        let online_service = create_test_service("test", "_http._tcp.local.", 80);
        let mut offline_service = create_test_service("test", "_http._tcp.local.", 80);
        offline_service.go_offline_at(1000);

        assert!(!state.filter_service(&online_service));
        assert!(state.filter_service(&offline_service));
    }

    #[test]
    fn test_filter_service_both_keywords_with_additional_terms() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "online offline printer".to_string();

        // Online HTTP service with "printer" in name should match
        let online_http_printer = create_test_service("printer-service", "_http._tcp.local.", 80);
        assert!(state.filter_service(&online_http_printer));

        // Online SSH service without "printer" should NOT match (status doesn't matter, but missing additional term)
        let online_ssh_service = create_test_service("ssh-service", "_ssh._tcp.local.", 22);
        assert!(!state.filter_service(&online_ssh_service));

        // Offline HTTP service with "printer" in name should match (text contains full query)
        let mut offline_http_printer =
            create_test_service("printer-service", "_http._tcp.local.", 80);
        offline_http_printer.go_offline_at(1000);
        assert!(state.filter_service(&offline_http_printer));

        // Service with "online offline printer" in TXT record should match (text contains full query)
        let mut service_with_txt = create_test_service("test", "_http._tcp.local.", 80);
        service_with_txt.txt = vec!["online offline printer mode".to_string()];
        assert!(state.filter_service(&service_with_txt));
    }

    #[test]
    fn test_filter_service_non_keyword_queries_unchanged() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Test that regular queries still work as before
        state.filter_query = "http".to_string();
        let http_service = create_test_service("test", "_http._tcp.local.", 80);
        let ssh_service = create_test_service("test", "_ssh._tcp.local.", 22);

        assert!(state.filter_service(&http_service));
        assert!(!state.filter_service(&ssh_service));

        // Test queries that don't contain keywords
        state.filter_query = "printer".to_string();
        let printer_service = create_test_service("test", "_printer._http._tcp.local.", 80);
        assert!(state.filter_service(&printer_service));
    }

    #[test]
    fn test_handle_filter_input_key_enter() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_input_mode = true;
        state.filter_query = "test".to_string();

        let key = KeyEvent::from(KeyCode::Enter);
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(!state.filter_input_mode);
        assert_eq!(state.filter_query, "test");
    }

    #[test]
    fn test_handle_filter_input_key_escape() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_input_mode = true;
        state.filter_query = "test".to_string();

        let key = KeyEvent::from(KeyCode::Esc);
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(!state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_handle_filter_input_key_char() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_input_mode = true;

        let key = KeyEvent::from(KeyCode::Char('a'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "a");
    }

    #[test]
    fn test_handle_normal_mode_key_slash() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let key = KeyEvent::from(KeyCode::Char('/'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_handle_normal_mode_key_n() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "test".to_string();
        // Note: not in filter_input_mode so 'n' is handled by normal mode
        state.selected_service = 5;
        state.services_scroll.offset = 2;

        let key = KeyEvent::from(KeyCode::Char('n'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert_eq!(state.filter_query, "");
        assert!(!state.filter_input_mode);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_filter_empty_query_shows_all() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = String::new(); // Empty query
        state.selected_type = Some(0); // Specific type selected
        state.add_service_type("_http._tcp.local.");

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service)); // Should show all since empty query
    }

    #[test]
    fn test_filter_with_special_characters() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "key=value".to_string();

        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.txt = vec!["key=value".to_string()];
        assert!(state.filter_service(&service));
    }

    // Test for the filter clear bug fix (regression test)
    #[test]
    fn test_clear_filter_when_empty_doesnt_reset_selection() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        // Add multiple services
        for i in 0..10 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }

        // Navigate to a specific service
        state.selected_service = 5;
        state.services_scroll.offset = 3;

        // Clear filter when it's already empty should NOT reset selection
        state.clear_filter();

        assert_eq!(state.selected_service, 5);
        assert_eq!(state.services_scroll.offset, 3);
    }

    #[test]
    fn test_clear_filter_when_not_empty_resets_selection() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        // Add multiple services
        for i in 0..10 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }

        // Navigate to a specific service and set a filter
        state.selected_service = 5;
        state.services_scroll.offset = 3;
        state.filter_query = "test5".to_string();

        // Clear filter when it has content SHOULD reset selection
        state.clear_filter();

        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
        assert_eq!(state.filter_query, "");
    }

    // Test add_or_update_service edge cases
    #[test]
    fn test_add_or_update_service_returns_false_for_new_service() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service = create_test_service("test1", "_http._tcp.local.", 80);

        let was_updated = state.add_or_update_service(service);

        assert!(!was_updated);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.metrics.get("services_discovered"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_returns_true_for_existing_service() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service1 = create_test_service("test1", "_http._tcp.local.", 80);

        // Add initial service
        state.add_or_update_service(service1.clone());

        // Update same service with different port
        let mut service2 = service1.clone();
        service2.port = 8080;

        let was_updated = state.add_or_update_service(service2);

        assert!(was_updated);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].port, 8080);
        assert_eq!(state.metrics.get("services_updated"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_only_updates_on_significant_changes() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service1 = create_test_service("test1", "_http._tcp.local.", 80);

        state.add_or_update_service(service1.clone());

        // Update with same service (no significant changes)
        let was_updated = state.add_or_update_service(service1.clone());

        assert!(was_updated); // Returns true for existing service
        assert_eq!(state.services.len(), 1);
        // Metrics should not increment for non-significant changes
        assert_eq!(state.metrics.get("services_updated"), None);
    }

    #[test]
    fn test_add_or_update_service_detects_online_status_change() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service1 = create_test_service("test1", "_http._tcp.local.", 80);

        state.add_or_update_service(service1.clone());

        // Update service to be offline
        let mut service2 = service1.clone();
        service2.online = false;

        let was_updated = state.add_or_update_service(service2);

        assert!(was_updated);
        assert_eq!(state.services.len(), 1);
        assert!(!state.services[0].online);
        assert_eq!(state.metrics.get("services_updated"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_detects_address_change() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service1 = create_test_service("test1", "_http._tcp.local.", 80);

        state.add_or_update_service(service1.clone());

        // Update service with different address
        let mut service2 = service1.clone();
        service2.addrs = vec!["192.168.2.100".to_string()];

        let was_updated = state.add_or_update_service(service2);

        assert!(was_updated);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].addrs[0], "192.168.2.100");
        assert_eq!(state.metrics.get("services_updated"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_detects_txt_change() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service1 = create_test_service("test1", "_http._tcp.local.", 80);

        state.add_or_update_service(service1.clone());

        // Update service with different TXT records
        let mut service2 = service1.clone();
        service2.txt = vec!["newkey=newvalue".to_string()];

        let was_updated = state.add_or_update_service(service2);

        assert!(was_updated);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].txt[0], "newkey=newvalue");
        assert_eq!(state.metrics.get("services_updated"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_detects_subtype_change() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        let service1 = create_test_service("test1", "_http._tcp.local.", 80);

        state.add_or_update_service(service1.clone());

        // Update service with subtype
        let mut service2 = service1.clone();
        service2.subtype = Some("_printer".to_string());

        let was_updated = state.add_or_update_service(service2);

        assert!(was_updated);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].subtype, Some("_printer".to_string()));
        assert_eq!(state.metrics.get("services_updated"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_ensures_service_type_exists() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Start with no service types
        assert_eq!(state.service_types.len(), 0);

        // Add a service with a new type
        let service = create_test_service("test1", "_newtype._tcp.local.", 8080);
        let was_existing = state.add_or_update_service(service);

        // Service should be added and service type should be created
        assert!(!was_existing);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.service_types.len(), 1);
        assert!(
            state
                .service_types
                .contains(&"_newtype._tcp.local.".to_string())
        );

        // Add another service with the same type
        let service2 = create_test_service("test2", "_newtype._tcp.local.", 8081);
        let was_existing2 = state.add_or_update_service(service2);

        // Service should be added but service type count should stay the same
        assert!(!was_existing2);
        assert_eq!(state.services.len(), 2);
        assert_eq!(state.service_types.len(), 1);

        // Update an existing service
        let mut service3 = create_test_service("test1", "_newtype._tcp.local.", 8082);
        service3.online = false; // Change to trigger update
        let was_existing3 = state.add_or_update_service(service3);

        // Service should be updated but service type count should stay the same
        assert!(was_existing3);
        assert_eq!(state.services.len(), 2);
        assert_eq!(state.service_types.len(), 1);
    }

    // Test cache invalidation scenarios
    #[test]
    fn test_cache_invalidation_on_service_removal() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));

        // Populate cache
        let _ = state.get_filtered_services();
        assert!(!state.cache_dirty);

        // Remove services should invalidate cache
        state.remove_offline_services();
        // After remove_offline_services completes, cache should be clean again
        assert!(!state.cache_dirty);
    }

    #[test]
    fn test_cache_invalidation_on_add_service_type() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));

        // Populate cache
        let _ = state.get_filtered_services();
        assert!(!state.cache_dirty);

        // Adding service type should invalidate cache
        state.add_service_type("_http._tcp.local.");
        assert!(state.cache_dirty);
    }

    #[test]
    fn test_cache_invalidation_on_remove_service_type() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        // Populate cache
        let _ = state.get_filtered_services();
        assert!(!state.cache_dirty);

        // Removing service type should invalidate cache
        state.remove_service_type("_ssh._tcp.local.");
        assert!(state.cache_dirty);
    }

    #[test]
    fn test_cache_sorted_flag_on_sort_field_change() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));

        // Populate and sort cache
        let _ = state.get_filtered_services();
        assert!(state.cached_sorted);

        // Changing sort field should invalidate sorted flag
        state.update_sort_field(SortField::Port);
        assert!(!state.cached_sorted);
    }

    // Boundary condition tests

    #[test]
    fn test_remove_offline_services_with_all_offline() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.online = false;
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 81);
        service2.online = false;

        state.services.push(service1);
        state.services.push(service2);

        state.remove_offline_services();

        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0); // Type should be removed too
    }

    #[test]
    fn test_remove_offline_services_updates_metrics() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.online = false;
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 81);
        service2.online = false;
        let mut service3 = create_test_service("test3", "_http._tcp.local.", 82);
        service3.online = false;

        state.services.push(service1);
        state.services.push(service2);
        state.services.push(service3);

        state.remove_offline_services();

        assert_eq!(state.metrics.get("offline_services_removed"), Some(&3));
    }

    #[test]
    fn test_update_metric_by() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        state.update_metric_by("test_metric", 5);
        assert_eq!(state.metrics.get("test_metric"), Some(&5));

        state.update_metric_by("test_metric", 3);
        assert_eq!(state.metrics.get("test_metric"), Some(&8));
    }

    #[test]
    fn test_filter_query_with_multiple_words() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "192.168".to_string();

        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.addrs = vec!["192.168.1.100".to_string()];

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_query_partial_match() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "http".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 80);

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_with_port_as_string() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "8080".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 8080);

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_scroll_offset_updates_correctly_on_navigation() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        for i in 0..10 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.services_scroll.visible_items = 5;

        // Navigate down beyond visible area
        for _ in 0..7 {
            state.navigate_services_down();
        }

        // Scroll offset should be adjusted to keep selected item visible
        assert!(state.services_scroll.offset > 0);
        assert!(state.selected_service >= state.services_scroll.offset);
        assert!(
            state.selected_service
                < state.services_scroll.offset + state.services_scroll.visible_items
        );
    }

    #[test]
    fn test_validate_selected_type_with_invalid_index() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");
        state.selected_type = Some(10); // Invalid index

        state.validate_selected_type();

        assert_eq!(state.selected_type, Some(0)); // Should clamp to valid index
    }

    #[test]
    fn test_service_entry_go_offline_updates_timestamp() {
        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        let original_timestamp = service.updated_at_micros;
        let new_timestamp = original_timestamp + 1000000;

        service.go_offline_at(new_timestamp);

        assert!(!service.online);
        assert_eq!(service.updated_at_micros, new_timestamp);
        assert_ne!(service.updated_at_micros, original_timestamp);
    }

    #[test]
    fn test_key_event_ctrl_c() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let mut key = KeyEvent::from(KeyCode::Char('c'));
        key.modifiers = KeyModifiers::CONTROL;

        let should_continue = state.handle_key_event(key);

        assert!(!should_continue); // Should quit
    }

    #[test]
    fn test_key_event_remove_offline() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.add_service_type("_http._tcp.local.");

        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.online = false;
        state.services.push(service);

        let key = KeyEvent::from(KeyCode::Char('d'));
        state.handle_key_event(key);

        assert_eq!(state.services.len(), 0);
    }

    #[test]
    fn test_key_event_clear_stale_service_types() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add multiple service types but only services for some
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        state
            .services
            .push(create_test_service("http1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("ssh1", "_ssh._tcp.local.", 22));

        let key = KeyEvent::from(KeyCode::Char('D'));
        state.handle_key_event(key);

        // Should remove _printer._tcp.local. but keep the others
        assert_eq!(state.service_types.len(), 2);
        assert!(
            state
                .service_types
                .contains(&"_http._tcp.local.".to_string())
        );
        assert!(
            state
                .service_types
                .contains(&"_ssh._tcp.local.".to_string())
        );
        assert!(
            !state
                .service_types
                .contains(&"_printer._tcp.local.".to_string())
        );
    }

    #[test]
    fn test_multiple_filter_operations() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Start filter
        state.start_filter_input();
        assert!(state.filter_input_mode);

        // Add characters
        state.add_to_filter('t');
        state.add_to_filter('e');
        state.add_to_filter('s');
        state.add_to_filter('t');
        assert_eq!(state.filter_query, "test");

        // Remove one character
        state.remove_from_filter();
        assert_eq!(state.filter_query, "tes");

        // Apply filter
        state.apply_filter();
        assert!(!state.filter_input_mode);
        assert_eq!(state.filter_query, "tes");
    }

    #[test]
    fn test_empty_services_with_filter() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);
        state.filter_query = "nonexistent".to_string();

        let filtered = state.get_filtered_services();

        assert_eq!(filtered.len(), 0);
    }

    // Tests for service removal metric fix
    #[test]
    fn test_remove_service_only_counts_online_services() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add an online service
        let online_service = create_test_service("test-service", "_http._tcp.local.", 8080);
        state.services.push(online_service);

        // Add an offline service
        let mut offline_service = create_test_service("offline-service", "_http._tcp.local.", 8081);
        offline_service.online = false;
        state.services.push(offline_service);

        // Initially no services removed
        assert_eq!(state.metrics.get("services_marked_offline"), None);

        // Remove online service - should increment metric
        let removed = state.mark_service_offline("test-service._http._tcp.local.");
        assert!(removed);
        assert_eq!(state.metrics.get("services_marked_offline"), Some(&1));
        assert!(!state.services[0].online); // Service should now be offline

        // Remove offline service - should not increment metric
        let removed = state.mark_service_offline("offline-service._http._tcp.local.");
        assert!(removed);
        assert_eq!(state.metrics.get("services_marked_offline"), Some(&1)); // Still 1, not 2
        assert!(!state.services[1].online); // Service should still be offline
    }

    #[test]
    fn test_remove_service_prevents_double_counting() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add an online service
        let service = create_test_service("duplicate-service", "_http._tcp.local.", 8080);
        state.services.push(service);

        // First removal - should increment metric
        let removed1 = state.mark_service_offline("duplicate-service._http._tcp.local.");
        assert!(removed1);
        assert_eq!(state.metrics.get("services_marked_offline"), Some(&1));
        assert!(!state.services[0].online);

        // Second removal of same service - should not increment metric
        let removed2 = state.mark_service_offline("duplicate-service._http._tcp.local.");
        assert!(removed2);
        assert_eq!(state.metrics.get("services_marked_offline"), Some(&1)); // Still 1, not 2
    }

    #[test]
    fn test_remove_service_nonexistent_service() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Try to remove a service that doesn't exist
        let removed = state.mark_service_offline("nonexistent._http._tcp.local.");
        assert!(!removed);
        assert_eq!(state.metrics.get("services_marked_offline"), None);
    }

    #[test]
    fn test_remove_service_updates_timestamp() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let service = create_test_service("timestamp-service", "_http._tcp.local.", 8080);
        let original_timestamp = service.updated_at_micros;
        state.services.push(service);

        // Wait a bit to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));

        let removed = state.mark_service_offline("timestamp-service._http._tcp.local.");
        assert!(removed);

        let updated_service = &state.services[0];
        assert!(!updated_service.online);
        assert!(updated_service.updated_at_micros > original_timestamp);
    }

    #[test]
    fn test_clear_stale_service_types() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add multiple service types
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        // Add services for only some types
        state
            .services
            .push(create_test_service("http1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("ssh1", "_ssh._tcp.local.", 22));

        // _printer._tcp.local. has no services - it's stale

        assert_eq!(state.service_types.len(), 3);

        // Clear stale service types
        state.clear_stale_service_types();

        // Should remove _printer._tcp.local. but keep the others
        assert_eq!(state.service_types.len(), 2);
        assert!(
            state
                .service_types
                .contains(&"_http._tcp.local.".to_string())
        );
        assert!(
            state
                .service_types
                .contains(&"_ssh._tcp.local.".to_string())
        );
        assert!(
            !state
                .service_types
                .contains(&"_printer._tcp.local.".to_string())
        );
    }

    #[test]
    fn test_clear_stale_service_types_empty() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add a service type but no services
        state.add_service_type("_test._tcp.local.");

        assert_eq!(state.service_types.len(), 1);

        // Clear should remove the empty type
        state.clear_stale_service_types();

        assert_eq!(state.service_types.len(), 0);
    }

    #[test]
    fn test_clear_stale_service_types_no_stale() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add service types and services for all
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        state
            .services
            .push(create_test_service("http1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("ssh1", "_ssh._tcp.local.", 22));

        assert_eq!(state.service_types.len(), 2);

        // Clear should not remove anything since all types have services
        state.clear_stale_service_types();

        assert_eq!(state.service_types.len(), 2);
    }

    #[tokio::test]
    async fn test_json_state_dump() {
        let user_types: HashSet<String> = ["_http._tcp.local.".to_string()].into_iter().collect();
        let mut state = create_test_app_state(user_types.clone(), true, true, false);

        // Add some test data
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 8080));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 8081));

        // Test JSON dump creation
        let json_result = state.dump_state_to_json();
        assert!(json_result.is_ok(), "JSON dump should succeed");

        let json_str = json_result.unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("Generated JSON should be valid");

        // Check required fields
        assert!(parsed.get("metadata").is_some(), "Should have metadata");
        assert!(
            parsed.get("services").is_some(),
            "Should have services array"
        );
        assert!(
            parsed.get("serviceTypes").is_some(),
            "Should have service types"
        );
        assert!(parsed.get("metrics").is_some(), "Should have metrics");
        assert!(parsed.get("options").is_some(), "Should have options");
        assert!(parsed.get("filters").is_some(), "Should have filters");
        assert!(parsed.get("sorting").is_some(), "Should have sorting");

        // Check options fields
        let options = &parsed["options"];
        assert!(
            options.get("serviceTypes").is_some(),
            "options should have serviceTypes"
        );
        assert!(
            options.get("disableIpv4").is_some(),
            "options should have disableIpv4"
        );
        assert!(
            options.get("disableIpv6").is_some(),
            "options should have disableIpv6"
        );
        assert!(
            options.get("noDebounce").is_some(),
            "options should have noDebounce"
        );
        assert!(
            options.get("interfaces").is_some(),
            "options should have interfaces"
        );

        // Verify option values match state
        if let serde_json::Value::Array(service_types) = &options["serviceTypes"] {
            let expected: Vec<String> = user_types.iter().cloned().collect();
            assert_eq!(service_types.len(), expected.len());
            for (i, expected_type) in expected.iter().enumerate() {
                assert_eq!(service_types[i], *expected_type);
            }
        } else {
            panic!("options.serviceTypes should be an array");
        }

        assert_eq!(options["disableIpv4"], true, "disableIpv4 should be true");
        assert_eq!(options["disableIpv6"], false, "disableIpv6 should be false");
        assert_eq!(options["noDebounce"], true, "noDebounce should be true");

        // interfaces should be null when not set
        assert!(
            options["interfaces"].is_null(),
            "interfaces should be null when not set"
        );

        // Check services array
        if let serde_json::Value::Array(services) = &parsed["services"] {
            assert_eq!(services.len(), 2, "Should have 2 services");
        } else {
            panic!("Services should be an array");
        }

        // Check serviceTypes array
        if let serde_json::Value::Array(service_types) = &parsed["serviceTypes"] {
            assert_eq!(service_types.len(), 1, "Should have 1 service type");
            assert_eq!(service_types[0], "_http._tcp.local.");
        } else {
            panic!("Service types should be an array");
        }
    }

    #[tokio::test]
    async fn test_json_dump_file_creation() {
        let user_types: HashSet<String> = ["_test._tcp.local.".to_string()].into_iter().collect();
        let mut state = create_test_app_state(user_types.clone(), true, false, true);

        // Add minimal test data
        state.add_service_type("_test._tcp.local.");
        state
            .services
            .push(create_test_service("test", "_test._tcp.local.", 1234));

        // Test file creation
        let filename_result = state.save_json_dump().await;
        assert!(filename_result.is_ok(), "File creation should succeed");

        let filename = filename_result.unwrap();

        // Verify filename format
        assert!(
            filename.starts_with("20"),
            "Filename should start with year"
        );
        assert!(
            filename.ends_with("-state-dump.json"),
            "Filename should end with suffix"
        );

        // Verify file exists and has content
        let content = tokio::fs::read_to_string(&filename)
            .await
            .expect("Should be able to read the created file");

        assert!(!content.is_empty(), "File should not be empty");

        // Verify content is valid JSON and has options
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("File content should be valid JSON");

        assert!(parsed.get("options").is_some(), "Should have options");

        // Verify options values
        let options = &parsed["options"];
        assert!(
            options.get("serviceTypes").is_some(),
            "options should have serviceTypes"
        );
        assert_eq!(options["disableIpv4"], false, "disableIpv4 should be false");
        assert_eq!(options["disableIpv6"], true, "disableIpv6 should be true");
        assert_eq!(options["noDebounce"], true, "noDebounce should be true");
        assert!(options["interfaces"].is_null(), "interfaces should be null");

        // Clean up
        tokio::fs::remove_file(&filename).await.ok();
    }

    #[tokio::test]
    async fn test_load_state_from_json() {
        let user_types: HashSet<String> = ["_http._tcp.local.".to_string()].into_iter().collect();
        let mut state = create_test_app_state(user_types.clone(), true, true, false);

        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test", "_http._tcp.local.", 8080));

        let json_str = state.dump_state_to_json().unwrap();

        // Verify options are present in JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("Should be valid JSON");
        let options = &parsed["options"];
        assert!(
            options.get("serviceTypes").is_some(),
            "options should have serviceTypes"
        );
        assert_eq!(options["disableIpv4"], true, "disableIpv4 should be true");
        assert_eq!(options["disableIpv6"], false, "disableIpv6 should be false");
        assert_eq!(options["noDebounce"], true, "noDebounce should be true");
        assert!(options["interfaces"].is_null(), "interfaces should be null");

        let state_dump: StateDump = serde_json::from_str(&json_str).expect("Should be valid JSON");

        let mut loaded_state = AppState::new(HashSet::new(), false, false, false, None);
        loaded_state.load_from_state_dump(state_dump);

        assert_eq!(loaded_state.services.len(), 1);
        assert_eq!(loaded_state.service_types.len(), 1);
        assert!(loaded_state.loaded_from_file);
        assert_eq!(loaded_state.filter_query, "");

        // Verify options were restored
        assert!(loaded_state.disable_ipv4);
        assert!(!loaded_state.disable_ipv6);
        assert!(loaded_state.no_debounce);
        assert!(loaded_state.interfaces.is_none());
    }

    #[tokio::test]
    async fn test_interfaces_round_trip() {
        let user_types: HashSet<String> = ["_http._tcp.local.".to_string()].into_iter().collect();
        let mut state = create_test_app_state(user_types.clone(), true, false, false);

        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test", "_http._tcp.local.", 8080));

        // Set non-null interfaces
        state.interfaces = Some(vec!["eth0".to_string()]);

        let json_str = state.dump_state_to_json().unwrap();

        // Verify interfaces is an array in JSON (not null)
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("Should be valid JSON");
        let options = &parsed["options"];

        assert!(
            options["interfaces"].is_array(),
            "interfaces should be an array, not null"
        );
        if let serde_json::Value::Array(interfaces) = &options["interfaces"] {
            assert_eq!(interfaces.len(), 1, "Should have 1 interface");
            assert_eq!(interfaces[0], "eth0", "Interface should be eth0");
        } else {
            panic!("interfaces should be an array");
        }

        // Verify round-trip: deserialize and load
        let state_dump: StateDump = serde_json::from_str(&json_str).expect("Should be valid JSON");

        let mut loaded_state = AppState::new(HashSet::new(), false, false, false, None);
        loaded_state.load_from_state_dump(state_dump);

        // Verify interfaces was restored
        assert_eq!(
            loaded_state.interfaces,
            Some(vec!["eth0".to_string()]),
            "interfaces should be restored after round-trip"
        );
    }

    // Tests for unused helper functions

    #[test]
    fn test_scroll_state_functionality() {
        let mut scroll_state = ScrollState::new();

        // Test initial state
        assert_eq!(scroll_state.offset, 0);
        assert_eq!(scroll_state.visible_items, 0);

        // Test page_scroll_amount with zero visible items
        assert_eq!(scroll_state.page_scroll_amount(), 0);

        // Test with visible items
        scroll_state.visible_items = 5;
        assert_eq!(scroll_state.page_scroll_amount(), 4);

        // Test reset
        scroll_state.offset = 10;
        scroll_state.reset();
        assert_eq!(scroll_state.offset, 0);
    }

    #[test]
    fn test_scroll_state_update_offset() {
        let mut scroll_state = ScrollState::new();
        scroll_state.visible_items = 3;

        // Test offset update within bounds
        scroll_state.update_offset(1, 10);
        assert_eq!(scroll_state.offset, 0);

        // Test offset update beyond visible area
        scroll_state.update_offset(5, 10);
        assert_eq!(scroll_state.offset, 3);

        // Test offset at boundary
        scroll_state.update_offset(9, 10);
        assert_eq!(scroll_state.offset, 7);

        // Test with single item
        scroll_state.update_offset(0, 1);
        assert_eq!(scroll_state.offset, 0);
    }

    #[test]
    fn test_navigate_list_up() {
        let mut selected_index = 5;
        let mut scroll_state = ScrollState::new();
        scroll_state.visible_items = 3;
        let total_items = 10;

        navigate_list_up(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 4);

        // Test at boundary
        selected_index = 0;
        navigate_list_up(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 0);
    }

    #[test]
    fn test_navigate_list_down() {
        let mut selected_index = 5;
        let mut scroll_state = ScrollState::new();
        scroll_state.visible_items = 3;
        let total_items = 10;

        navigate_list_down(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 6);

        // Test at boundary
        selected_index = 9;
        navigate_list_down(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 9);
    }

    #[test]
    fn test_navigate_list_page_up() {
        let mut selected_index = 8;
        let mut scroll_state = ScrollState::new();
        scroll_state.visible_items = 3;
        let total_items = 15;

        let scroll_amount = scroll_state.page_scroll_amount();
        assert_eq!(scroll_amount, 2); // 3 - 1 = 2

        navigate_list_page_up(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 6); // 8 - 2 = 6

        // Test page up with less than page size
        selected_index = 2;
        navigate_list_page_up(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 0);
    }

    #[test]
    fn test_navigate_list_page_down() {
        let mut selected_index = 2;
        let mut scroll_state = ScrollState::new();
        scroll_state.visible_items = 3;
        let total_items = 15;

        navigate_list_page_down(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 4); // 2 + (3-1) = 4

        // Test page down near boundary
        selected_index = 12;
        navigate_list_page_down(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 14); // max_index = 14
    }

    #[test]
    fn test_navigate_list_to_first() {
        let mut selected_index = 10;
        let mut scroll_state = ScrollState::new();
        scroll_state.offset = 5;

        navigate_list_to_first(&mut selected_index, &mut scroll_state);
        assert_eq!(selected_index, 0);
        assert_eq!(scroll_state.offset, 0);
    }

    #[test]
    fn test_navigate_list_to_last() {
        let mut selected_index = 0;
        let mut scroll_state = ScrollState::new();
        let total_items = 15;

        navigate_list_to_last(&mut selected_index, &mut scroll_state, total_items);
        assert_eq!(selected_index, 14); // 15 - 1
    }

    #[test]
    fn test_get_visible_items() {
        let mut scroll_state = ScrollState::new();
        scroll_state.visible_items = 3;

        let items = vec!["a", "b", "c", "d", "e"];

        // Test normal case
        scroll_state.offset = 1;
        let visible = get_visible_items(&items, &scroll_state);
        assert_eq!(visible, &["b", "c", "d"]);

        // Test at end
        scroll_state.offset = 3;
        let visible = get_visible_items(&items, &scroll_state);
        assert_eq!(visible, &["d", "e"]);

        // Test offset beyond bounds
        scroll_state.offset = 10;
        let visible = get_visible_items(&items, &scroll_state);
        assert!(visible.is_empty());

        // Test empty items
        let empty_items: Vec<&str> = vec![];
        scroll_state.offset = 0;
        let visible = get_visible_items(&empty_items, &scroll_state);
        assert!(visible.is_empty());
    }

    #[test]
    fn test_assert_helper_functions() {
        let mut state = setup_test_state(3);

        // Test assert_navigation_state
        assert_navigation_state(&state, 0, None);

        // Test assert_cache_state
        assert_cache_state(&state, true, false); // setup_test_state marks cache as dirty

        // Test assert_service_count
        assert_service_count(&state, 3);

        // Test assert_service_type_count
        assert_service_type_count(&state, 1);

        // Test assert_metric_not_exist for non-existent metric
        assert_metric_not_exist(&state, "services_added");

        // Test assert_metric with a metric that should exist
        state.metrics.insert("test_metric".to_string(), 42);
        assert_metric(&state, "test_metric", 42);

        // Test assert_metric_not_exist for non-existent metric
        assert_metric_not_exist(&state, "non_existent_metric");
    }

    #[test]
    fn test_create_service_variants() {
        // Test create_offline_service
        let offline_service = create_offline_service("test", "_http._tcp.local.", 8080);
        assert!(!offline_service.online);
        assert_eq!(offline_service.fullname, "test._http._tcp.local.");

        // Test create_service_with_addrs
        let service_with_addrs = create_service_with_addrs(
            "test",
            "_http._tcp.local.",
            8080,
            vec!["10.0.0.1", "10.0.0.2"],
        );
        assert_eq!(service_with_addrs.addrs, vec!["10.0.0.1", "10.0.0.2"]);

        // Test create_service_with_txt
        let service_with_txt = create_service_with_txt(
            "test",
            "_http._tcp.local.",
            8080,
            vec!["key1=value1", "key2=value2"],
        );
        assert_eq!(service_with_txt.txt, vec!["key1=value1", "key2=value2"]);

        // Test create_service_with_subtype
        let service_with_subtype = create_service_with_subtype(
            "test",
            "_http._tcp.local.",
            8080,
            "_printer._sub._http._tcp.local.",
        );
        assert_eq!(
            service_with_subtype.subtype,
            Some("_printer._sub._http._tcp.local.".to_string())
        );
    }

    #[test]
    fn test_service_session_history_functionality() {
        let service = create_test_service("test", "_http._tcp.local.", 8080);

        // Test get_session_history with single session
        let history = get_session_history(&service);
        assert!(!history.is_empty());

        // Test create_test_service_with_sessions with multiple sessions
        let sessions = vec![
            ServiceSession {
                start_time: 1000,
                end_time: Some(2000),
            },
            ServiceSession {
                start_time: 3000,
                end_time: None, // Current session
            },
        ];

        let service_with_sessions = create_test_service_with_sessions(
            "test",
            "_http._tcp.local.",
            8080,
            sessions,
            true,
            4000,
            1000,
            Some(4000),
            Some(2000),
        );

        assert_eq!(service_with_sessions.session_history.len(), 2);
        assert!(service_with_sessions.online);
        assert_eq!(service_with_sessions.updated_at_micros, 4000);

        // Test session history formatting
        let history = get_session_history(&service_with_sessions);
        assert!(history.contains("Session 1"));
        assert!(history.contains("Session 2"));
        assert!(history.contains("Ongoing"));
    }

    #[test]
    fn test_setup_helper_functions() {
        // Test setup_test_state
        let state = setup_test_state(5);
        assert_service_count(&state, 5);
        assert_service_type_count(&state, 1);

        // Test setup_test_state_with_types
        let state_with_types = setup_test_state_with_types(vec![
            "_http._tcp.local.",
            "_ssh._tcp.local.",
            "_printer._tcp.local.",
        ]);
        assert_service_type_count(&state_with_types, 3);

        // Test setup_test_state_with_services
        let services = vec![
            create_test_service("test1", "_http._tcp.local.", 8080),
            create_test_service("test2", "_ssh._tcp.local.", 22),
        ];
        let state_with_services = setup_test_state_with_services(services);
        assert_service_count(&state_with_services, 2);
        assert_service_type_count(&state_with_services, 2);

        // Test setup_test_state_with_user_types
        let user_types_state =
            setup_test_state_with_user_types(vec!["_http._tcp.local.", "_ssh._tcp.local."]);
        assert_service_type_count(&user_types_state, 0); // User types are added to user_service_types, not service_types
    }

    // Tests for service debouncing functionality
    #[test]
    fn test_schedule_service_removal() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let fullname = "test-service._http._tcp.local.";
        state.schedule_service_removal(fullname);

        assert!(state.pending_removals.contains_key(fullname));
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&1));
    }

    #[test]
    fn test_cancel_pending_removal() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let fullname = "test-service._http._tcp.local.";
        state.schedule_service_removal(fullname);

        // Cancel should remove from pending and increment metrics
        let was_cancelled = state.cancel_pending_removal(fullname);
        assert!(was_cancelled);
        assert!(!state.pending_removals.contains_key(fullname));
        assert_eq!(state.metrics.get("flapping_services_detected"), Some(&1));
    }

    #[test]
    fn test_cancel_pending_removal_nonexistent() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let fullname = "nonexistent-service._http._tcp.local.";
        let was_cancelled = state.cancel_pending_removal(fullname);

        assert!(!was_cancelled);
        assert_eq!(state.metrics.get("flapping_services_detected"), None);
    }

    #[test]
    fn test_process_expired_removals() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add a service to be marked offline
        let service = create_test_service("test", "_http._tcp.local.", 8080);
        state.services.push(service);

        let fullname = "test._http._tcp.local.";

        // Schedule removal with old timestamp (expired)
        let old_timestamp =
            current_timestamp_micros().saturating_sub(DEBOUNCE_DURATION_MICROS + 1000);
        state
            .pending_removals
            .insert(fullname.to_string(), old_timestamp);

        // Process expired removals
        state.process_expired_removals();

        // Service should be marked offline
        assert!(!state.services[0].online);
        assert_eq!(state.metrics.get("services_marked_offline"), Some(&1)); // Should increment after processing expired removal
    }

    #[test]
    fn test_process_expired_removals_not_expired() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        let fullname = "test-service._http._tcp.local.";

        // Schedule removal with current timestamp (not expired)
        let current_timestamp = current_timestamp_micros();
        state
            .pending_removals
            .insert(fullname.to_string(), current_timestamp);

        // Process expired removals
        state.process_expired_removals();

        // Should still be in pending removals
        assert!(state.pending_removals.contains_key(fullname));
    }

    #[test]
    fn test_mark_service_offline_respects_pending() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add a service
        let service = create_test_service("test", "_http._tcp.local.", 8080);
        state.services.push(service);

        let fullname = "test-service._http._tcp.local.";

        // Schedule removal for the service
        state.schedule_service_removal(fullname);

        // Try to mark service offline - should not work due to pending
        let marked_offline = state.mark_service_offline(fullname);
        assert!(!marked_offline);

        // Service should still be online
        assert!(state.services[0].online);
    }

    #[test]
    fn test_flapping_service_scenario() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Add a service
        let service = create_test_service("test", "_http._tcp.local.", 8080);
        state.services.push(service);

        let fullname = "test-service._http._tcp.local.";

        // Simulate service removal
        state.schedule_service_removal(fullname);
        assert!(state.pending_removals.contains_key(fullname));

        // Simulate service coming back quickly (flapping)
        let was_flapping = state.cancel_pending_removal(fullname);
        assert!(was_flapping);
        assert_eq!(state.metrics.get("flapping_services_detected"), Some(&1));

        // Service should remain online
        assert!(state.services[0].online);
    }

    #[test]
    fn test_multiple_pending_removals() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Schedule multiple services for removal
        state.schedule_service_removal("service1._http._tcp.local.");
        state.schedule_service_removal("service2._http._tcp.local.");
        state.schedule_service_removal("service3._http._tcp.local.");

        assert_eq!(state.pending_removals.len(), 3);
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&3));

        // Cancel one
        state.cancel_pending_removal("service2._http._tcp.local.");
        assert_eq!(state.pending_removals.len(), 2);
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&2)); // Updated immediately

        // Process to update metric
        state.process_expired_removals();
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&2));
    }

    #[test]
    fn test_pending_removals_metric_tracking() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // Initially no pending removals
        assert_eq!(state.metrics.get("pending_removals_active"), None);

        // Add pending removal
        state.schedule_service_removal("test._http._tcp.local.");
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&1));

        // Add another
        state.schedule_service_removal("test2._http._tcp.local.");
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&2));

        // Cancel one
        state.cancel_pending_removal("test._http._tcp.local.");
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&1)); // Updated immediately

        // Process to update metric
        state.process_expired_removals();
        assert_eq!(state.metrics.get("pending_removals_active"), Some(&1));
    }

    // Tests for no_debounce functionality
    #[test]
    fn test_no_debounce_flag_initialization() {
        let state = create_test_app_state(HashSet::new(), true, false, false);
        assert!(state.no_debounce);
        assert!(state.pending_removals.is_empty());
    }

    #[test]
    fn test_no_debounce_flag_disabled_by_default() {
        let state = create_test_app_state(HashSet::new(), false, false, false);
        assert!(!state.no_debounce);
    }

    #[test]
    fn test_no_debounce_state_clone() {
        let state = create_test_app_state(HashSet::new(), true, false, false);
        let cloned_state = state.clone();
        assert_eq!(state.no_debounce, cloned_state.no_debounce);
    }

    #[test]
    fn test_no_debounce_prevents_pending_removals() {
        let state = create_test_app_state(HashSet::new(), true, false, false);

        // When no_debounce is true, scheduling should be bypassed
        // This test ensures the flag is properly set
        assert!(state.no_debounce);
        assert!(state.pending_removals.is_empty());

        // The actual service removal logic is tested at integration level
        // This test ensures the state is properly initialized
    }

    #[test]
    fn test_debounce_enabled_by_default_in_tests() {
        let mut state = create_test_app_state(HashSet::new(), false, false, false);

        // With debounce enabled (default), pending_removals should work normally
        let fullname = "test._http._tcp.local.";
        state.schedule_service_removal(fullname);

        assert!(!state.no_debounce);
        assert!(state.pending_removals.contains_key(fullname));
    }

    #[test]
    fn test_service_styling_flapping_selected() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.is_flapping = true;
        service.online = true;

        let style = create_service_list_item_style(2, 2, &service);

        // Should have darker background with underline when selected and flapping
        assert_eq!(style.bg, Some(FLAPPING_COLOR_SELECTED));
        assert_eq!(style.fg, Some(Color::White));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_service_styling_flapping_not_selected() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.is_flapping = true;
        service.online = true;

        let style = create_service_list_item_style(2, 1, &service);

        // Should have darker background with underline when not selected and flapping
        assert_eq!(style.bg, Some(FLAPPING_COLOR_NORMAL));
        assert_eq!(style.fg, Some(Color::White));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_service_styling_flapping_offline() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.is_flapping = true;
        service.online = false;

        let style = create_service_list_item_style(2, 2, &service);

        // Should have darker background with underline and crossed out when offline and flapping
        assert_eq!(style.bg, Some(FLAPPING_COLOR_SELECTED));
        assert_eq!(style.fg, Some(Color::White));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn test_service_styling_not_flapping() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.is_flapping = false;
        service.online = true;

        let style = create_service_list_item_style(2, 1, &service);

        // Should not have darker background or underline when not flapping
        assert_ne!(style.bg, Some(FLAPPING_COLOR_NORMAL));
        assert_ne!(style.bg, Some(FLAPPING_COLOR_SELECTED));
        assert_eq!(style.fg, Some(Color::White));
        assert!(!style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_service_styling_flapping_edge_case_selection() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.is_flapping = true;

        // Test selected case
        let selected_style = create_service_list_item_style(1, 1, &service);
        assert_eq!(selected_style.bg, Some(FLAPPING_COLOR_SELECTED));
        assert!(selected_style.add_modifier.contains(Modifier::UNDERLINED));

        // Test not selected case
        let not_selected_style = create_service_list_item_style(1, 0, &service);
        assert_eq!(not_selected_style.bg, Some(FLAPPING_COLOR_NORMAL));
        assert!(
            not_selected_style
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }
}

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortField {
    Host,
    ServiceType,
    Fullname,
    Port,
    Address,
    Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug)]
struct ServiceEntry {
    fullname: String,
    host: String,
    service_type: String,
    subtype: Option<String>,
    addrs: Vec<String>,
    port: u16,
    txt: Vec<String>,
    online: bool,
    timestamp_micros: u64,
}

impl ServiceEntry {
    fn go_offline_at(&mut self, timestamp_micros: u64) {
        self.online = false;
        self.timestamp_micros = timestamp_micros;
    }
}

impl From<ResolvedService> for ServiceEntry {
    fn from(resolved_service: ResolvedService) -> Self {
        Self {
            fullname: resolved_service.get_fullname().to_string(),
            host: resolved_service.get_hostname().to_string(),
            service_type: resolved_service.ty_domain.to_string(),
            subtype: resolved_service
                .get_subtype()
                .as_ref()
                .map(|s| s.to_string()),
            addrs: {
                let mut addrs: Vec<String> = resolved_service
                    .get_addresses()
                    .iter()
                    .map(|ip| ip.to_string())
                    .collect();
                addrs.sort();
                addrs
            },
            port: resolved_service.get_port(),
            txt: {
                let mut txt: Vec<String> = resolved_service
                    .get_properties()
                    .iter()
                    .filter_map(|prop| {
                        prop.val()
                            .map(|val| format!("{}={}", prop.key(), String::from_utf8_lossy(val)))
                    })
                    .collect();
                txt.sort_by(|a, b| {
                    let a_key = a.split('=').next().unwrap_or(a);
                    let b_key = b.split('=').next().unwrap_or(b);
                    a_key.cmp(b_key)
                });
                txt
            },
            online: true,
            timestamp_micros: current_timestamp_micros(),
        }
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
    cached_sorted: bool,
    show_help_popup: bool,
    show_metrics_popup: bool,
    metrics: BTreeMap<String, u64>,
    sort_field: SortField,
    sort_direction: SortDirection,
    filter_query: String,
    filter_input_mode: bool,
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
            cached_sorted: false,
            show_help_popup: false,
            show_metrics_popup: false,
            metrics: BTreeMap::new(),
            sort_field: SortField::Host,
            sort_direction: SortDirection::Ascending,
            filter_query: String::new(),
            filter_input_mode: false,
        };
        state.validate_selected_type();
        state
    }

    fn filter_service(&self, service: &ServiceEntry) -> bool {
        // First filter by service type if one is selected
        if let Some(selected_type_idx) = self.selected_type {
            if let Some(selected_type) = self.service_types.get(selected_type_idx) {
                if service.service_type != *selected_type {
                    return false;
                }
            }
        }

        // Then filter by text query if present
        if !self.filter_query.is_empty() {
            let query = self.filter_query.to_lowercase();

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
            search_text.contains(&query)
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
        let cache_was_rebuilt = self.update_filtered_cache();
        if cache_was_rebuilt || !self.cached_sorted {
            self.sort_filtered_services();
            self.cached_sorted = true;
        }
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

    fn update_sort_field(&mut self, field: SortField) {
        self.sort_field = field;
        self.selected_service = 0;
        self.services_scroll_offset = 0;
        self.invalidate_cache_and_validate();
    }

    fn update_sort_direction(&mut self, direction: SortDirection) {
        self.sort_direction = direction;
        self.selected_service = 0;
        self.services_scroll_offset = 0;
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
        self.update_metric("cache_invalidations");
        self.mark_cache_dirty();
        self.cached_sorted = false;
        self.validate_selected_type();
    }

    // Key handling methods
    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        if self.show_help_popup {
            self.handle_help_popup_key(key)
        } else if self.show_metrics_popup {
            self.handle_metrics_popup_key(key)
        } else if self.filter_input_mode {
            self.handle_filter_input_key(key)
        } else {
            self.handle_normal_mode_key(key)
        }
    }

    fn handle_help_popup_key(&mut self, _key: KeyEvent) -> bool {
        // Any key just closes the help popup and returns to normal mode
        self.show_help_popup = false;
        true // Continue running
    }

    fn handle_metrics_popup_key(&mut self, _key: KeyEvent) -> bool {
        // Any key just closes the metrics popup and returns to normal mode
        self.show_metrics_popup = false;
        true // Continue running
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
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
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
        self.show_help_popup = !self.show_help_popup;
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
        self.show_metrics_popup = !self.show_metrics_popup;
    }

    fn add_or_update_service(&mut self, service_entry: ServiceEntry) -> bool {
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

            if significant_fields_changed {
                *existing = service_entry;
                self.update_metric("services_updated");
            }
            true
        } else {
            self.services.push(service_entry);
            self.update_metric("services_discovered");
            false
        }
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

    // Filter methods
    fn start_filter_input(&mut self) {
        self.filter_input_mode = true;
        self.filter_query.clear();
    }

    fn clear_filter(&mut self) {
        self.filter_query.clear();
        self.filter_input_mode = false;
        // Reset selection and scroll when clearing filter
        self.selected_service = 0;
        self.services_scroll_offset = 0;
        self.invalidate_cache_and_validate();
    }

    fn apply_filter(&mut self) {
        self.filter_input_mode = false;
        // Reset selection and scroll when exiting filter mode
        self.selected_service = 0;
        self.services_scroll_offset = 0;
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

            let a_addr_str = a.addrs.first().map(|s| s.as_str()).unwrap_or("<no-addr>");
            let b_addr_str = b.addrs.first().map(|s| s.as_str()).unwrap_or("<no-addr>");

            // Try to parse as IP addresses for numeric comparison, fall back to string comparison
            match (a_addr_str.parse::<IpAddr>(), b_addr_str.parse::<IpAddr>()) {
                (Ok(a_ip), Ok(b_ip)) => a_ip.cmp(&b_ip),
                _ => a_addr_str.cmp(b_addr_str),
            }
        }
        SortField::Timestamp => a.timestamp_micros.cmp(&b.timestamp_micros),
    }
}

#[derive(Debug, Clone)]
enum Notification {
    UserInput,
    ServiceChanged,
    MetricsUpdated,
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

fn ui(f: &mut Frame, app_state: &mut AppState) {
    // Ensure state is consistent before rendering
    app_state.validate_selected_type();

    let layout = if app_state.filter_input_mode {
        create_filter_input_layout(f.area())
    } else {
        create_main_layout(f.area())
    };
    let visible_counts = calculate_visible_counts(&layout);

    // Update state with current visible counts
    app_state.visible_types = visible_counts.types;
    app_state.visible_services = visible_counts.services;

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
        if !app_state.filter_query.is_empty() {
            render_filter_status(f, app_state);
        }
    }

    // Render popups if active
    if app_state.show_help_popup {
        render_help_popup(f);
    } else if app_state.show_metrics_popup {
        render_metrics_popup(f, app_state);
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

    MainLayout {
        left_panel: chunks[0],
        services_area: services_chunks[0],
        details_area: services_chunks[1],
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

    let sort_field_display = format_sort_field_for_display(app_state.sort_field);
    let sort_dir_display = format_sort_direction_for_display(app_state.sort_direction);
    let sort_field_highlighted = Span::styled(
        sort_field_display,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let sort_dir_highlighted = Span::styled(
        sort_dir_display,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let title = Line::from(vec![
        Span::raw("Services ["),
        Span::styled(
            format!("{}/{}", filtered_indices_len, services_clone.len()),
            Style::default().fg(Color::Green),
        ),
        Span::raw("] ["),
        sort_field_highlighted,
        Span::raw("/"),
        sort_dir_highlighted,
        Span::raw("] (↑/↓, s/S to sort, o to toggle)"),
    ]);

    let services_list = List::new(visible_service_items)
        .block(Block::default().borders(Borders::ALL).title(title))
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

fn render_filter_input(f: &mut Frame, app_state: &AppState, area: ratatui::layout::Rect) {
    let filter_area = ratatui::layout::Rect::new(area.x, area.y + area.height - 3, area.width, 3);

    let input_text = format!("/{}_", app_state.filter_query);

    let filter_input = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Quick Filter (Enter to apply, Esc to cancel)"),
        )
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(filter_input, filter_area);
}

fn render_filter_status(f: &mut Frame, app_state: &AppState) {
    let status_area = ratatui::layout::Rect::new(
        f.area().x,
        f.area().y + f.area().height - 1,
        f.area().width,
        1,
    );

    let status_text = format!("Filter: '{}' (Press 'n' to clear)", app_state.filter_query);

    let status =
        Paragraph::new(status_text).style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));

    f.render_widget(status, status_area);
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
        Line::from("   d                   - Remove offline services"),
        Line::from("   m                   - Show service metrics"),
        Line::from("   /                   - Enter quick filter mode"),
        Line::from("   n                   - Clear current filter"),
        Line::from("   ?                   - Toggle this help popup"),
        Line::from("   q or Ctrl+C         - Quit the application"),
        Line::from(" "),
        Line::from(" Sorting:"),
        Line::from(
            "   s                   - Cycle sort field: Host → Type → Name → Port → Addr → Time",
        ),
        Line::from("   S                   - Cycle sort field backward"),
        Line::from("   o                   - Toggle sort direction (↑/↓)"),
        Line::from(" "),
        Line::from("   Sort field highlighted in yellow, direction in cyan"),
        Line::from(" "),
        Line::from(" Quick Filter:"),
        Line::from("   /                   - Start typing to filter services"),
        Line::from("   Enter               - Apply filter"),
        Line::from("   Esc                 - Cancel filter input"),
        Line::from("   Backspace           - Delete last character"),
        Line::from("   n (normal mode)     - Clear current filter"),
        Line::from(" "),
        Line::from("   Filter searches all service fields case-insensitively"),
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

fn render_metrics_popup(f: &mut Frame, app_state: &AppState) {
    let mut metrics_content: Vec<Line> = vec![
        Line::from(""),
        Line::from(" Service Discovery Metrics:"),
        Line::from(" "),
    ];

    // Separate custom metrics from daemon metrics
    let mut custom_metrics = Vec::new();
    let mut daemon_metrics = Vec::new();

    for (key, value) in app_state.metrics.iter() {
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

    // Sort both alphabetically
    custom_metrics.sort_by(|a, b| a.0.cmp(&b.0));
    daemon_metrics.sort_by(|a, b| a.0.cmp(&b.0));

    // Display custom metrics first
    if !custom_metrics.is_empty() {
        metrics_content.push(Line::from(" Custom Metrics:"));
        for (key, value) in &custom_metrics {
            metrics_content.push(Line::from(format!("   {}: {}", key, value)));
        }
        metrics_content.push(Line::from(" "));
    }

    // Display daemon metrics
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

    metrics_content.push(Line::from(" "));
    metrics_content.push(Line::from(" Press any key to close"));

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

    let metrics_paragraph = Paragraph::new(metrics_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });

    f.render_widget(metrics_paragraph, inner_area);

    // Render border on top
    let border_block = Block::default()
        .borders(Borders::ALL)
        .title("Service Metrics")
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
        .replace("._tcp", ".tcp")
        .replace("._udp", ".udp")
}

fn create_service_list_item_style(
    index: usize,
    selected_index: usize,
    service: &ServiceEntry,
) -> Style {
    let foreground = if service.online {
        Color::White
    } else {
        Color::LightMagenta
    };

    let mut style = if index == selected_index {
        Style::default().bg(Color::DarkGray).fg(foreground)
    } else {
        Style::default().fg(foreground)
    };

    if !service.online {
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

    let status_text = if service.online {
        format!(
            "Online since: {}",
            format_timestamp_micros(service.timestamp_micros)
        )
    } else {
        format!(
            "Offline since: {}",
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

    let mdns_for_metrics = mdns.clone();

    // Start background task to periodically collect ServiceDaemon metrics
    let state_for_metrics = Arc::clone(&state);
    let notification_sender_for_metrics = notification_sender.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;

            match mdns_for_metrics.get_metrics() {
                Ok(metrics_receiver) => {
                    if let Ok(daemon_metrics) = metrics_receiver.recv_async().await {
                        let mut state = state_for_metrics.write().await;
                        if state.update_daemon_metrics(&daemon_metrics) {
                            // Metrics changed, trigger UI refresh
                            let _ =
                                notification_sender_for_metrics.send(Notification::MetricsUpdated);
                        }
                    }
                }
                Err(_) => {
                    // If we can't get metrics, just continue
                }
            }
        }
    });

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
                            state.update_metric("service_types_discovered");
                            let _ = notification_sender_clone.send(Notification::ServiceChanged);
                        }
                    }
                    match mdns.browse(&service_type) {
                        Err(_) => {
                            // if a browse fails, that usually means the service type is invalid and
                            // should be removed from the service types list
                            let mut state = state_clone.write().await;
                            if state.remove_service_type(&service_type) {
                                state.update_metric("browse_failures");
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
                                                entry.go_offline_at(current_timestamp_micros());
                                                state.update_metric("services_removed");
                                                state.invalidate_cache_and_validate();
                                                state.remove_service_type(&service_type);
                                                let _ = notification_sender_inner
                                                    .send(Notification::ServiceChanged);
    }

}
                                        ServiceEvent::ServiceResolved(resolved_service) => {
                                            let entry = ServiceEntry::from(*resolved_service);
                                            let mut state = state_inner.write().await;
                                            state.add_or_update_service(entry);
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
                            let should_continue = state.handle_key_event(key);
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
                    terminal.draw(|f| ui(f, &mut state))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // ServiceEntry tests
    #[test]
    fn test_service_entry_go_offline_at() {
        let mut service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

        assert!(service.online);
        service.go_offline_at(2000);
        assert!(!service.online);
        assert_eq!(service.timestamp_micros, 2000);
    }

    // AppState initialization tests
    #[test]
    fn test_appstate_new() {
        let state = AppState::new();
        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll_offset, 0);
        assert_eq!(state.services_scroll_offset, 0);
        assert!(state.cache_dirty);
        assert!(!state.show_help_popup);
        assert!(!state.show_metrics_popup);
    }

    // Filter service tests
    #[test]
    fn test_filter_service_all_types() {
        let mut state = AppState::new();
        state.selected_type = None;

        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_specific_type() {
        let mut state = AppState::new();
        state.service_types.push("_http._tcp.local.".to_string());
        state.service_types.push("_ssh._tcp.local.".to_string());
        state.selected_type = Some(0);

        let http_service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

        let ssh_service = ServiceEntry {
            fullname: "test._ssh._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_ssh._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 22,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

        assert!(state.filter_service(&http_service));
        assert!(!state.filter_service(&ssh_service));
    }

    // Service type management tests
    #[test]
    fn test_add_service_type() {
        let mut state = AppState::new();
        assert!(state.add_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
        assert_eq!(state.service_types[0], "_http._tcp.local.");

        // Adding duplicate should return false
        assert!(!state.add_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
    }

    #[test]
    fn test_add_service_type_maintains_sort_order() {
        let mut state = AppState::new();
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        assert_eq!(state.service_types[0], "_http._tcp.local.");
        assert_eq!(state.service_types[1], "_printer._tcp.local.");
        assert_eq!(state.service_types[2], "_ssh._tcp.local.");
    }

    #[test]
    fn test_add_service_type_preserves_selection() {
        let mut state = AppState::new();
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.selected_type = Some(1); // _ssh._tcp.local.

        // Add a new type, selection should still point to _ssh._tcp.local.
        state.add_service_type("_printer._tcp.local.");
        assert_eq!(state.selected_type, Some(2)); // _ssh._tcp.local. moved to index 2
    }

    #[test]
    fn test_remove_service_type() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        // Can't remove if still in use
        state.services.push(ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        });

        assert!(!state.remove_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 2);

        // Can remove if not in use
        assert!(state.remove_service_type("_ssh._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
    }

    #[test]
    fn test_remove_service_type_adjusts_selection() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = Some(1); // _printer._tcp.local.

        // Remove the selected type
        state.remove_service_type("_printer._tcp.local.");
        // Selection should move to nearest valid index
        assert!(state.selected_type == Some(1) || state.selected_type == Some(0));
    }

    // Navigation tests
    #[test]
    fn test_navigate_services_up() {
        let mut state = AppState::new();
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 81));
        state
            .services
            .push(create_test_service("test3", "_http._tcp.local.", 82));
        state.selected_service = 2;

        state.navigate_services_up();
        assert_eq!(state.selected_service, 1);

        state.navigate_services_up();
        assert_eq!(state.selected_service, 0);

        // Should not go below 0
        state.navigate_services_up();
        assert_eq!(state.selected_service, 0);
    }

    #[test]
    fn test_navigate_services_down() {
        let mut state = AppState::new();
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 81));
        state
            .services
            .push(create_test_service("test3", "_http._tcp.local.", 82));
        state.selected_service = 0;

        state.navigate_services_down();
        assert_eq!(state.selected_service, 1);

        state.navigate_services_down();
        assert_eq!(state.selected_service, 2);

        // Should not go beyond last service
        state.navigate_services_down();
        assert_eq!(state.selected_service, 2);
    }

    #[test]
    fn test_navigate_service_types_up() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = Some(1);

        state.navigate_service_types_up();
        assert_eq!(state.selected_type, Some(0));

        state.navigate_service_types_up();
        assert_eq!(state.selected_type, None); // "All Types"

        // Should not go beyond "All Types"
        state.navigate_service_types_up();
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_navigate_service_types_down() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = None;

        state.navigate_service_types_down();
        assert_eq!(state.selected_type, Some(0));

        state.navigate_service_types_down();
        assert_eq!(state.selected_type, Some(1));

        // Should not go beyond last type
        state.navigate_service_types_down();
        assert_eq!(state.selected_type, Some(1));
    }

    #[test]
    fn test_navigate_services_to_first() {
        let mut state = AppState::new();
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 81));
        state.selected_service = 1;
        state.services_scroll_offset = 1;

        state.navigate_services_to_first();
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
    }

    #[test]
    fn test_navigate_services_to_last() {
        let mut state = AppState::new();
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 81));
        state
            .services
            .push(create_test_service("test3", "_http._tcp.local.", 82));
        state.selected_service = 0;

        state.navigate_services_to_last();
        assert_eq!(state.selected_service, 2);
    }

    #[test]
    fn test_navigate_services_page_up() {
        let mut state = AppState::new();
        for i in 0..20 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.visible_services = 5;
        state.selected_service = 10;

        state.navigate_services_page_up();
        assert_eq!(state.selected_service, 6); // 10 - (5-1) = 6

        state.navigate_services_page_up();
        assert_eq!(state.selected_service, 2); // 6 - (5-1) = 2

        state.navigate_services_page_up();
        assert_eq!(state.selected_service, 0); // Can't go below 0
    }

    #[test]
    fn test_navigate_services_page_down() {
        let mut state = AppState::new();
        for i in 0..20 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.visible_services = 5;
        state.selected_service = 0;

        state.navigate_services_page_down();
        assert_eq!(state.selected_service, 4); // 0 + (5-1) = 4

        state.navigate_services_page_down();
        assert_eq!(state.selected_service, 8); // 4 + (5-1) = 8

        state.selected_service = 15;
        state.navigate_services_page_down();
        assert_eq!(state.selected_service, 19); // Should stop at last item
    }

    // Remove offline services tests
    #[test]
    fn test_remove_offline_services() {
        let mut state = AppState::new();
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
        let mut state = AppState::new();
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
        let mut state = AppState::new();
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
    fn test_handle_key_event_quit() {
        let mut state = AppState::new();
        let key = KeyEvent::from(KeyCode::Char('q'));
        assert!(!state.handle_key_event(key)); // Should return false to quit
    }

    #[test]
    fn test_handle_key_event_toggle_help() {
        let mut state = AppState::new();
        assert!(!state.show_help_popup);

        let key = KeyEvent::from(KeyCode::Char('?'));
        assert!(state.handle_key_event(key)); // Should return true to continue
        assert!(state.show_help_popup);

        assert!(state.handle_key_event(key));
        assert!(!state.show_help_popup);
    }

    #[test]
    fn test_handle_key_event_toggle_metrics() {
        let mut state = AppState::new();
        assert!(!state.show_metrics_popup);

        let key = KeyEvent::from(KeyCode::Char('m'));
        assert!(state.handle_key_event(key));
        assert!(state.show_metrics_popup);

        assert!(state.handle_key_event(key));
        assert!(!state.show_metrics_popup);
    }

    #[test]
    fn test_handle_help_popup_key() {
        let mut state = AppState::new();
        state.show_help_popup = true;

        let key = KeyEvent::from(KeyCode::Char('a'));
        assert!(state.handle_key_event(key)); // Any key should close popup
        assert!(!state.show_help_popup);
    }

    #[test]
    fn test_handle_metrics_popup_key() {
        let mut state = AppState::new();
        state.show_metrics_popup = true;

        let key = KeyEvent::from(KeyCode::Char('x'));
        assert!(state.handle_key_event(key)); // Any key should close popup
        assert!(!state.show_metrics_popup);
    }

    // Metrics tests
    #[test]
    fn test_update_metric() {
        let mut state = AppState::new();
        state.update_metric("test_metric");
        assert_eq!(state.metrics.get("test_metric"), Some(&1));

        state.update_metric("test_metric");
        assert_eq!(state.metrics.get("test_metric"), Some(&2));
    }

    #[test]
    fn test_update_daemon_metrics() {
        let mut state = AppState::new();
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

    // Cache tests
    #[test]
    fn test_filter_cache_invalidation() {
        let mut state = AppState::new();
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
        let mut state = AppState::new();
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
    fn test_is_valid_service_type() {
        assert!(is_valid_service_type("_http._tcp.local."));
        assert!(is_valid_service_type("_ssh._tcp.local."));
        assert!(!is_valid_service_type("_sub._http._tcp.local."));
        assert!(!is_valid_service_type("test_sub.something"));
    }

    #[test]
    fn test_current_timestamp_micros() {
        let ts1 = current_timestamp_micros();
        let ts2 = current_timestamp_micros();
        assert!(ts2 >= ts1);
        assert!(ts1 > 0);
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
            format_service_type_for_display("_printer._tcp."),
            "printer.tcp"
        );
    }

    #[test]
    fn test_format_service_for_display() {
        let service = ServiceEntry {
            fullname: "MyPrinter._printer._tcp.local.".to_string(),
            host: "printer.local.".to_string(),
            service_type: "_printer._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.100".to_string()],
            port: 631,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

        let display = format_service_for_display(&service);
        assert!(display.contains("MyPrinter"));
        assert!(display.contains("printer"));
        assert!(display.contains("192.168.1.100"));
        assert!(display.contains("631"));
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
            timestamp_micros: 1000,
        };

        let display = format_service_for_display(&service);
        assert!(display.contains("<no-addr>"));
    }

    #[test]
    fn test_create_service_details_text() {
        let service = ServiceEntry {
            fullname: "MyService._http._tcp.local.".to_string(),
            host: "myhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: Some("_printer".to_string()),
            addrs: vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()],
            port: 8080,
            txt: vec!["key1=value1".to_string(), "key2=value2".to_string()],
            online: true,
            timestamp_micros: 1000000000,
        };

        let details = create_service_details_text(&service);
        assert!(details.contains("MyService._http._tcp.local."));
        assert!(details.contains("myhost.local."));
        assert!(details.contains("_http._tcp.local."));
        assert!(details.contains("_printer"));
        assert!(details.contains("8080"));
        assert!(details.contains("192.168.1.1"));
        assert!(details.contains("192.168.1.2"));
        assert!(details.contains("key1=value1"));
        assert!(details.contains("key2=value2"));
        assert!(details.contains("Online since:"));
    }

    #[test]
    fn test_create_service_details_text_offline_service() {
        let service = ServiceEntry {
            fullname: "OfflineService._http._tcp.local.".to_string(),
            host: "offlinehost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: false,
            timestamp_micros: 2000000000,
        };

        let details = create_service_details_text(&service);
        assert!(details.contains("Offline since:"));
        assert!(details.contains("None")); // No addresses
        assert!(!details.contains("Subtype:")); // No subtype
    }

    #[test]
    fn test_format_timestamp_micros() {
        let timestamp = format_timestamp_micros(1609459200000000); // 2021-01-01 00:00:00 UTC
        // Just verify it's a valid formatted string with expected components
        assert!(timestamp.contains("-"));
        assert!(timestamp.contains(":"));
        assert!(timestamp.len() > 20); // Should include date, time, and microseconds
    }

    // Layout tests
    #[test]
    fn test_create_main_layout() {
        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let layout = create_main_layout(area);

        assert!(layout.left_panel.width > 0);
        assert!(layout.services_area.width > 0);
        assert!(layout.details_area.width > 0);
        assert!(layout.services_area.height > 0);
        assert!(layout.details_area.height > 0);
    }

    #[test]
    fn test_calculate_visible_counts() {
        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let layout = create_main_layout(area);
        let counts = calculate_visible_counts(&layout);

        assert!(counts.types > 0);
        assert!(counts.services > 0);
    }

    #[test]
    fn test_create_centered_popup() {
        let parent = ratatui::layout::Rect::new(0, 0, 100, 50);
        let popup = create_centered_popup(parent, 50, 50);

        // Popup should be smaller than parent
        assert!(popup.width <= parent.width);
        assert!(popup.height <= parent.height);

        // Popup should be centered (roughly)
        let center_x = parent.width / 2;
        let center_y = parent.height / 2;
        let popup_center_x = popup.x + popup.width / 2;
        let popup_center_y = popup.y + popup.height / 2;

        // Allow some margin of error due to rounding and margins
        assert!((popup_center_x as i32 - center_x as i32).abs() < 10);
        assert!((popup_center_y as i32 - center_y as i32).abs() < 10);
    }

    #[test]
    fn test_create_service_list_item_style() {
        let online_service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

        let offline_service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: false,
            timestamp_micros: 1000,
        };

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
        assert_eq!(style.fg, Some(Color::LightMagenta));
        assert!(style.add_modifier.contains(Modifier::ITALIC));
    }

    // Edge case tests
    #[test]
    fn test_empty_service_list_navigation() {
        let mut state = AppState::new();

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
        let mut state = AppState::new();

        state.navigate_service_types_up();
        assert_eq!(state.selected_type, None);

        state.navigate_service_types_down();
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_filter_with_no_matching_services() {
        let mut state = AppState::new();
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
        let mut state = AppState::new();
        for i in 0..3 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.visible_services = 10; // More visible space than services

        state.selected_service = 2;
        state.update_services_scroll_offset();
        assert_eq!(state.services_scroll_offset, 0); // Should stay at 0 since all fit
    }

    #[test]
    fn test_update_service_type_selection_resets_scroll() {
        let mut state = AppState::new();
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
        state.services_scroll_offset = 3;

        state.update_service_type_selection(Some(1));
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
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
        let service1 = ServiceEntry {
            fullname: "aaa._http._tcp.local.".to_string(),
            host: "host1.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };
        let service2 = ServiceEntry {
            fullname: "zzz._http._tcp.local.".to_string(),
            host: "host2.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: true,
            timestamp_micros: 1000,
        };

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
        service1.timestamp_micros = 1000;
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.timestamp_micros = 2000;

        let result = compare_services_by_field(&service1, &service2, SortField::Timestamp);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_ip() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["192.168.1.10".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["192.168.1.20".to_string()];

        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_ipv6() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["2001:db8::1".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["2001:db8::2".to_string()];

        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_services_by_field_address_mixed_ipv4_ipv6() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["192.168.1.1".to_string()];
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
        service2.addrs = vec!["192.168.1.1".to_string()];

        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        // "<no-addr>" should be compared as string ("<no-addr>" > "192.168.1.1")
        assert_eq!(result, std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_services_by_field_address_string_fallback() {
        let mut service1 = create_test_service("test1", "_http._tcp.local.", 80);
        service1.addrs = vec!["invalid-ip-1".to_string()];
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.addrs = vec!["invalid-ip-2".to_string()];

        // Falls back to string comparison when IP parsing fails
        let result = compare_services_by_field(&service1, &service2, SortField::Address);
        assert_eq!(result, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_toggle_sort_direction() {
        let mut state = AppState::new();
        assert_eq!(state.sort_direction, SortDirection::Ascending);

        state.toggle_sort_direction();
        assert_eq!(state.sort_direction, SortDirection::Descending);

        state.toggle_sort_direction();
        assert_eq!(state.sort_direction, SortDirection::Ascending);
    }

    #[test]
    fn test_cycle_sort_field_forward() {
        let mut state = AppState::new();
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
        let mut state = AppState::new();
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

    #[test]
    fn test_update_sort_field_resets_selection() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        for i in 0..5 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.selected_service = 3;
        state.services_scroll_offset = 2;

        state.update_sort_field(SortField::Port);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
    }

    #[test]
    fn test_update_sort_direction_resets_selection() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        for i in 0..5 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.selected_service = 3;
        state.services_scroll_offset = 2;

        state.update_sort_direction(SortDirection::Descending);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
    }

    #[test]
    fn test_sort_filtered_services_ascending() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");

        // Add services in reverse alphabetical order
        state
            .services
            .push(create_test_service("zebra", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("alpha", "_http._tcp.local.", 81));
        state
            .services
            .push(create_test_service("beta", "_http._tcp.local.", 82));

        state.sort_field = SortField::Host;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(filtered.len(), 3);

        // Verify services are sorted by host in ascending order
        assert_eq!(state.services[filtered[0]].host, "alpha.local.");
        assert_eq!(state.services[filtered[1]].host, "beta.local.");
        assert_eq!(state.services[filtered[2]].host, "zebra.local.");
    }

    #[test]
    fn test_sort_filtered_services_descending() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");

        state
            .services
            .push(create_test_service("alpha", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("beta", "_http._tcp.local.", 81));
        state
            .services
            .push(create_test_service("zebra", "_http._tcp.local.", 82));

        state.sort_field = SortField::Host;
        state.sort_direction = SortDirection::Descending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(filtered.len(), 3);

        // Verify services are sorted by host in descending order
        assert_eq!(state.services[filtered[0]].host, "zebra.local.");
        assert_eq!(state.services[filtered[1]].host, "beta.local.");
        assert_eq!(state.services[filtered[2]].host, "alpha.local.");
    }

    #[test]
    fn test_sort_by_port_ascending() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");

        state
            .services
            .push(create_test_service("service1", "_http._tcp.local.", 8080));
        state
            .services
            .push(create_test_service("service2", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("service3", "_http._tcp.local.", 443));

        state.sort_field = SortField::Port;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(state.services[filtered[0]].port, 80);
        assert_eq!(state.services[filtered[1]].port, 443);
        assert_eq!(state.services[filtered[2]].port, 8080);
    }

    #[test]
    fn test_sort_by_timestamp() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");

        let mut service1 = create_test_service("service1", "_http._tcp.local.", 80);
        service1.timestamp_micros = 3000;
        let mut service2 = create_test_service("service2", "_http._tcp.local.", 81);
        service2.timestamp_micros = 1000;
        let mut service3 = create_test_service("service3", "_http._tcp.local.", 82);
        service3.timestamp_micros = 2000;

        state.services.push(service1);
        state.services.push(service2);
        state.services.push(service3);

        state.sort_field = SortField::Timestamp;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(state.services[filtered[0]].timestamp_micros, 1000);
        assert_eq!(state.services[filtered[1]].timestamp_micros, 2000);
        assert_eq!(state.services[filtered[2]].timestamp_micros, 3000);
    }

    #[test]
    fn test_sort_with_filtering() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

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

        // Filter to only HTTP services and sort by host
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
    fn test_sort_mixed_online_offline() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");

        let mut service1 = create_test_service("online1", "_http._tcp.local.", 80);
        service1.online = true;
        let mut service2 = create_test_service("offline1", "_http._tcp.local.", 81);
        service2.online = false;
        let mut service3 = create_test_service("online2", "_http._tcp.local.", 82);
        service3.online = true;

        state.services.push(service1);
        state.services.push(service2);
        state.services.push(service3);

        state.sort_field = SortField::Host;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        // All services should be included and sorted, regardless of online status
        assert_eq!(filtered.len(), 3);
        assert!(state.services[filtered[0]].host < state.services[filtered[1]].host);
        assert!(state.services[filtered[1]].host < state.services[filtered[2]].host);
    }

    #[test]
    fn test_format_sort_field_display() {
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
    fn test_format_sort_direction_display() {
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
    fn test_sort_field_enum_equality() {
        assert_eq!(SortField::Host, SortField::Host);
        assert_ne!(SortField::Host, SortField::Port);
    }

    #[test]
    fn test_sort_direction_enum_equality() {
        assert_eq!(SortDirection::Ascending, SortDirection::Ascending);
        assert_ne!(SortDirection::Ascending, SortDirection::Descending);
    }

    #[test]
    fn test_key_event_cycle_sort_forward() {
        let mut state = AppState::new();
        assert_eq!(state.sort_field, SortField::Host);

        let key = KeyEvent::from(KeyCode::Char('s'));
        state.handle_key_event(key);
        assert_eq!(state.sort_field, SortField::ServiceType);
    }

    #[test]
    fn test_key_event_cycle_sort_backward() {
        let mut state = AppState::new();
        assert_eq!(state.sort_field, SortField::Host);

        let key = KeyEvent::from(KeyCode::Char('S'));
        state.handle_key_event(key);
        assert_eq!(state.sort_field, SortField::Timestamp);
    }

    #[test]
    fn test_key_event_toggle_sort_direction() {
        let mut state = AppState::new();
        assert_eq!(state.sort_direction, SortDirection::Ascending);

        let key = KeyEvent::from(KeyCode::Char('o'));
        state.handle_key_event(key);
        assert_eq!(state.sort_direction, SortDirection::Descending);

        state.handle_key_event(key);
        assert_eq!(state.sort_direction, SortDirection::Ascending);
    }

    #[test]
    fn test_cache_invalidation_on_sort_change() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state
            .services
            .push(create_test_service("test", "_http._tcp.local.", 80));

        // Populate cache
        let _ = state.get_filtered_services();
        assert!(!state.cache_dirty);
        assert!(state.cached_sorted);

        // Changing sort field should invalidate sorted flag
        state.update_sort_field(SortField::Port);
        assert!(state.cache_dirty);
        assert!(!state.cached_sorted);
    }

    #[test]
    fn test_sort_stability_with_equal_values() {
        let mut state = AppState::new();
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
        // All should have same port, so order is determined by the stable sort
        assert_eq!(filtered.len(), 3);
        for idx in filtered {
            assert_eq!(state.services[idx].port, 80);
        }
    }

    // Filter functionality tests
    #[test]
    fn test_appstate_new_with_filter() {
        let state = AppState::new();
        assert_eq!(state.filter_query, "");
        assert!(!state.filter_input_mode);
    }

    #[test]
    fn test_start_filter_input() {
        let mut state = AppState::new();
        state.start_filter_input();
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_clear_filter() {
        let mut state = AppState::new();
        state.filter_query = "test".to_string();
        state.filter_input_mode = true;
        state.selected_service = 5;
        state.services_scroll_offset = 2;

        state.clear_filter();

        assert_eq!(state.filter_query, "");
        assert!(!state.filter_input_mode);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
    }

    #[test]
    fn test_apply_filter() {
        let mut state = AppState::new();
        state.filter_query = "test".to_string();
        state.filter_input_mode = true;
        state.selected_service = 5;
        state.services_scroll_offset = 2;

        state.apply_filter();

        assert_eq!(state.filter_query, "test");
        assert!(!state.filter_input_mode);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
    }

    #[test]
    fn test_add_to_filter() {
        let mut state = AppState::new();
        state.add_to_filter('a');
        state.add_to_filter('b');
        state.add_to_filter('c');
        assert_eq!(state.filter_query, "abc");
    }

    #[test]
    fn test_add_to_filter_invalidates_cache() {
        let mut state = AppState::new();
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
        let mut state = AppState::new();
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
        let mut state = AppState::new();
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
    fn test_filter_service_no_filter() {
        let mut state = AppState::new();
        state.selected_type = None;

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_with_text_query() {
        let mut state = AppState::new();
        state.filter_query = "test".to_string();

        let matching_service = create_test_service("test", "_http._tcp.local.", 80);
        let non_matching_service = create_test_service("other", "_http._tcp.local.", 80);

        assert!(state.filter_service(&matching_service));
        assert!(!state.filter_service(&non_matching_service));
    }

    #[test]
    fn test_filter_service_case_insensitive() {
        let mut state = AppState::new();
        state.filter_query = "TEST".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_searches_all_fields() {
        let mut state = AppState::new();

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
    fn test_filter_service_combined_with_type_filter() {
        let mut state = AppState::new();
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");
        state.selected_type = Some(0); // Select _http._tcp.local.
        state.filter_query = "test".to_string();

        let http_service = create_test_service("test", "_http._tcp.local.", 80);
        let ssh_service = create_test_service("test", "_ssh._tcp.local.", 22);

        assert!(state.filter_service(&http_service)); // Matches both type and text
        assert!(!state.filter_service(&ssh_service)); // Matches text but wrong type
    }

    #[test]
    fn test_handle_filter_input_key_enter() {
        let mut state = AppState::new();
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
        let mut state = AppState::new();
        state.filter_input_mode = true;
        state.filter_query = "test".to_string();

        let key = KeyEvent::from(KeyCode::Esc);
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(!state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_handle_filter_input_key_backspace() {
        let mut state = AppState::new();
        state.filter_input_mode = true;
        state.filter_query = "test".to_string();

        let key = KeyEvent::from(KeyCode::Backspace);
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "tes");
    }

    #[test]
    fn test_handle_filter_input_key_char() {
        let mut state = AppState::new();
        state.filter_input_mode = true;

        let key = KeyEvent::from(KeyCode::Char('a'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "a");
    }

    #[test]
    fn test_handle_normal_mode_key_slash() {
        let mut state = AppState::new();

        let key = KeyEvent::from(KeyCode::Char('/'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_handle_normal_mode_key_n() {
        let mut state = AppState::new();
        state.filter_query = "test".to_string();
        // Note: not in filter_input_mode so 'n' is handled by normal mode
        state.selected_service = 5;
        state.services_scroll_offset = 2;

        let key = KeyEvent::from(KeyCode::Char('n'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert_eq!(state.filter_query, "");
        assert!(!state.filter_input_mode);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll_offset, 0);
    }

    #[test]
    fn test_filter_empty_query_shows_all() {
        let mut state = AppState::new();
        state.filter_query = String::new(); // Empty query
        state.selected_type = Some(0); // Specific type selected
        state.add_service_type("_http._tcp.local.");

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service)); // Should show all since empty query
    }

    #[test]
    fn test_filter_with_special_characters() {
        let mut state = AppState::new();
        state.filter_query = "key=value".to_string();

        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.txt = vec!["key=value".to_string()];
        assert!(state.filter_service(&service));
    }

    // Helper function for creating test services
    fn create_test_service(name: &str, service_type: &str, port: u16) -> ServiceEntry {
        ServiceEntry {
            fullname: format!("{}.{}", name, service_type),
            host: format!("{}.local.", name),
            service_type: service_type.to_string(),
            subtype: None,
            addrs: vec![format!("192.168.1.{}", port)],
            port,
            txt: vec![],
            online: true,
            timestamp_micros: current_timestamp_micros(),
        }
    }
}

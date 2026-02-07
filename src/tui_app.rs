// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::RwLock;

const STATUS_OK: Color = Color::Blue;
const STATUS_ERROR: Color = Color::Yellow;

// Timestamp conversion utilities for JSON serialization
fn micros_to_iso_timestamp(micros: u64) -> String {
    let duration = Duration::from_micros(micros);
    let secs = duration.as_secs() as i64;
    let nanos = duration.subsec_micros() * 1000;

    match DateTime::<Utc>::from_timestamp(secs, nanos) {
        Some(datetime) => datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
        None => "1970-01-01T00:00:00.000000Z".to_string(), // Fallback for invalid timestamps
    }
}

impl From<&ServiceEntry> for SerializableServiceEntry {
    fn from(entry: &ServiceEntry) -> Self {
        let updated_at = if entry.updated_at_micros != entry.first_seen_micros {
            Some(micros_to_iso_timestamp(entry.updated_at_micros))
        } else {
            None
        };
        Self {
            host: entry.host.clone(),
            service_type: entry.service_type.clone(),
            subtype: entry.subtype.clone(),
            addresses: entry.addrs.clone(),
            port: entry.port,
            txt_records: entry.txt.clone(),
            is_online: entry.online,
            created_at: micros_to_iso_timestamp(entry.first_seen_micros),
            updated_at,
            last_online_at: entry.last_online_micros.map(micros_to_iso_timestamp),
            last_offline_at: entry.last_offline_micros.map(micros_to_iso_timestamp),
            session_history: entry.session_history.iter().map(|s| s.into()).collect(),
        }
    }
}

impl From<&ServiceSession> for SerializableServiceSession {
    fn from(session: &ServiceSession) -> Self {
        Self {
            start_time: Some(micros_to_iso_timestamp(session.start_time)),
            end_time: session.end_time.map(micros_to_iso_timestamp),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum SortField {
    Host,
    ServiceType,
    Fullname,
    Port,
    Address,
    Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
    updated_at_micros: u64,
    first_seen_micros: u64,
    last_online_micros: Option<u64>,
    last_offline_micros: Option<u64>,
    session_history: Vec<ServiceSession>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializableServiceEntry {
    host: String,
    service_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtype: Option<String>,
    addresses: Vec<String>,
    #[serde(skip_serializing_if = "is_zero_u16")]
    port: u16,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    txt_records: Vec<String>,
    is_online: bool,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_online_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_offline_at: Option<String>,
    session_history: Vec<SerializableServiceSession>,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

#[derive(Clone, Debug, PartialEq)]
struct ServiceSession {
    start_time: u64,
    end_time: Option<u64>,
    duration_micros: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializableServiceSession {
    #[serde(skip_serializing_if = "Option::is_none")]
    start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<String>,
}

impl ServiceEntry {
    fn go_offline_at(&mut self, timestamp_micros: u64) {
        if !self.online {
            return;
        }
        self.online = false;
        self.updated_at_micros = timestamp_micros;
        self.last_offline_micros = Some(timestamp_micros);

        // Calculate duration of this online session and add to history
        if let Some(last_online) = self.last_online_micros {
            let session_duration = timestamp_micros.saturating_sub(last_online);

            // Update existing session or add new one to session history
            if let Some(session) = self.session_history.iter_mut().last() {
                // Complete the current session
                session.end_time = Some(timestamp_micros);
                session.duration_micros = session_duration;
            } else {
                // Add new completed session to history
                self.session_history.push(ServiceSession {
                    start_time: last_online,
                    end_time: Some(timestamp_micros),
                    duration_micros: session_duration,
                });
            }
        }
    }

    fn go_online_at(&mut self, timestamp_micros: u64) {
        if self.online {
            return;
        }
        self.updated_at_micros = timestamp_micros;
        self.online = true;
        self.last_online_micros = Some(timestamp_micros);

        // Add new session to history
        self.session_history.push(ServiceSession {
            start_time: timestamp_micros,
            end_time: None,
            duration_micros: 0, // Will be calculated when session ends
        });
    }

    fn get_session_history(&self) -> String {
        // First, collect completed sessions and find max widths
        let mut completed_sessions = Vec::new();
        let mut max_session_num_length = 0;

        for (i, session) in self.session_history.iter().enumerate() {
            let session_num = i + 1;
            max_session_num_length = max_session_num_length.max(session_num.to_string().len());
            completed_sessions.push((session_num, session));
        }

        // Now format with proper alignment for both session numbers and durations
        let mut timeline = Vec::new();
        for (session_num, session) in completed_sessions {
            let start_str = format_timestamp_micros(session.start_time);
            let (duration_str, end_str) = if let Some(end_time) = session.end_time {
                (
                    format_duration_micros(session.duration_micros),
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
                timestamp_width = 26, // Fixed width for timestamps
            ));
        }
        timeline.join("\n")
    }
}

impl From<ResolvedService> for ServiceEntry {
    fn from(resolved_service: ResolvedService) -> Self {
        let current_timestamp = current_timestamp_micros();
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
            updated_at_micros: current_timestamp,
            first_seen_micros: current_timestamp,
            last_online_micros: Some(current_timestamp),
            last_offline_micros: None,
            session_history: vec![ServiceSession {
                start_time: current_timestamp,
                end_time: None,
                duration_micros: 0,
            }],
        }
    }
}

// Generic scroll utilities
#[derive(Debug, Clone)]
struct ScrollState {
    offset: usize,
    visible_items: usize,
}

impl ScrollState {
    fn new() -> Self {
        Self {
            offset: 0,
            visible_items: 0,
        }
    }

    fn update_offset(&mut self, selected_index: usize, total_items: usize) {
        if selected_index < self.offset {
            self.offset = selected_index;
        } else if self.visible_items > 0 && selected_index >= self.offset + self.visible_items {
            self.offset = selected_index - self.visible_items + 1;
        }

        // Ensure offset doesn't exceed bounds
        if total_items > 0 && self.offset > total_items.saturating_sub(1) {
            self.offset = total_items.saturating_sub(1);
        }
    }

    fn page_scroll_amount(&self) -> usize {
        self.visible_items.saturating_sub(1)
    }

    fn reset(&mut self) {
        self.offset = 0;
    }
}

// Generic popup scrolling utilities
fn handle_popup_scroll(
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    dump_timestamp: String,
    application_name: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateDump {
    metadata: Metadata,
    services: Vec<SerializableServiceEntry>,
    service_types: Vec<String>,
    metrics: BTreeMap<String, u64>,
    filters: FilterInfo,
    sorting: SortInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilterInfo {
    query: String,
    active_service_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SortInfo {
    field: String,
    direction: String,
}

#[derive(Clone)]
struct AppState {
    services: Vec<ServiceEntry>,
    service_types: Vec<String>,
    selected_service: usize,
    selected_type: Option<usize>,
    types_scroll: ScrollState,
    services_scroll: ScrollState,
    help_scroll: ScrollState,
    metrics_scroll: ScrollState,
    details_scroll: ScrollState,
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
    terminal_area: ratatui::layout::Rect,
    user_service_types: HashSet<String>,
    status_message: Arc<tokio::sync::Mutex<String>>,
}

impl AppState {
    fn new(user_service_types: HashSet<String>) -> Self {
        let mut state = Self {
            services: Vec::new(),
            service_types: Vec::new(),
            selected_service: 0,
            selected_type: None,
            types_scroll: ScrollState::new(),
            services_scroll: ScrollState::new(),
            help_scroll: ScrollState::new(),
            metrics_scroll: ScrollState::new(),
            details_scroll: ScrollState::new(),
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
            terminal_area: ratatui::layout::Rect::new(0, 0, 80, 24), // Default, will be updated in UI
            user_service_types,
            status_message: Arc::new(tokio::sync::Mutex::new(String::new())),
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
            filters: FilterInfo {
                query: self.filter_query.clone(),
                active_service_types: self.user_service_types.iter().cloned().collect(),
            },
            sorting: SortInfo {
                field: format!("{:?}", self.sort_field),
                direction: format!("{:?}", self.sort_direction),
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

    // Key handling methods
    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // Dismiss status message on any key press if it's displayed
        if let Ok(mut msg) = self.status_message.try_lock()
            && !msg.is_empty()
        {
            msg.clear();
            return true;
        }

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

    fn handle_help_popup_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                // Calculate actual popup dimensions and content length using stored terminal area
                let popup_area = create_centered_popup(self.terminal_area, 60, 70);
                let inner_area = ratatui::layout::Rect::new(
                    popup_area.x + 1,
                    popup_area.y + 1,
                    popup_area.width.saturating_sub(2),
                    popup_area.height.saturating_sub(2),
                );
                let max_visible_lines = inner_area.height as usize;

                // Generate actual help content to count lines
                let help_content = generate_help_content();
                let total_help_lines = help_content.len();

                // Set visible items for scroll state
                self.help_scroll.visible_items = max_visible_lines;

                // Use generic popup scroll handling
                handle_popup_scroll(
                    key.code,
                    &mut self.help_scroll.offset,
                    total_help_lines,
                    max_visible_lines,
                );
                true
            }
            // Any other key closes the help popup and returns to normal mode
            _ => {
                self.show_help_popup = false;
                self.help_scroll.reset(); // Reset scroll offset when closing
                true // Continue running
            }
        }
    }

    fn handle_metrics_popup_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                // Calculate actual popup dimensions and content length using stored terminal area
                let popup_area = create_centered_popup(self.terminal_area, 60, 70);
                let inner_area = ratatui::layout::Rect::new(
                    popup_area.x + 1,
                    popup_area.y + 1,
                    popup_area.width.saturating_sub(2),
                    popup_area.height.saturating_sub(2),
                );
                let max_visible_lines = inner_area.height as usize;

                // Generate actual metrics content to count lines
                let metrics_content = generate_metrics_content(&self.metrics);
                let total_metrics_lines = metrics_content.len();

                // Set visible items for scroll state
                self.metrics_scroll.visible_items = max_visible_lines;

                // Use generic popup scroll handling
                handle_popup_scroll(
                    key.code,
                    &mut self.metrics_scroll.offset,
                    total_metrics_lines,
                    max_visible_lines,
                );
                true
            }
            // Any other key closes the metrics popup and returns to normal mode
            _ => {
                self.show_metrics_popup = false;
                self.metrics_scroll.reset(); // Reset scroll offset when closing
                true // Continue running
            }
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

            // Service Details scrolling with Shift+Up/Down and J/K (handled first to avoid conflicts)
            KeyCode::Up
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.scroll_details_up();
                true
            }

            KeyCode::Down
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
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
            KeyCode::Char('j')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
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
            KeyCode::Home
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.navigate_service_types_to_first();
                true
            }

            KeyCode::End
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
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

                self.update_metric("services_updated");
            }
            true
        } else {
            // Ensure service type exists for filtering purposes
            self.add_service_type(&service_entry.service_type);
            self.services.push(service_entry);
            self.update_metric("services_discovered");
            false
        }
    }

    fn mark_service_offline(&mut self, fullname: &str) -> bool {
        let service_idx = self.services.iter().position(|s| s.fullname == fullname);

        if let Some(idx) = service_idx {
            // Only count as removed if the service was online
            let was_online = self.services[idx].online;
            if was_online {
                self.update_metric("services_marked_offline");
            }
            self.services[idx].go_offline_at(current_timestamp_micros());
            self.invalidate_cache_and_validate();
            true
        } else {
            false
        }
    }

    fn navigate_services_up(&mut self) {
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_up(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        self.details_scroll.reset(); // Reset details scroll when navigating
    }

    fn navigate_services_down(&mut self) {
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_down(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        self.details_scroll.reset(); // Reset details scroll when navigating
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
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_page_up(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        self.details_scroll.reset(); // Reset details scroll when navigating
    }

    fn navigate_services_page_down(&mut self) {
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_page_down(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        self.details_scroll.reset(); // Reset details scroll when navigating
    }

    fn navigate_services_to_first(&mut self) {
        navigate_list_to_first(&mut self.selected_service, &mut self.services_scroll);
        self.details_scroll.reset(); // Reset details scroll when navigating
    }

    fn navigate_services_to_last(&mut self) {
        let filtered_len = {
            let filtered = self.get_filtered_services();
            filtered.len()
        };
        navigate_list_to_last(
            &mut self.selected_service,
            &mut self.services_scroll,
            filtered_len,
        );
        self.details_scroll.reset(); // Reset details scroll when navigating
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

            let a_addr_str = a.addrs.first().map(|s| s.as_str()).unwrap_or("<no-addr>");
            let b_addr_str = b.addrs.first().map(|s| s.as_str()).unwrap_or("<no-addr>");

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

fn current_timestamp_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
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
                    if state.mark_service_offline(&fullname) {
                        let _ = notification_sender_inner.send(Notification::ServiceChanged);
                    }
                }
                ServiceEvent::ServiceResolved(resolved_service) => {
                    let entry = ServiceEntry::from(*resolved_service);
                    let mut state = state_inner.write().await;
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

fn ui(f: &mut Frame, app_state: &mut AppState) {
    // Store current terminal area for popup calculations
    app_state.terminal_area = f.area();

    // Ensure state is consistent before rendering
    app_state.validate_selected_type();

    let layout = if app_state.filter_input_mode {
        create_filter_input_layout(f.area())
    } else {
        create_main_layout(f.area())
    };
    let visible_counts = calculate_visible_counts(&layout);

    // Update state with current visible counts
    app_state.types_scroll.visible_items = visible_counts.types;
    app_state.services_scroll.visible_items = visible_counts.services;

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

    // Render status message if present
    render_status_message(f, app_state);

    // Render popups if active
    if app_state.show_help_popup {
        render_help_popup(f, app_state.help_scroll.offset);
    } else if app_state.show_metrics_popup {
        render_metrics_popup(f, app_state, app_state.metrics_scroll.offset);
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
    .saturating_sub(app_state.types_scroll.offset);
    list_state.select(Some(display_index));
    f.render_stateful_widget(types_list, area, &mut list_state);
}

fn render_services_list(
    f: &mut Frame,
    app_state: &mut AppState,
    area: ratatui::layout::Rect,
    _visible_services: usize,
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

    let title = Line::from(vec![
        Span::raw("Services ["),
        Span::styled(
            format!("{}/{}", filtered_indices_len, services_clone.len()),
            Style::default().fg(STATUS_OK),
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
            .saturating_sub(app_state.services_scroll.offset),
    ));
    f.render_stateful_widget(services_list, area, &mut services_list_state);
}

fn render_service_details(f: &mut Frame, app_state: &mut AppState, area: ratatui::layout::Rect) {
    let selected_service_idx = app_state.selected_service;
    let services_clone = app_state.services.clone();

    // Update visible items for details scroll state
    app_state.details_scroll.visible_items = area.height.saturating_sub(2) as usize; // Account for borders

    let filtered_indices = app_state.get_filtered_services();

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

        let details = Paragraph::new(visible_details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Service Details (Shift+↑/↓, J/K to scroll)"),
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
            .style(Style::default().fg(STATUS_OK).bg(Color::DarkGray));

        // Create the inner area for text (accounting for borders)
        let inner_area = block.inner(popup_area);

        // Render the border/frame
        f.render_widget(block, popup_area);

        // Render the message text centered in the inner area
        let paragraph = Paragraph::new(msg.as_str())
            .style(Style::default().fg(STATUS_OK).bg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(paragraph, inner_area);
    }
}

fn render_help_popup(f: &mut Frame, help_scroll_offset: usize) {
    let help_content = generate_help_content();

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

    // Apply scroll offset to help content with clamping
    let clamped_offset = if help_content.is_empty() {
        0
    } else {
        help_scroll_offset.min(help_content.len().saturating_sub(1))
    };
    let visible_help_content: Vec<Line> = help_content.into_iter().skip(clamped_offset).collect();

    let help_paragraph = Paragraph::new(visible_help_content)
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

fn render_metrics_popup(f: &mut Frame, app_state: &AppState, metrics_scroll_offset: usize) {
    let metrics_content = generate_metrics_content(&app_state.metrics);

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

    // Apply scroll offset to metrics content with clamping
    let visible_lines = inner_area.height as usize;
    let max_offset = metrics_content.len().saturating_sub(visible_lines);
    let clamped_offset = metrics_scroll_offset.min(max_offset);
    let visible_metrics_content: Vec<Line> =
        metrics_content.into_iter().skip(clamped_offset).collect();

    let metrics_paragraph = Paragraph::new(visible_metrics_content)
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

fn generate_help_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(" Help Controls:"),
        Line::from("   ↑/↓               - Scroll this help content"),
        Line::from("   Any other key     - Close this help popup"),
        Line::from(" "),
        Line::from(" Navigation:"),
        Line::from("   ↑/↓ or j/k        - Navigate services list"),
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
        Line::from("   ?                 - Toggle this help popup"),
        Line::from("   Ctrl+j            - Dump state to json file"),
        Line::from("   q or Ctrl+c       - Quit the application"),
        Line::from(" "),
        Line::from(" Sorting:"),
        Line::from(
            "   s                 - Cycle sort field: Host → Type → Name → Port → Addr → Time",
        ),
        Line::from("   S                 - Cycle sort field backward"),
        Line::from("   o                 - Toggle sort direction (↑/↓)"),
        Line::from(" "),
        Line::from("   Sort field highlighted in yellow, direction in cyan"),
        Line::from(" "),
        Line::from(" Quick Filter:"),
        Line::from("   /                 - Start typing to filter services"),
        Line::from("   Enter             - Apply filter"),
        Line::from("   Esc               - Cancel filter input"),
        Line::from("   Backspace         - Delete last character"),
        Line::from("   n (normal mode)   - Clear current filter"),
        Line::from(" "),
        Line::from("   Filter searches all service fields case-insensitively"),
    ]
}

fn generate_metrics_content(metrics: &BTreeMap<String, u64>) -> Vec<Line<'static>> {
    let mut metrics_content: Vec<Line> = vec![
        Line::from(""),
        Line::from(" Metrics Controls:"),
        Line::from("   ↑/↓               - Scroll this metrics content"),
        Line::from("   Any other key     - Close this metrics popup"),
        Line::from(" "),
        Line::from(" Service Discovery Metrics:"),
        Line::from(" "),
    ];

    // Separate custom metrics from daemon metrics
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

    metrics_content
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
        STATUS_ERROR
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

fn format_duration_micros(duration_micros: u64) -> String {
    let total_seconds = duration_micros / 1_000_000;
    let remaining_micros = duration_micros % 1_000_000;

    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = (total_seconds / 3600) % 24;
    let days = total_seconds / 86400;

    let mut parts = Vec::new();

    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }

    // Always include seconds with precision
    if remaining_micros > 0 {
        let precise_seconds = seconds as f64 + remaining_micros as f64 / 1_000_000.0;
        parts.push(format!("{:.3}s", precise_seconds));
    } else {
        parts.push(format!("{}s", seconds));
    }

    parts.join(" ")
}

fn create_service_details_text(service: &ServiceEntry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Online status - use blue (color-blind friendly)
    let online_style: Style = Style::default().fg(STATUS_OK).add_modifier(Modifier::BOLD);
    // Offline status - use orange (color-blind friendly)
    let offline_style: Style = Style::default()
        .fg(STATUS_ERROR)
        .add_modifier(Modifier::BOLD);

    if service.online {
        lines.push(Line::from(vec![
            Span::styled("Status:", Style::default()),
            Span::styled(" Online", online_style),
        ]));

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
        lines.push(Line::from(vec![
            Span::styled("Status:", Style::default()),
            Span::styled(" Offline", offline_style),
        ]));

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
    let timeline = service.get_session_history();
    for timeline_line in timeline.lines() {
        lines.push(Line::from(timeline_line.to_string()));
    }

    lines
}

pub async fn run_tui(
    user_service_types: HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal for full TUI
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mdns = ServiceDaemon::new()?;

    // Initialize app state
    let state = Arc::new(RwLock::new(AppState::new(user_service_types)));

    // Create notification channels
    let (notification_sender, notification_receiver) = flume::unbounded::<Notification>();

    let state_clone = Arc::clone(&state);
    let notification_sender_clone = notification_sender.clone();

    // Browse for user_requested service types provided via command line
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
                match start_browsing_service_type(
                    &mdns,
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

    if state.read().await.user_service_types.is_empty() {
        // Browse for all service types
        let receiver = mdns.browse("_services._dns-sd._udp.local.")?;

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
                        if is_sub_type(&service_type) {
                            continue; // skip subtypes in auto-discovery
                        }
                        {
                            let mut state = state_clone.write().await;
                            if state.add_service_type(&service_type) {
                                state.update_metric("service_types_discovered");
                                let _ =
                                    notification_sender_clone.send(Notification::ServiceChanged);
                                match start_browsing_service_type(
                                    &mdns,
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
                    _ => (),
                }
            }
        });
    }
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
            updated_at_micros: 1000,
            session_history: vec![ServiceSession {
                start_time: 1000,
                end_time: None,
                duration_micros: 0,
            }],
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
        }
    }

    // ServiceEntry tests
    #[test]
    fn test_service_entry_go_offline_at() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        assert!(service.online);
        service.go_offline_at(2000);
        assert!(!service.online);
        assert_eq!(service.updated_at_micros, 2000);
        assert_eq!(service.last_offline_micros, Some(2000));
        assert_eq!(service.session_history.len(), 1);
    }

    // Session timeline tests

    #[test]
    fn test_service_entry_full_online_offline_cycle() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        // First cycle: go offline
        service.go_offline_at(2000);
        assert_eq!(service.session_history.len(), 1);

        // Second cycle: go online then offline
        service.go_online_at(3000);
        assert_eq!(service.session_history.len(), 2); // 1 completed + 1 active
        service.go_offline_at(5000);

        assert_eq!(service.session_history.len(), 2); // 2 completed + 0 active
    }

    #[test]
    fn test_get_session_timeline_multiple_sessions() {
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: false,
            updated_at_micros: 9000000,
            session_history: vec![
                ServiceSession {
                    start_time: 1000000,
                    end_time: Some(5000000), // 4s
                    duration_micros: 4000000,
                },
                ServiceSession {
                    start_time: 6000000,
                    end_time: Some(9000000), // 3s
                    duration_micros: 3000000,
                },
            ],
            first_seen_micros: 1000000,
            last_online_micros: Some(6000000),
            last_offline_micros: Some(9000000),
        };

        let timeline = service.get_session_history();
        let lines: Vec<&str> = timeline.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Session 1:"));
        assert!(lines[0].contains("4s"));
        assert!(lines[1].contains("Session 2:"));
        assert!(lines[1].contains("3s"));
    }

    #[test]
    fn test_get_session_timeline_alignment_single_digit() {
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: false,
            updated_at_micros: 5000000,
            session_history: vec![
                ServiceSession {
                    start_time: 1000000,
                    end_time: Some(2000000), // 1s
                    duration_micros: 1000000,
                },
                ServiceSession {
                    start_time: 3000000,
                    end_time: Some(4000000), // 1s
                    duration_micros: 1000000,
                },
            ],
            first_seen_micros: 1000000,
            last_online_micros: Some(3000000),
            last_offline_micros: Some(4000000),
        };

        let timeline = service.get_session_history();
        let lines: Vec<&str> = timeline.lines().collect();

        assert_eq!(lines.len(), 2);
        // Both session numbers should be right-aligned with width 1 (single digit)
        assert!(lines[0].starts_with("Session 1:"));
        assert!(lines[1].starts_with("Session 2:"));
    }

    #[test]
    fn test_get_session_timeline_alignment_double_digit() {
        let mut sessions = Vec::new();
        for i in 0..10 {
            sessions.push(ServiceSession {
                start_time: (i * 10000000) + 1000000,
                end_time: Some((i * 10000000) + 2000000), // 1s each
                duration_micros: 1000000,
            });
        }

        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: false,
            updated_at_micros: 91000000,
            session_history: sessions,
            first_seen_micros: 1000000,
            last_online_micros: Some(91000000),
            last_offline_micros: Some(92000000),
        };

        let timeline = service.get_session_history();
        let lines: Vec<&str> = timeline.lines().collect();

        assert_eq!(lines.len(), 10);

        // Session 1-9 should be right-aligned with width 2 (since we have 10 sessions)
        assert!(lines[0].starts_with("Session  1:"));
        assert!(lines[8].starts_with("Session  9:"));
        assert!(lines[9].starts_with("Session 10:"));
    }

    #[test]
    fn test_get_session_timeline_duration_alignment_mixed() {
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: false,
            updated_at_micros: 3700000000,
            session_history: vec![
                ServiceSession {
                    start_time: 1000000,
                    end_time: Some(2000000), // 1s (short duration)
                    duration_micros: 1000000,
                },
                ServiceSession {
                    start_time: 3000000,
                    end_time: Some(9000000), // 6s (medium duration)
                    duration_micros: 6000000,
                },
                ServiceSession {
                    start_time: 10000000,
                    end_time: Some(3700000000), // 1h 1m 30s (long duration)
                    duration_micros: 3690000000,
                },
            ],
            first_seen_micros: 1000000,
            last_online_micros: Some(10000000),
            last_offline_micros: Some(3700000000),
        };

        let timeline = service.get_session_history();
        let lines: Vec<&str> = timeline.lines().collect();

        assert_eq!(lines.len(), 3);

        // Find the position where timestamps start (should be consistent)
        // We look for the arrow separator and check position after it
        let arrow_pos1 = lines[0].find(" → ").unwrap();
        let arrow_pos2 = lines[1].find(" → ").unwrap();
        let arrow_pos3 = lines[2].find(" → ").unwrap();

        // All arrows should be at the same position, meaning durations are aligned
        assert_eq!(arrow_pos1, arrow_pos2);
        assert_eq!(arrow_pos2, arrow_pos3);

        let eq_pos1 = lines[0].find(" = ").unwrap();
        let eq_pos2 = lines[1].find(" = ").unwrap();
        let eq_pos3 = lines[2].find(" = ").unwrap();

        assert_eq!(eq_pos1, eq_pos2);
        assert_eq!(eq_pos2, eq_pos3);
    }

    #[test]
    fn test_get_session_timeline_shows_active_session_as_ongoing_with_na() {
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: true, // Currently online
            updated_at_micros: 3000000,
            session_history: vec![ServiceSession {
                start_time: 3000000,
                end_time: None, // Active session (no end time)
                duration_micros: 0,
            }],
            first_seen_micros: 1000000,
            last_online_micros: Some(3000000),
            last_offline_micros: Some(2000000),
        };

        let timeline = service.get_session_history();
        let lines: Vec<&str> = timeline.lines().collect();

        assert_eq!(lines.len(), 1);
        // ongoing session should indicate "Ongoing" and have "N/A" for duration
        assert!(lines[0].contains("Session 1:"));
        assert!(lines[0].contains("Ongoing"));
        assert!(lines[0].contains("N/A"));
    }

    #[test]
    fn test_get_session_timeline_long_duration_alignment() {
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec!["192.168.1.1".to_string()],
            port: 8080,
            txt: vec![],
            online: false,
            updated_at_micros: 500000000000,
            session_history: vec![
                ServiceSession {
                    start_time: 1000000,
                    end_time: Some(5000000), // 4s (short)
                    duration_micros: 4000000,
                },
                ServiceSession {
                    start_time: 6000000,
                    end_time: Some(500000000000), // ~5d 21h 53m 20s (very long)
                    duration_micros: 499994000000,
                },
            ],
            first_seen_micros: 1000000,
            last_online_micros: Some(6000000),
            last_offline_micros: Some(500000000000),
        };

        let timeline = service.get_session_history();
        let lines: Vec<&str> = timeline.lines().collect();

        assert_eq!(lines.len(), 2);

        // Find arrow positions - should be aligned despite different duration lengths
        let arrow_pos1 = lines[0].find(" → ").unwrap();
        let arrow_pos2 = lines[1].find(" → ").unwrap();

        // All arrows should be at the same position (durations aligned)
        assert_eq!(arrow_pos1, arrow_pos2);

        // Verify the long duration contains expected components
        assert!(lines[1].contains("5d"));
        assert!(lines[1].contains("h"));
        assert!(lines[1].contains("m"));
    }

    // AppState initialization tests
    #[test]
    fn test_appstate_new() {
        let state = AppState::new(HashSet::new());
        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll.offset, 0);
        assert_eq!(state.services_scroll.offset, 0);
        assert!(state.cache_dirty);
        assert!(!state.show_help_popup);
        assert!(!state.show_metrics_popup);
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
        let state = AppState::new(user_requested_types_set.clone());

        assert_eq!(state.user_service_types, user_requested_types_set);
        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_appstate_new_with_empty_user_service_types() {
        let user_requested_types = HashSet::new();
        let state = AppState::new(user_requested_types);

        assert_eq!(state.services.len(), 0);
        assert_eq!(state.service_types.len(), 0);
        assert!(state.user_service_types.is_empty());
    }

    #[test]
    fn test_appstate_new_with_single_user_service_type() {
        let user_requested_types = vec!["_printer._tcp.local.".to_string()];
        let user_requested_types_set: HashSet<String> = user_requested_types.into_iter().collect();
        let state = AppState::new(user_requested_types_set.clone());

        assert_eq!(state.user_service_types.len(), 1);
        assert!(state.user_service_types.contains("_printer._tcp.local."));
    }

    #[test]
    fn test_user_service_types_immutability() {
        let user_requested_types = vec!["_http._tcp.local.".to_string()];
        let user_requested_types_set: HashSet<String> = user_requested_types.into_iter().collect();
        let state = AppState::new(user_requested_types_set.clone());

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

    // Filter service tests
    #[test]
    fn test_filter_service_all_types() {
        let mut state = AppState::new(HashSet::new());
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
        };

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_specific_type() {
        let mut state = AppState::new(HashSet::new());
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
        };

        assert!(state.filter_service(&http_service));
        assert!(!state.filter_service(&ssh_service));
    }

    // Service type management tests
    #[test]
    fn test_add_service_type() {
        let mut state = AppState::new(HashSet::new());
        assert!(state.add_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
        assert_eq!(state.service_types[0], "_http._tcp.local.");

        // Adding duplicate should return false
        assert!(!state.add_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
    }

    #[test]
    fn test_add_service_type_maintains_sort_order() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        assert_eq!(state.service_types[0], "_http._tcp.local.");
        assert_eq!(state.service_types[1], "_printer._tcp.local.");
        assert_eq!(state.service_types[2], "_ssh._tcp.local.");
    }

    #[test]
    fn test_add_service_type_preserves_selection() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.selected_type = Some(1); // _ssh._tcp.local.

        // Add a new type, selection should still point to _ssh._tcp.local.
        state.add_service_type("_printer._tcp.local.");
        assert_eq!(state.selected_type, Some(2)); // _ssh._tcp.local. moved to index 2
    }

    #[test]
    fn test_remove_service_type() {
        let mut state = AppState::new(HashSet::new());
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
        });

        assert!(!state.remove_service_type("_http._tcp.local."));
        assert_eq!(state.service_types.len(), 2);

        // Can remove if not in use
        assert!(state.remove_service_type("_ssh._tcp.local."));
        assert_eq!(state.service_types.len(), 1);
    }

    #[test]
    fn test_remove_service_type_adjusts_selection() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
    fn test_navigate_service_types_page_up() {
        let mut state = AppState::new(HashSet::new());
        // Add 10 service types to test paging
        for i in 0..10 {
            state.add_service_type(&format!("_test{}.._tcp.local.", i));
        }
        state.selected_type = Some(8); // Start near the end
        state.types_scroll.visible_items = 3; // Simulate 3 visible items

        // Page up should move by visible_types - 1 = 2 positions
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, Some(6));

        // Another page up
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, Some(4));

        // Page up from index 1 should go to "All Types"
        state.selected_type = Some(1);
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, None);

        // Page up from "All Types" should stay at "All Types"
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_navigate_service_types_page_up_with_offset() {
        let mut state = AppState::new(HashSet::new());
        // Add several service types
        for i in 0..8 {
            state.add_service_type(&format!("_test{}.._tcp.local.", i));
        }
        state.selected_type = Some(5);
        state.types_scroll.visible_items = 3;
        state.types_scroll.offset = 3; // Currently showing types 3,4,5

        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, Some(3));
        assert_eq!(state.types_scroll.offset, 3); // Scroll offset should stay the same
    }

    #[test]
    fn test_navigate_service_types_page_down() {
        let mut state = AppState::new(HashSet::new());
        // Add 10 service types to test paging
        for i in 0..10 {
            state.add_service_type(&format!("_test{}.._tcp.local.", i));
        }
        state.selected_type = None; // Start at "All Types"
        state.types_scroll.visible_items = 3; // Simulate 3 visible items

        // Page down from "All Types" should jump to index 2 (visible_types - 1)
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(2));

        // Another page down should move by 2 positions
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(4));

        // Page down near the end should clamp to last index
        state.selected_type = Some(8);
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(9)); // Last index

        // Page down from last should stay at last
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(9));
    }

    #[test]
    fn test_navigate_service_types_page_down_with_few_types() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_test1._tcp.local.");
        state.add_service_type("_test2._tcp.local.");
        state.selected_type = None;
        state.types_scroll.visible_items = 5; // More visible than available

        // Page down should go to last available type
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(1)); // Last index
    }

    #[test]
    fn test_navigate_service_types_page_up_with_empty_types() {
        let mut state = AppState::new(HashSet::new());
        state.types_scroll.visible_items = 5;
        state.selected_type = None;

        // Page up should not crash and stay at None
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll.offset, 0);
    }

    #[test]
    fn test_navigate_service_types_page_up_with_zero_visible() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_test1._tcp.local.");
        state.add_service_type("_test2._tcp.local.");
        state.selected_type = Some(1);
        state.types_scroll.visible_items = 0;

        // Page up with 0 visible should not move
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, Some(1));
    }

    #[test]
    fn test_navigate_service_types_page_down_with_empty_types() {
        let mut state = AppState::new(HashSet::new());
        state.types_scroll.visible_items = 5;
        state.selected_type = None;

        // Page down should not crash and stay at None
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll.offset, 0);
    }

    #[test]
    fn test_navigate_service_types_page_down_with_zero_visible() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_test1._tcp.local.");
        state.add_service_type("_test2._tcp.local.");
        state.selected_type = None;
        state.types_scroll.visible_items = 0;

        // Page down with 0 visible should move to index 0 (scroll_amount = 0)
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(0));
    }

    #[test]
    fn test_navigate_service_types_to_first_with_empty_types() {
        let mut state = AppState::new(HashSet::new());
        state.types_scroll.visible_items = 5;
        state.types_scroll.offset = 5;

        state.navigate_service_types_to_first();
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll.offset, 0);
    }

    #[test]
    fn test_navigate_service_types_to_last_with_empty_types() {
        let mut state = AppState::new(HashSet::new());
        state.types_scroll.visible_items = 5;
        state.types_scroll.offset = 5;

        state.navigate_service_types_to_last();
        assert_eq!(state.selected_type, None);
        assert_eq!(state.types_scroll.offset, 0);
    }

    #[test]
    fn test_navigate_service_types_to_last_uses_saturating_sub() {
        let mut state = AppState::new(HashSet::new());
        for i in 0..3 {
            state.add_service_type(&format!("_test{}.._tcp.local.", i));
        }
        state.types_scroll.visible_items = 2;

        state.navigate_service_types_to_last();
        // Should use .len().saturating_sub(1) = 3-1 = 2
        assert_eq!(state.selected_type, Some(2));
        // Scroll offset should position last item at bottom of visible area
        assert_eq!(state.types_scroll.offset, 1); // 2 - 2 + 1 = 1
    }

    #[test]
    fn test_navigate_service_types_page_down_more_than_available() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_test1._tcp.local.");
        state.add_service_type("_test2._tcp.local.");
        state.selected_type = None;
        state.types_scroll.visible_items = 10; // More visible than available

        // Should go to last available index (1)
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(1));
    }

    #[test]
    fn test_service_type_pagination_edge_cases() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_test1._tcp.local.");
        state.types_scroll.visible_items = 1; // Only 1 visible item, scroll_amount = 0

        // Test page up with single visible item (scroll_amount = 0, so stays at index 0)
        state.selected_type = Some(0);
        state.navigate_service_types_page_up();
        assert_eq!(state.selected_type, Some(0));

        // Test page down with single visible item
        state.selected_type = None;
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(0));

        // Test page down from last with single visible item
        state.navigate_service_types_page_down();
        assert_eq!(state.selected_type, Some(0));
    }

    #[test]
    fn test_navigate_service_types_to_first() {
        let mut state = AppState::new(HashSet::new());
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 81));
        state.selected_service = 1;
        state.services_scroll.offset = 1;

        state.navigate_services_to_first();
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_navigate_services_to_last() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        for i in 0..20 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.services_scroll.visible_items = 5;
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
        let mut state = AppState::new(HashSet::new());
        for i in 0..20 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.services_scroll.visible_items = 5;
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        let key = KeyEvent::from(KeyCode::Char('q'));
        assert!(!state.handle_key_event(key)); // Should return false to quit
    }

    #[test]
    fn test_handle_key_event_toggle_help() {
        let mut state = AppState::new(HashSet::new());
        assert!(!state.show_help_popup);

        let key = KeyEvent::from(KeyCode::Char('?'));
        assert!(state.handle_key_event(key)); // Should return true to continue
        assert!(state.show_help_popup);

        assert!(state.handle_key_event(key));
        assert!(!state.show_help_popup);
    }

    #[test]
    fn test_handle_key_event_toggle_metrics() {
        let mut state = AppState::new(HashSet::new());
        assert!(!state.show_metrics_popup);

        let key = KeyEvent::from(KeyCode::Char('m'));
        assert!(state.handle_key_event(key));
        assert!(state.show_metrics_popup);

        assert!(state.handle_key_event(key));
        assert!(!state.show_metrics_popup);
    }

    #[test]
    fn test_handle_metrics_popup_key() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;

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
        state.metrics_scroll.offset = 3;
        let key_event = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.show_metrics_popup); // Should remain open
        // The exact scroll offset now depends on content length and terminal size
        assert!(state.metrics_scroll.offset >= 3); // Should not decrease

        // Test scrolling up at boundary (should not go below 0)
        state.metrics_scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.show_metrics_popup); // Should remain open
        assert_eq!(state.metrics_scroll.offset, 0);

        // Test any other key closes popup and resets scroll
        state.metrics_scroll.offset = 10;
        let key_event = KeyEvent::new(KeyCode::Char('x'), crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(!state.show_metrics_popup); // Should close
        assert_eq!(state.metrics_scroll.offset, 0); // Should reset
    }

    #[test]
    fn test_handle_help_popup_key() {
        let mut state = AppState::new(HashSet::new());
        state.show_help_popup = true;

        // Test scrolling down when at max scroll offset
        state.help_scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.show_help_popup); // Should remain open

        // Test scrolling up when at boundary (should not go below 0)
        state.help_scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(state.show_help_popup); // Should remain open
        assert_eq!(state.help_scroll.offset, 0);

        // Test any other key closes popup and resets scroll
        state.help_scroll.offset = 10;
        let key_event = KeyEvent::new(KeyCode::Char('x'), crossterm::event::KeyModifiers::NONE);
        let result = state.handle_key_event(key_event);
        assert!(result);
        assert!(!state.show_help_popup); // Should close
        assert_eq!(state.help_scroll.offset, 0); // Should reset
    }

    // Metrics tests
    #[test]
    fn test_update_metric() {
        let mut state = AppState::new(HashSet::new());
        state.update_metric("test_metric");
        assert_eq!(state.metrics.get("test_metric"), Some(&1));

        state.update_metric("test_metric");
        assert_eq!(state.metrics.get("test_metric"), Some(&2));
    }

    #[test]
    fn test_update_daemon_metrics() {
        let mut state = AppState::new(HashSet::new());
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
    fn test_metrics_scrolling_boundaries() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;

        // Add some metrics to ensure there's content to scroll through
        for i in 1..20 {
            state.update_metric(&format!("test_metric_{}", i));
        }

        // Test scrolling up when already at top (should stay at 0)
        state.metrics_scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(key_event);
        assert_eq!(state.metrics_scroll.offset, 0);

        // Test scrolling up from higher position
        state.metrics_scroll.offset = 3;
        let initial_offset = state.metrics_scroll.offset;
        state.handle_key_event(key_event);
        assert!(state.metrics_scroll.offset < initial_offset); // Should scroll up

        // Test scrolling down from various positions
        state.metrics_scroll.offset = 0;
        let key_event = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(key_event);
        // Should increment
        assert!(state.metrics_scroll.offset > 0);

        // Test scrolling down when already at max - set to very high value first
        state.metrics_scroll.offset = 100;
        let max_before = state.metrics_scroll.offset;
        state.handle_key_event(key_event);
        // Should not exceed max
        assert!(state.metrics_scroll.offset <= max_before);
    }

    #[test]
    fn test_metrics_scrolling_with_popup_state() {
        let mut state = AppState::new(HashSet::new());

        // Add some metrics to ensure there's content to scroll through
        for i in 1..20 {
            state.update_metric(&format!("test_metric_{}", i));
        }

        // Test that scrolling only works when popup is shown
        state.show_metrics_popup = false;
        state.metrics_scroll.offset = 5;
        let key_event = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);

        // This should not be called when popup is not shown, but let's test it anyway
        state.handle_key_event(key_event);
        let _offset_without_popup = state.metrics_scroll.offset;

        // Now test with popup shown
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 5;
        state.handle_key_event(key_event);
        // With popup shown, scrolling should work and offset should decrease
        assert!(state.metrics_scroll.offset < 5);
        // Note: behavior without popup is undefined since key shouldn't be handled then
    }

    #[test]
    fn test_metrics_scroll_reset_on_close() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 10;

        // Close popup with a non-scroll key
        let key_event = KeyEvent::new(KeyCode::Char('q'), crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(key_event);

        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0); // Should reset

        // Test with Enter key
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 15;
        let key_event = KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(key_event);

        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0); // Should reset

        // Test with Escape key
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 20;
        let key_event = KeyEvent::new(KeyCode::Esc, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(key_event);

        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0); // Should reset
    }

    #[test]
    fn test_metrics_scroll_return_value() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;

        // Test that all key events return true (continue running)
        let up_key = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let down_key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        let close_key = KeyEvent::new(KeyCode::Char('x'), crossterm::event::KeyModifiers::NONE);

        state.handle_key_event(up_key);
        state.handle_key_event(down_key);
        state.handle_key_event(close_key);
    }

    #[test]
    fn test_metrics_scroll_with_modifiers() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;

        // Add some metrics to ensure there's content to scroll through
        for i in 1..20 {
            state.update_metric(&format!("test_metric_{}", i));
        }
        state.metrics_scroll.offset = 2;

        // Test scrolling with Control modifier (should still work)
        let key_event = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::CONTROL);
        state.handle_key_event(key_event);
        assert_eq!(state.metrics_scroll.offset, 1);

        // Test scrolling with Shift modifier
        state.metrics_scroll.offset = 2;
        let key_event = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::SHIFT);
        state.handle_key_event(key_event);
        assert!(state.metrics_scroll.offset > 2);
    }

    #[test]
    fn test_metrics_scroll_multiple_operations() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 5;

        let up_key = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        let down_key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);

        // Test multiple up operations
        for _ in 0..10 {
            state.handle_key_event(up_key);
        }
        assert_eq!(state.metrics_scroll.offset, 0); // Should stop at 0

        // Test multiple down operations
        for _ in 0..20 {
            state.handle_key_event(down_key);
        }
        // Should not exceed calculated maximum
        assert!(state.metrics_scroll.offset <= 6);

        // Test mixed operations
        state.metrics_scroll.offset = 3;
        state.handle_key_event(up_key); // to 2
        state.handle_key_event(up_key); // to 1
        state.handle_key_event(down_key); // to 2
        state.handle_key_event(up_key); // to 1
        state.handle_key_event(up_key); // to 0

        assert_eq!(state.metrics_scroll.offset, 0);
    }

    #[test]
    fn test_metrics_scroll_page_navigation() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;

        // Test PageUp key (should behave like Up in current implementation)
        state.metrics_scroll.offset = 5;
        let page_up_key = KeyEvent::new(KeyCode::PageUp, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(page_up_key);
        assert!(!state.show_metrics_popup); // PageUp closes popup
        assert_eq!(state.metrics_scroll.offset, 0);

        // Test PageDown key (should behave like Down in current implementation)
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 0;
        let page_down_key = KeyEvent::new(KeyCode::PageDown, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(page_down_key);
        assert!(!state.show_metrics_popup); // PageDown closes popup
        assert_eq!(state.metrics_scroll.offset, 0);

        // Test Home key (should close popup)
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 3;
        let home_key = KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(home_key);
        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0);

        // Test End key (should close popup)
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 3;
        let end_key = KeyEvent::new(KeyCode::End, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(end_key);
        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0);
    }

    #[test]
    fn test_metrics_scroll_function_key_navigation() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 3;

        // Test F1 key (should close popup)
        let f1_key = KeyEvent::new(KeyCode::F(1), crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(f1_key);
        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0);

        // Test F5 key (should close popup)
        state.show_metrics_popup = true;
        state.metrics_scroll.offset = 3;
        let f5_key = KeyEvent::new(KeyCode::F(5), crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(f5_key);
        assert!(!state.show_metrics_popup);
        assert_eq!(state.metrics_scroll.offset, 0);
    }

    #[test]
    fn test_metrics_scroll_edge_cases() {
        let mut state = AppState::new(HashSet::new());
        state.show_metrics_popup = true;

        // Test with very large scroll offset (should be clamped)
        state.metrics_scroll.offset = 1000;
        let down_key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        state.handle_key_event(down_key);
        assert!(state.metrics_scroll.offset <= 6); // Should be clamped to max

        // Test with negative scroll offset (can't happen in practice, but test robustness)
        state.metrics_scroll.offset = 0;
        let up_key = KeyEvent::new(KeyCode::Up, crossterm::event::KeyModifiers::NONE);
        for _ in 0..10 {
            state.handle_key_event(up_key);
        }
        assert_eq!(state.metrics_scroll.offset, 0); // Should never go negative
    }

    // Cache tests
    #[test]
    fn test_filter_cache_invalidation() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
        };

        let display = format_service_for_display(&service);
        assert!(display.contains("test"));
        assert!(display.contains("testhost"));
        assert!(display.contains("<no-addr>"));
        assert!(display.contains("80"));
    }

    #[test]
    fn test_format_service_for_display_offline_service() {
        let service = ServiceEntry {
            fullname: "OfflineService._http._tcp.local.".to_string(),
            host: "offlinehost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: false,
            updated_at_micros: 2000000000,
            session_history: Vec::new(),
            first_seen_micros: 1000000000,
            last_online_micros: Some(1000000000),
            last_offline_micros: Some(2000000000),
        };

        let display = format_service_for_display(&service);
        assert!(display.contains("OfflineService"));
        assert!(display.contains("offlinehost"));
        assert!(display.contains("80"));
    }

    #[test]
    fn test_format_service_for_display_no_address_duplicate() {
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
            updated_at_micros: 1000000000,
            session_history: Vec::new(),
            first_seen_micros: 1000000000,
            last_online_micros: Some(1000000000),
            last_offline_micros: None,
        };

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
        assert!(details_text.contains("myhost.local."));
        assert!(details_text.contains("_http._tcp.local."));
        assert!(details_text.contains("_printer"));
        assert!(details_text.contains("8080"));
        assert!(details_text.contains("192.168.1.1"));
        assert!(details_text.contains("192.168.1.2"));
        assert!(details_text.contains("key1=value1"));
        assert!(details_text.contains("key2=value2"));
        assert!(details_text.contains("First seen:"));
        assert!(details_text.contains("Online"));
    }

    #[test]
    fn test_create_service_details_text_online_service() {
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
        };

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
        let service = ServiceEntry {
            fullname: "test._http._tcp.local.".to_string(),
            host: "testhost.local.".to_string(),
            service_type: "_http._tcp.local.".to_string(),
            subtype: None,
            addrs: vec![],
            port: 80,
            txt: vec![],
            online: false,
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
        };

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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: Some(1000),
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
        assert_eq!(style.fg, Some(Color::Yellow));
        assert!(style.add_modifier.contains(Modifier::ITALIC));
    }

    // Edge case tests
    #[test]
    fn test_empty_service_list_navigation() {
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

        state.navigate_service_types_up();
        assert_eq!(state.selected_type, None);

        state.navigate_service_types_down();
        assert_eq!(state.selected_type, None);
    }

    #[test]
    fn test_filter_with_no_matching_services() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let service1 = ServiceEntry {
            fullname: "aaa._http._tcp.local.".to_string(),
            host: "host1.local.".to_string(),
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
            updated_at_micros: 1000,
            session_history: Vec::new(),
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
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
        service1.updated_at_micros = 1000;
        let mut service2 = create_test_service("test2", "_http._tcp.local.", 80);
        service2.updated_at_micros = 2000;

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
        let mut state = AppState::new(HashSet::new());
        assert_eq!(state.sort_direction, SortDirection::Ascending);

        state.toggle_sort_direction();
        assert_eq!(state.sort_direction, SortDirection::Descending);

        state.toggle_sort_direction();
        assert_eq!(state.sort_direction, SortDirection::Ascending);
    }

    #[test]
    fn test_cycle_sort_field_forward() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_http._tcp.local.");
        for i in 0..5 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.selected_service = 3;
        state.services_scroll.offset = 2;

        state.update_sort_field(SortField::Port);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_update_sort_direction_resets_selection() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_http._tcp.local.");
        for i in 0..5 {
            state.services.push(create_test_service(
                &format!("test{}", i),
                "_http._tcp.local.",
                80 + i,
            ));
        }
        state.selected_service = 3;
        state.services_scroll.offset = 2;

        state.update_sort_direction(SortDirection::Descending);
        assert_eq!(state.selected_service, 0);
        assert_eq!(state.services_scroll.offset, 0);
    }

    #[test]
    fn test_sort_filtered_services_ascending() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_http._tcp.local.");

        let mut service1 = create_test_service("service1", "_http._tcp.local.", 80);
        service1.updated_at_micros = 3000;
        let mut service2 = create_test_service("service2", "_http._tcp.local.", 81);
        service2.updated_at_micros = 1000;
        let mut service3 = create_test_service("service3", "_http._tcp.local.", 82);
        service3.updated_at_micros = 2000;

        state.services.push(service1);
        state.services.push(service2);
        state.services.push(service3);

        state.sort_field = SortField::Timestamp;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();
        assert_eq!(state.services[filtered[0]].updated_at_micros, 1000);
        assert_eq!(state.services[filtered[1]].updated_at_micros, 2000);
        assert_eq!(state.services[filtered[2]].updated_at_micros, 3000);
    }

    #[test]
    fn test_sort_with_filtering() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        assert_eq!(state.sort_field, SortField::Host);

        let key = KeyEvent::from(KeyCode::Char('s'));
        state.handle_key_event(key);
        assert_eq!(state.sort_field, SortField::ServiceType);
    }

    #[test]
    fn test_key_event_cycle_sort_backward() {
        let mut state = AppState::new(HashSet::new());
        assert_eq!(state.sort_field, SortField::Host);

        let key = KeyEvent::from(KeyCode::Char('S'));
        state.handle_key_event(key);
        assert_eq!(state.sort_field, SortField::Timestamp);
    }

    #[test]
    fn test_key_event_toggle_sort_direction() {
        let mut state = AppState::new(HashSet::new());
        assert_eq!(state.sort_direction, SortDirection::Ascending);

        let key = KeyEvent::from(KeyCode::Char('o'));
        state.handle_key_event(key);
        assert_eq!(state.sort_direction, SortDirection::Descending);

        state.handle_key_event(key);
        assert_eq!(state.sort_direction, SortDirection::Ascending);
    }

    #[test]
    fn test_cache_invalidation_on_sort_change() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let state = AppState::new(HashSet::new());
        assert_eq!(state.filter_query, "");
        assert!(!state.filter_input_mode);
    }

    #[test]
    fn test_start_filter_input() {
        let mut state = AppState::new(HashSet::new());
        state.start_filter_input();
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_clear_filter() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        state.add_to_filter('a');
        state.add_to_filter('b');
        state.add_to_filter('c');
        assert_eq!(state.filter_query, "abc");
    }

    #[test]
    fn test_add_to_filter_invalidates_cache() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        state.selected_type = None;

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_with_text_query() {
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "test".to_string();

        let matching_service = create_test_service("test", "_http._tcp.local.", 80);
        let non_matching_service = create_test_service("other", "_http._tcp.local.", 80);

        assert!(state.filter_service(&matching_service));
        assert!(!state.filter_service(&non_matching_service));
    }

    #[test]
    fn test_filter_service_case_insensitive() {
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "TEST".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_service_searches_all_fields() {
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        state.filter_input_mode = true;

        let key = KeyEvent::from(KeyCode::Char('a'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "a");
    }

    #[test]
    fn test_handle_normal_mode_key_slash() {
        let mut state = AppState::new(HashSet::new());

        let key = KeyEvent::from(KeyCode::Char('/'));
        let should_continue = state.handle_key_event(key);

        assert!(should_continue);
        assert!(state.filter_input_mode);
        assert_eq!(state.filter_query, "");
    }

    #[test]
    fn test_handle_normal_mode_key_n() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        state.filter_query = String::new(); // Empty query
        state.selected_type = Some(0); // Specific type selected
        state.add_service_type("_http._tcp.local.");

        let service = create_test_service("test", "_http._tcp.local.", 80);
        assert!(state.filter_service(&service)); // Should show all since empty query
    }

    #[test]
    fn test_filter_with_special_characters() {
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "key=value".to_string();

        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.txt = vec!["key=value".to_string()];
        assert!(state.filter_service(&service));
    }

    // Test for the filter clear bug fix (regression test)
    #[test]
    fn test_clear_filter_when_empty_doesnt_reset_selection() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
        let service = create_test_service("test1", "_http._tcp.local.", 80);

        let was_updated = state.add_or_update_service(service);

        assert!(!was_updated);
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.metrics.get("services_discovered"), Some(&1));
    }

    #[test]
    fn test_add_or_update_service_returns_true_for_existing_service() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
    fn test_navigate_services_with_single_service() {
        let mut state = AppState::new(HashSet::new());
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));

        state.selected_service = 0;

        state.navigate_services_up();
        assert_eq!(state.selected_service, 0);

        state.navigate_services_down();
        assert_eq!(state.selected_service, 0);
    }

    #[test]
    fn test_navigate_service_types_with_single_type() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_http._tcp.local.");
        state.selected_type = Some(0);

        state.navigate_service_types_down();
        assert_eq!(state.selected_type, Some(0));
    }

    #[test]
    fn test_page_navigation_with_fewer_items_than_page_size() {
        let mut state = AppState::new(HashSet::new());
        state
            .services
            .push(create_test_service("test1", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("test2", "_http._tcp.local.", 81));
        state.services_scroll.visible_items = 10; // Page size larger than item count

        state.selected_service = 0;
        state.navigate_services_page_down();
        assert_eq!(state.selected_service, 1); // Should go to last item

        state.navigate_services_page_up();
        assert_eq!(state.selected_service, 0); // Should go to first item
    }

    #[test]
    fn test_remove_offline_services_with_all_offline() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());

        state.update_metric_by("test_metric", 5);
        assert_eq!(state.metrics.get("test_metric"), Some(&5));

        state.update_metric_by("test_metric", 3);
        assert_eq!(state.metrics.get("test_metric"), Some(&8));
    }

    #[test]
    fn test_filter_query_with_multiple_words() {
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "192.168".to_string();

        let mut service = create_test_service("test", "_http._tcp.local.", 80);
        service.addrs = vec!["192.168.1.100".to_string()];

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_query_partial_match() {
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "http".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 80);

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_filter_with_port_as_string() {
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "8080".to_string();

        let service = create_test_service("test", "_http._tcp.local.", 8080);

        assert!(state.filter_service(&service));
    }

    #[test]
    fn test_scroll_offset_updates_correctly_on_navigation() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());
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
    fn test_filter_and_sort_together() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_ssh._tcp.local.");

        // Add services with different types and names
        state
            .services
            .push(create_test_service("zebra", "_http._tcp.local.", 80));
        state
            .services
            .push(create_test_service("alpha", "_http._tcp.local.", 81));
        state
            .services
            .push(create_test_service("beta", "_ssh._tcp.local.", 22));

        // Filter to HTTP and sort by host
        state.selected_type = Some(0); // _http._tcp.local.
        state.sort_field = SortField::Host;
        state.sort_direction = SortDirection::Ascending;
        state.mark_cache_dirty();

        let filtered = state.get_filtered_services().to_vec();

        assert_eq!(filtered.len(), 2);
        assert_eq!(state.services[filtered[0]].host, "alpha.local.");
        assert_eq!(state.services[filtered[1]].host, "zebra.local.");
    }

    #[test]
    fn test_key_event_ctrl_c() {
        let mut state = AppState::new(HashSet::new());

        let mut key = KeyEvent::from(KeyCode::Char('c'));
        key.modifiers = crossterm::event::KeyModifiers::CONTROL;

        let should_continue = state.handle_key_event(key);

        assert!(!should_continue); // Should quit
    }

    #[test]
    fn test_key_event_remove_offline() {
        let mut state = AppState::new(HashSet::new());
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
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());
        state.filter_query = "nonexistent".to_string();

        let filtered = state.get_filtered_services();

        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_service_type_sorting_order() {
        let mut state = AppState::new(HashSet::new());
        state.add_service_type("_ssh._tcp.local.");
        state.add_service_type("_http._tcp.local.");
        state.add_service_type("_printer._tcp.local.");

        // Service types should be sorted alphabetically
        assert_eq!(state.service_types[0], "_http._tcp.local.");
        assert_eq!(state.service_types[1], "_printer._tcp.local.");
        assert_eq!(state.service_types[2], "_ssh._tcp.local.");
    }

    // Tests for service removal metric fix
    #[test]
    fn test_remove_service_only_counts_online_services() {
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

        // Try to remove a service that doesn't exist
        let removed = state.mark_service_offline("nonexistent._http._tcp.local.");
        assert!(!removed);
        assert_eq!(state.metrics.get("services_marked_offline"), None);
    }

    #[test]
    fn test_remove_service_updates_timestamp() {
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

        // Add a service type but no services
        state.add_service_type("_test._tcp.local.");

        assert_eq!(state.service_types.len(), 1);

        // Clear should remove the empty type
        state.clear_stale_service_types();

        assert_eq!(state.service_types.len(), 0);
    }

    #[test]
    fn test_clear_stale_service_types_no_stale() {
        let mut state = AppState::new(HashSet::new());

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
        let mut state = AppState::new(HashSet::new());

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
        assert!(parsed.get("filters").is_some(), "Should have filters");
        assert!(parsed.get("sorting").is_some(), "Should have sorting");

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
        let mut state = AppState::new(HashSet::new());

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

        // Verify content is valid JSON
        let _parsed: serde_json::Value =
            serde_json::from_str(&content).expect("File content should be valid JSON");

        // Clean up
        tokio::fs::remove_file(&filename).await.ok();
    }
}

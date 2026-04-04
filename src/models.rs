// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use mdns_sd::{ResolvedService, ScopedIp};
use serde::{Deserialize, Serialize};

pub fn format_ip_for_display(ip: &ScopedIp) -> String {
    match ip {
        ScopedIp::V4(v4) => v4.addr().to_string(),
        ScopedIp::V6(v6) => v6.addr().to_string(),
        _ => ip.to_string(),
    }
}

pub fn format_scoped_ip_with_context(ip: &ScopedIp, all_addrs: &[ScopedIp]) -> String {
    match ip {
        ScopedIp::V4(v4) => {
            let addr = v4.addr();
            let interface_ids = v4.interface_ids();
            if interface_ids.is_empty() {
                addr.to_string()
            } else {
                let mut sorted_ids = interface_ids.to_vec();
                sorted_ids.sort_by_key(|id| id.index);
                let names: Vec<&str> = sorted_ids.iter().map(|id| id.name.as_str()).collect();
                format!("{} via {}", addr, names.join(", "))
            }
        }
        ScopedIp::V6(v6) => {
            let addr = v6.addr();
            let scope_id = v6.scope_id();
            let is_link_local = addr.is_unicast_link_local();

            // For link-local: only consider entries with SAME address AND SAME interface
            // For non-link-local: consider all entries with same address (regardless of interface)

            if is_link_local {
                let my_index = all_addrs.iter().position(|a| std::ptr::eq(a, ip));
                let seen_before = my_index.is_some_and(|my_pos| {
                    all_addrs[..my_pos].iter().any(|a| {
                        if let ScopedIp::V6(av6) = a {
                            av6.addr() == addr && av6.scope_id().name == scope_id.name
                        } else {
                            false
                        }
                    })
                });

                if seen_before {
                    return String::new();
                }

                #[cfg(windows)]
                {
                    format!("{}%{}", addr, scope_id.index)
                }
                #[cfg(not(windows))]
                {
                    format!("{}%{}", addr, scope_id.name)
                }
            } else {
                let mut all_interfaces: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();
                for a in all_addrs {
                    if let ScopedIp::V6(av6) = a
                        && av6.addr() == addr
                    {
                        all_interfaces.insert(av6.scope_id().name.clone());
                    }
                }
                let my_actual_index = all_addrs.iter().position(|a| std::ptr::eq(a, ip));
                let first_occurrence_for_addr = all_addrs.iter().position(|a| {
                    if let ScopedIp::V6(av6) = a {
                        av6.addr() == addr
                    } else {
                        false
                    }
                });

                if let (Some(my_idx), Some(first_idx)) =
                    (my_actual_index, first_occurrence_for_addr)
                    && my_idx != first_idx
                {
                    return String::new();
                }

                if all_interfaces.is_empty() {
                    addr.to_string()
                } else {
                    let interfaces: Vec<&str> = all_interfaces.iter().map(|s| s.as_str()).collect();
                    format!("{} via {}", addr, interfaces.join(", "))
                }
            }
        }
        _ => ip.to_string(),
    }
}

pub fn format_service_addrs(addrs: &[ScopedIp], separator: &str) -> String {
    addrs
        .iter()
        .filter_map(|addr| {
            let s = format_scoped_ip_with_context(addr, addrs);
            if s.is_empty() { None } else { Some(s) }
        })
        .collect::<Vec<_>>()
        .join(separator)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceSession {
    pub start_time: u64,
    pub end_time: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortField {
    #[default]
    Host,
    ServiceType,
    Fullname,
    Port,
    Address,
    Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableServiceEntry {
    pub fullname: String,
    pub host: String,
    pub service_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    pub addresses: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub txt_records: Vec<String>,
    pub is_online: bool,
    pub is_flapping: bool,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_online_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_offline_at: Option<String>,
    pub session_history: Vec<SerializableServiceSession>,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}
/// Converts a timestamp from microseconds since the Unix epoch to an ISO 8601 formatted UTC string.
///
/// # Arguments
/// * `micros` - Number of microseconds since the Unix epoch (1970-01-01T00:00:00Z)
///
/// # Returns
/// An ISO 8601 formatted UTC timestamp string in the format `YYYY-MM-DDTHH:MM:SS.ffffffZ`
///
/// # Behavior Notes
/// - Input is in microseconds since the Unix epoch
/// - Output is in UTC (no timezone offset applied)
/// - Sub-microsecond precision is truncated (nanos are derived from subsec_micros * 1000)
/// - For timestamps before the Unix epoch or other invalid values, returns the Unix epoch as fallback
///
/// # Example
/// ```
/// let micros = 1_000_000; // 1 second after epoch
/// let timestamp = crate::models::micros_to_iso_timestamp(micros);
/// assert_eq!(timestamp, "1970-01-01T00:00:01.000000Z");
/// ```
pub fn micros_to_iso_timestamp(micros: u64) -> String {
    let duration = Duration::from_micros(micros);
    let secs = duration.as_secs() as i64;
    let nanos = duration.subsec_micros() * 1000;

    match DateTime::<Utc>::from_timestamp(secs, nanos) {
        Some(datetime) => datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
        None => "1970-01-01T00:00:00.000000Z".to_string(), // Fallback for invalid timestamps
    }
}

fn iso_timestamp_to_micros(timestamp: &str) -> u64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
        let duration = dt.signed_duration_since(DateTime::<Utc>::from_timestamp(0, 0).unwrap());
        let micros = duration.num_microseconds().unwrap_or(0);
        return if micros < 0 { 0 } else { micros as u64 };
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S%.fZ") {
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let duration = dt.signed_duration_since(epoch);
        let micros = duration.num_microseconds().unwrap_or(0);
        return if micros < 0 { 0 } else { micros as u64 };
    }
    0
}

#[derive(Clone, Debug)]
pub struct ServiceEntry {
    pub fullname: String,
    pub host: String,
    pub service_type: String,
    pub subtype: Option<String>,
    pub addrs: Vec<ScopedIp>,
    pub port: u16,
    pub txt: Vec<String>,
    pub online: bool,
    pub updated_at_micros: u64,
    pub first_seen_micros: u64,
    pub last_online_micros: Option<u64>,
    pub last_offline_micros: Option<u64>,
    pub session_history: Vec<ServiceSession>,
    pub is_flapping: bool,
}

impl ServiceEntry {
    /// Returns all URLs associated with this service, deduplicated.
    pub fn get_urls(&self) -> Vec<String> {
        let mut urls = BTreeSet::new();

        for txt in &self.txt {
            if let Some((_key, value)) = txt.split_once('=')
                && let Ok(url) = url::Url::parse(value)
                && (url.scheme() == "http" || url.scheme() == "https")
            {
                urls.insert(url.into());
            }
        }

        if self.service_type.contains("http") {
            let path = self
                .txt
                .iter()
                .find_map(|txt| {
                    txt.split_once('=').and_then(|(key, value)| {
                        if key == "path" {
                            Some(value.trim())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or("");

            let scheme = if self.service_type.contains("https") {
                "https"
            } else {
                "http"
            };

            let mut insert_url = |host: &str| {
                if let Ok(base) = url::Url::parse(&format!("{}://{}:{}", scheme, host, self.port))
                    && let Ok(url) = base.join(path)
                {
                    urls.insert(url.into());
                }
            };

            insert_url(self.host.trim_end_matches('.'));

            for addr in &self.addrs {
                if let IpAddr::V6(ip) = addr.to_ip_addr()
                    && ip.is_unicast_link_local()
                {
                    continue;
                }
                let host_part = match addr.to_ip_addr() {
                    IpAddr::V4(ip) => ip.to_string(),
                    IpAddr::V6(ip) => format!("[{}]", ip),
                };
                insert_url(&host_part);
            }
        }

        urls.into_iter().collect()
    }

    /// Marks the service as offline at the given timestamp.
    ///
    /// Updates `online` to `false`, sets `updated_at_micros` and `last_offline_micros`.
    /// If there was a previous online session, closes it by setting its `end_time`.
    /// Calls `update_flapping_status` as a side effect.
    pub fn go_offline_at(&mut self, timestamp_micros: u64) {
        if !self.online {
            return;
        }
        self.online = false;
        self.updated_at_micros = timestamp_micros;
        self.last_offline_micros = Some(timestamp_micros);

        if let Some(last_online) = self.last_online_micros {
            if let Some(session) = self.session_history.iter_mut().last() {
                session.end_time = Some(timestamp_micros);
            } else {
                self.session_history.push(ServiceSession {
                    start_time: last_online,
                    end_time: Some(timestamp_micros),
                });
            }
        }

        self.update_flapping_status();
    }

    /// Marks the service as online at the given timestamp.
    ///
    /// Updates `online` to `true`, sets `updated_at_micros` and `last_online_micros`.
    /// Creates a new session in `session_history` with the current timestamp as start time.
    /// Calls `update_flapping_status` as a side effect.
    pub fn go_online_at(&mut self, timestamp_micros: u64) {
        if self.online {
            return;
        }
        self.updated_at_micros = timestamp_micros;
        self.online = true;
        self.last_online_micros = Some(timestamp_micros);

        self.session_history.push(ServiceSession {
            start_time: timestamp_micros,
            end_time: None,
        });

        self.update_flapping_status();
    }

    /// Determines if the service is flapping based on session history.
    ///
    /// Returns `true` if the service has at least 3 sessions with at least 3 completed,
    /// and at least half of the completed sessions are shorter than 5 minutes.
    pub fn is_flapping_service(&self) -> bool {
        const FLAPPING_SESSION_THRESHOLD: usize = 3;
        const MIN_COMPLETED_SESSIONS: usize = 3;
        const SHORT_SESSION_DURATION_MICROS: u64 = 300_000_000;

        if self.session_history.len() < FLAPPING_SESSION_THRESHOLD {
            return false;
        }

        let mut short_sessions = 0;
        for session in &self.session_history {
            if let Some(end_time) = session.end_time {
                let duration = end_time.saturating_sub(session.start_time);
                if duration < SHORT_SESSION_DURATION_MICROS {
                    short_sessions += 1;
                }
            }
        }

        let completed_sessions = self
            .session_history
            .iter()
            .filter(|s| s.end_time.is_some())
            .count();

        if completed_sessions < MIN_COMPLETED_SESSIONS {
            return false;
        }

        short_sessions * 2 >= completed_sessions
    }

    /// Updates the flapping status based on the current session history.
    ///
    /// Side effect: sets `is_flapping` field based on `is_flapping_service()`.
    pub fn update_flapping_status(&mut self) {
        self.is_flapping = self.is_flapping_service();
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
                let mut addrs: Vec<ScopedIp> =
                    resolved_service.get_addresses().iter().cloned().collect();
                addrs.sort_by(|a, b| {
                    a.to_ip_addr()
                        .cmp(&b.to_ip_addr())
                        .then_with(|| a.to_string().cmp(&b.to_string()))
                });
                addrs
            },
            port: resolved_service.get_port(),
            txt: {
                let mut txt: Vec<String> = resolved_service
                    .get_properties()
                    .iter()
                    .map(|prop| match prop.val() {
                        Some(val) => format!("{}={}", prop.key(), String::from_utf8_lossy(val)),
                        None => prop.key().to_string(),
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
            }],
            is_flapping: false,
        }
    }
}

impl From<&ServiceEntry> for SerializableServiceEntry {
    fn from(entry: &ServiceEntry) -> Self {
        let updated_at = if entry.updated_at_micros != entry.first_seen_micros {
            Some(micros_to_iso_timestamp(entry.updated_at_micros))
        } else {
            None
        };
        let last_online_at = if entry.last_online_micros != Some(entry.first_seen_micros) {
            entry.last_online_micros.map(micros_to_iso_timestamp)
        } else {
            None
        };
        Self {
            fullname: entry.fullname.clone(),
            host: entry.host.clone(),
            service_type: entry.service_type.clone(),
            subtype: entry.subtype.clone(),
            addresses: entry.addrs.iter().map(|ip| ip.to_string()).collect(),
            port: entry.port,
            txt_records: entry.txt.clone(),
            is_online: entry.online,
            is_flapping: entry.is_flapping,
            created_at: micros_to_iso_timestamp(entry.first_seen_micros),
            updated_at,
            last_online_at,
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

fn parse_scoped_ip(s: &str) -> Option<ScopedIp> {
    if let Some((addr_part, _scope)) = s.split_once('%') {
        addr_part.parse::<IpAddr>().ok().map(ScopedIp::from)
    } else {
        s.parse::<IpAddr>().ok().map(ScopedIp::from)
    }
}

impl From<&SerializableServiceEntry> for ServiceEntry {
    fn from(entry: &SerializableServiceEntry) -> Self {
        let first_seen_micros = iso_timestamp_to_micros(&entry.created_at);
        let updated_at_micros = entry
            .updated_at
            .as_ref()
            .map(|ts| iso_timestamp_to_micros(ts))
            .unwrap_or(first_seen_micros);
        let last_online_micros = entry
            .last_online_at
            .as_ref()
            .map(|ts| iso_timestamp_to_micros(ts));
        let last_offline_micros = entry
            .last_offline_at
            .as_ref()
            .map(|ts| iso_timestamp_to_micros(ts));

        Self {
            fullname: entry.fullname.clone(),
            host: entry.host.clone(),
            service_type: entry.service_type.clone(),
            subtype: entry.subtype.clone(),
            addrs: entry
                .addresses
                .iter()
                .filter_map(|s| parse_scoped_ip(s))
                .collect(),
            port: entry.port,
            txt: entry.txt_records.clone(),
            online: entry.is_online,
            updated_at_micros,
            first_seen_micros,
            last_online_micros,
            last_offline_micros,
            session_history: entry.session_history.iter().map(|s| s.into()).collect(),
            is_flapping: entry.is_flapping,
        }
    }
}

impl From<&SerializableServiceSession> for ServiceSession {
    fn from(session: &SerializableServiceSession) -> Self {
        Self {
            start_time: session
                .start_time
                .as_ref()
                .map(|ts| iso_timestamp_to_micros(ts))
                .unwrap_or(0),
            end_time: session
                .end_time
                .as_ref()
                .map(|ts| iso_timestamp_to_micros(ts)),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableServiceSession {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub dump_timestamp: String,
    pub application_name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDump {
    pub metadata: Metadata,
    pub services: Vec<SerializableServiceEntry>,
    pub service_types: Vec<String>,
    pub metrics: BTreeMap<String, u64>,
    #[serde(default)]
    pub options: AppOptions,
    #[serde(default)]
    pub filters: FilterInfo,
    pub sorting: SortInfo,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FilterInfo {
    pub query: String,
    #[serde(default)]
    pub active_service_types: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppOptions {
    pub service_types: Vec<String>,
    #[serde(default)]
    pub disable_ipv4: bool,
    #[serde(default)]
    pub disable_ipv6: bool,
    pub interfaces: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SortInfo {
    pub field: SortField,
    pub direction: SortDirection,
}

/// Returns the current timestamp in microseconds since the Unix epoch.
pub fn current_timestamp_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[cfg(test)]
pub mod tests {
    //! Unit tests for the models module.

    use super::*;

    pub fn scoped_ip(s: &str) -> ScopedIp {
        ScopedIp::from(s.parse::<IpAddr>().unwrap())
    }

    pub fn create_test_service(name: &str, service_type: &str, port: u16) -> ServiceEntry {
        let last_octet = (port % 254) + 1;
        let addr: IpAddr = format!("192.168.1.{}", last_octet).parse().unwrap();
        ServiceEntry {
            fullname: format!("{}.{}", name, service_type),
            host: format!("{}.local.", name),
            service_type: service_type.to_string(),
            subtype: None,
            addrs: vec![ScopedIp::from(addr)],
            port,
            txt: vec![],
            online: true,
            updated_at_micros: 1000,
            session_history: vec![ServiceSession {
                start_time: 1000,
                end_time: None,
            }],
            first_seen_micros: 1000,
            last_online_micros: Some(1000),
            last_offline_micros: None,
            is_flapping: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_test_service_with_sessions(
        name: &str,
        service_type: &str,
        port: u16,
        sessions: Vec<ServiceSession>,
        online: bool,
        updated_at_micros: u64,
        first_seen_micros: u64,
        last_online_micros: Option<u64>,
        last_offline_micros: Option<u64>,
    ) -> ServiceEntry {
        let mut service = create_test_service(name, service_type, port);
        service.online = online;
        service.updated_at_micros = updated_at_micros;
        service.session_history = sessions;
        service.first_seen_micros = first_seen_micros;
        service.last_online_micros = last_online_micros;
        service.last_offline_micros = last_offline_micros;
        service
    }

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

    #[test]
    fn test_service_entry_full_online_offline_cycle() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        service.go_offline_at(2000);
        assert_eq!(service.session_history.len(), 1);

        service.go_online_at(3000);
        assert_eq!(service.session_history.len(), 2);
        service.go_offline_at(5000);

        assert_eq!(service.session_history.len(), 2);
    }

    #[test]
    fn test_flapping_service_not_enough_sessions() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        service.session_history = vec![
            ServiceSession {
                start_time: 1000,
                end_time: Some(2000),
            },
            ServiceSession {
                start_time: 3000,
                end_time: Some(4000),
            },
        ];

        service.update_flapping_status();
        assert!(!service.is_flapping);
    }

    #[test]
    fn test_flapping_service_with_short_sessions() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        service.session_history = vec![
            ServiceSession {
                start_time: 1000,
                end_time: Some(2000),
            },
            ServiceSession {
                start_time: 3000,
                end_time: Some(4000),
            },
            ServiceSession {
                start_time: 5000,
                end_time: Some(6000),
            },
        ];

        service.update_flapping_status();
        assert!(service.is_flapping);
    }

    #[test]
    fn test_flapping_service_with_mixed_sessions() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        // 3 sessions, 2 short and 1 long - should be flapping (2/3 >= 1/2)
        service.session_history = vec![
            ServiceSession {
                start_time: 1000,
                end_time: Some(2000),
            }, // 1 second (short)
            ServiceSession {
                start_time: 3000,
                end_time: Some(15000000),
            }, // 12 seconds (not short)
            ServiceSession {
                start_time: 16000000,
                end_time: Some(17000000),
            }, // 1 second (short)
        ];

        service.update_flapping_status();
        assert!(service.is_flapping);
    }

    /// Converts hours, minutes, and seconds to microseconds since the Unix epoch.
    ///
    /// # Arguments
    /// * `hours` - Number of hours
    /// * `minutes` - Number of minutes
    /// * `seconds` - Number of seconds
    ///
    /// # Returns
    /// The total number of microseconds (hours * 3600 + minutes * 60 + seconds) * 1_000_000
    ///
    /// # Example
    /// ```
    /// let micros = micros_from(0, 0, 1); // 1 second = 1,000,000 microseconds
    /// assert_eq!(micros, 1_000_000);
    /// ```
    pub fn micros_from(hours: u32, minutes: u32, seconds: u32) -> u64 {
        (hours as u64 * 3600 + minutes as u64 * 60 + seconds as u64) * 1_000_000
    }

    #[test]
    fn test_flapping_service_with_long_sessions() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        service.session_history = vec![
            ServiceSession {
                start_time: 1_000,
                end_time: Some(1_000 + micros_from(0, 5, 1)),
            },
            ServiceSession {
                start_time: 12_000_000,
                end_time: Some(1_200_000 + micros_from(0, 5, 1)),
            },
            ServiceSession {
                start_time: 24_000_000,
                end_time: Some(24_000_000 + micros_from(0, 5, 1)), // 11 seconds
            },
        ];

        service.update_flapping_status();
        assert!(!service.is_flapping);
    }

    #[test]
    fn test_flapping_service_with_ongoing_sessions() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        service.session_history = vec![
            ServiceSession {
                start_time: 1000,
                end_time: Some(2000),
            },
            ServiceSession {
                start_time: 3000,
                end_time: Some(4000),
            },
            ServiceSession {
                start_time: 5000,
                end_time: None, // Ongoing - shouldn't count as short
            },
        ];

        service.update_flapping_status();
        assert!(!service.is_flapping);
    }

    #[test]
    fn test_flapping_service_no_completed_sessions() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);

        service.session_history = vec![
            ServiceSession {
                start_time: 1000,
                end_time: None,
            },
            ServiceSession {
                start_time: 3000,
                end_time: None,
            },
            ServiceSession {
                start_time: 5000,
                end_time: None,
            },
        ];

        service.update_flapping_status();
        assert!(!service.is_flapping);
    }

    #[test]
    fn test_get_urls_empty() {
        let service = create_test_service("test", "_service._tcp.local.", 8080);

        let urls = service.get_urls();
        assert!(urls.is_empty());
    }

    #[test]
    fn test_get_urls_single_url_from_txt() {
        let mut service = create_test_service("test", "_service._tcp.local.", 8080);
        service.txt = vec!["internal_url=http://example.com".to_string()];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "http://example.com/");
    }

    #[test]
    fn test_get_urls_multiple_urls_from_txt() {
        let mut service = create_test_service("test", "_service._tcp.local.", 8080);
        service.txt = vec![
            "internal_url=http://example.com".to_string(),
            "base_url=http://backup.example.com".to_string(),
        ];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"http://example.com/".to_string()));
        assert!(urls.contains(&"http://backup.example.com/".to_string()));
    }

    #[test]
    fn test_get_urls_txt_key_agnostic() {
        let mut service = create_test_service("test", "_service._tcp.local.", 8080);
        service.txt = vec!["foo=http://example.org".to_string()];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "http://example.org/");
    }

    #[test]
    fn test_get_urls_deduplicated() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![];
        service.txt = vec!["internal_url=http://myhost.local:8080/".to_string()];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
    }

    #[test]
    fn test_get_urls_http_service_default_root() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.txt = vec![];
        service.host = "myhost.local".to_string();
        service.addrs = vec![];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "http://myhost.local:8080/");
    }

    #[test]
    fn test_get_urls_http_service_with_path() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.txt = vec!["path=/api".to_string()];
        service.host = "myhost.local".to_string();
        service.addrs = vec![];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "http://myhost.local:8080/api");
    }

    #[test]
    fn test_get_urls_mixed_txt_and_constructed() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![];
        service.txt = vec!["internal_url=http://explicit.example.com".to_string()];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"http://explicit.example.com/".to_string()));
        assert!(urls.contains(&"http://myhost.local:8080/".to_string()));
    }

    #[test]
    fn test_get_urls_https_urls() {
        let mut service = create_test_service("test", "_service._tcp.local.", 443);
        service.txt = vec!["internal_url=https://secure.example.com".to_string()];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://secure.example.com/");
    }

    #[test]
    fn test_get_urls_non_http_rejected() {
        let mut service = create_test_service("test", "_service._tcp.local.", 8080);
        service.txt = vec!["internal_url=mailto:test@example.com".to_string()];

        let urls = service.get_urls();
        assert!(urls.is_empty());
    }

    #[test]
    fn test_get_urls_service_type_with_http_substring() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.txt = vec![];
        service.host = "myhost.local".to_string();
        service.addrs = vec![];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "http://myhost.local:8080/");
    }

    #[test]
    fn test_get_urls_service_type_with_https() {
        let mut service = create_test_service("test", "_https._tcp.local.", 443);
        service.txt = vec![];
        service.host = "myhost.local".to_string();
        service.addrs = vec![];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://myhost.local/");
    }

    #[test]
    fn test_get_urls_ipv4_address() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![scoped_ip("192.168.1.100")];

        let urls = service.get_urls();
        assert!(urls.contains(&"http://192.168.1.100:8080/".to_string()));
    }

    #[test]
    fn test_get_urls_ipv6_address() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![scoped_ip("2001:db8::1")];

        let urls = service.get_urls();
        assert!(urls.contains(&"http://[2001:db8::1]:8080/".to_string()));
    }

    #[test]
    fn test_get_urls_ipv6_link_local_skipped() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![scoped_ip("fe80::1")];

        let urls = service.get_urls();
        let ipv6_urls: Vec<_> = urls.iter().filter(|u| u.contains("fe80")).collect();
        assert!(ipv6_urls.is_empty());
    }

    #[test]
    fn test_get_urls_ipv4_and_ipv6_multiple_addrs() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![
            scoped_ip("192.168.1.100"),
            scoped_ip("fe80::1"),
            scoped_ip("2001:db8::1"),
        ];

        let urls = service.get_urls();
        assert!(urls.contains(&"http://192.168.1.100:8080/".to_string()));
        assert!(urls.contains(&"http://[2001:db8::1]:8080/".to_string()));
        let ipv6_link_local_urls: Vec<_> = urls.iter().filter(|u| u.contains("fe80")).collect();
        assert!(ipv6_link_local_urls.is_empty());
    }

    #[test]
    fn test_get_urls_hostname_and_ip_coexist() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![scoped_ip("192.168.1.100")];

        let urls = service.get_urls();
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"http://myhost.local:8080/".to_string()));
        assert!(urls.contains(&"http://192.168.1.100:8080/".to_string()));
    }

    #[test]
    fn test_get_urls_ip_with_path() {
        let mut service = create_test_service("test", "_http._tcp.local.", 8080);
        service.host = "myhost.local".to_string();
        service.addrs = vec![scoped_ip("192.168.1.100")];
        service.txt = vec!["path=/api/v1".to_string()];

        let urls = service.get_urls();
        assert!(urls.contains(&"http://192.168.1.100:8080/api/v1".to_string()));
    }

    #[test]
    fn test_get_urls_non_http_service_ignores_addrs() {
        let mut service = create_test_service("test", "_ssh._tcp.local.", 22);
        service.addrs = vec![scoped_ip("192.168.1.100")];

        let urls = service.get_urls();
        assert!(urls.is_empty());
    }
}

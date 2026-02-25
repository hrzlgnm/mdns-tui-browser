// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    Host,
    ServiceType,
    Fullname,
    Port,
    Address,
    Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug)]
pub struct ServiceEntry {
    pub fullname: String,
    pub host: String,
    pub service_type: String,
    pub subtype: Option<String>,
    pub addrs: Vec<String>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceSession {
    pub start_time: u64,
    pub end_time: Option<u64>,
}

impl ServiceEntry {
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

    pub fn get_session_history(&self) -> String {
        let mut completed_sessions = Vec::new();
        let mut max_session_num_length = 0;

        for (i, session) in self.session_history.iter().enumerate() {
            let session_num = i + 1;
            max_session_num_length = max_session_num_length.max(session_num.to_string().len());
            completed_sessions.push((session_num, session));
        }

        let mut timeline = Vec::new();
        for (session_num, session) in completed_sessions {
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

    pub fn is_flapping_service(&self) -> bool {
        const FLAPPING_SESSION_THRESHOLD: usize = 3;
        const MIN_COMPLETED_SESSIONS: usize = 3;
        const SHORT_SESSION_DURATION_MICROS: u64 = 10_000_000;

        if self.session_history.len() < FLAPPING_SESSION_THRESHOLD {
            return false;
        }

        let completed_sessions: Vec<&ServiceSession> = self
            .session_history
            .iter()
            .filter(|s| s.end_time.is_some())
            .collect();

        if completed_sessions.len() < MIN_COMPLETED_SESSIONS {
            return false;
        }

        let short_sessions = completed_sessions
            .iter()
            .filter(|s| {
                if let Some(end_time) = s.end_time {
                    let duration = end_time.saturating_sub(s.start_time);
                    duration < SHORT_SESSION_DURATION_MICROS
                } else {
                    false
                }
            })
            .count();

        short_sessions * 2 >= completed_sessions.len()
    }

    pub fn update_flapping_status(&mut self) {
        self.is_flapping = self.is_flapping_service();
    }
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

    if remaining_micros > 0 {
        let precise_seconds = seconds as f64 + remaining_micros as f64 / 1_000_000.0;
        let rounded_seconds = (precise_seconds * 1000.0).round() / 1000.0;

        if rounded_seconds >= 60.0 {
            minutes += 1;

            if minutes >= 60 {
                minutes = 0;
                hours += 1;

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

        if seconds > 0 || (days == 0 && hours == 0 && minutes == 0) {
            parts.push(format!("{}s", seconds));
        }
    }

    parts.join(" ")
}

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

pub fn micros_to_iso_timestamp(micros: u64) -> String {
    let duration = std::time::Duration::from_micros(micros);
    let secs = duration.as_secs() as i64;
    let nanos = duration.subsec_micros() * 1000;

    match DateTime::<Utc>::from_timestamp(secs, nanos) {
        Some(datetime) => datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
        None => "1970-01-01T00:00:00.000000Z".to_string(),
    }
}

pub fn iso_timestamp_to_micros(timestamp: &str) -> u64 {
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

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEntryDto {
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
    #[serde(default)]
    pub session_history: Vec<ServiceSessionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSessionDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DumpMetadata {
    pub dump_timestamp: String,
    pub application_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterSettings {
    pub query: String,
    pub active_service_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSettings {
    pub field: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub metadata: DumpMetadata,
    pub services: Vec<ServiceEntryDto>,
    pub service_types: Vec<String>,
    pub metrics: BTreeMap<String, u64>,
    pub filters: FilterSettings,
    pub sorting: SortSettings,
}

impl From<&ServiceEntry> for ServiceEntryDto {
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
            addresses: entry.addrs.clone(),
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

impl From<&ServiceSession> for ServiceSessionDto {
    fn from(session: &ServiceSession) -> Self {
        Self {
            start_time: Some(micros_to_iso_timestamp(session.start_time)),
            end_time: session.end_time.map(micros_to_iso_timestamp),
        }
    }
}

impl From<&ServiceEntryDto> for ServiceEntry {
    fn from(entry: &ServiceEntryDto) -> Self {
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
            addrs: entry.addresses.clone(),
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

impl From<&ServiceSessionDto> for ServiceSession {
    fn from(session: &ServiceSessionDto) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service(name: &str, service_type: &str, port: u16) -> ServiceEntry {
        let last_octet = (port % 254) + 1;
        ServiceEntry {
            fullname: format!("{}.{}", name, service_type),
            host: format!("{}.local.", name),
            service_type: service_type.to_string(),
            subtype: None,
            addrs: vec![format!("192.168.1.{}", last_octet)],
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

    fn create_test_service_with_sessions(
        name: &str,
        service_type: &str,
        port: u16,
        sessions: Vec<ServiceSession>,
    ) -> ServiceEntry {
        let mut service = create_test_service(name, service_type, port);
        service.session_history = sessions;
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
    fn test_get_session_timeline_multiple_sessions() {
        let service = create_test_service_with_sessions(
            "test",
            "_http._tcp.local.",
            8080,
            vec![
                ServiceSession {
                    start_time: 1000000,
                    end_time: Some(5000000),
                },
                ServiceSession {
                    start_time: 6000000,
                    end_time: Some(9000000),
                },
            ],
        );

        let timeline = service.get_session_history();
        assert!(!timeline.is_empty());
        assert!(timeline.contains("Session"));
        assert!(timeline.contains("1:"));
        assert!(timeline.contains("2:"));
    }

    #[test]
    fn test_get_session_timeline_shows_active_session_as_ongoing() {
        let service = create_test_service_with_sessions(
            "test",
            "_http._tcp.local.",
            8080,
            vec![ServiceSession {
                start_time: 1000000,
                end_time: None,
            }],
        );

        let timeline = service.get_session_history();
        assert!(timeline.contains("Ongoing"));
    }
}

// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};

use chrono::Utc;

pub use crate::models::{AppStateSnapshot, DumpMetadata, FilterSettings, SortSettings};

use crate::models::{ServiceEntry, ServiceSession, SortDirection, SortField};

pub fn create_state_dump(
    services: &[ServiceEntry],
    service_types: &[String],
    metrics: &BTreeMap<String, u64>,
    filter_query: &str,
    user_service_types: &HashSet<String>,
    sort_field: SortField,
    sort_direction: SortDirection,
) -> AppStateSnapshot {
    let meta = DumpMetadata {
        dump_timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        application_name: env!("CARGO_PKG_NAME").to_string(),
    };
    AppStateSnapshot {
        metadata: meta,
        services: services.iter().map(|s| s.into()).collect(),
        service_types: service_types.to_vec(),
        metrics: metrics.clone(),
        filters: FilterSettings {
            query: filter_query.to_string(),
            active_service_types: user_service_types.iter().cloned().collect(),
        },
        sorting: SortSettings {
            field: format!("{:?}", sort_field),
            direction: format!("{:?}", sort_direction),
        },
    }
}

pub fn dump_state_to_json(
    services: &[ServiceEntry],
    service_types: &[String],
    metrics: &BTreeMap<String, u64>,
    filter_query: &str,
    user_service_types: &HashSet<String>,
    sort_field: SortField,
    sort_direction: SortDirection,
) -> Result<String, Box<dyn std::error::Error>> {
    let dump = create_state_dump(
        services,
        service_types,
        metrics,
        filter_query,
        user_service_types,
        sort_field,
        sort_direction,
    );
    Ok(serde_json::to_string_pretty(&dump)?)
}

pub async fn save_json_dump(
    services: &[ServiceEntry],
    service_types: &[String],
    metrics: &BTreeMap<String, u64>,
    filter_query: &str,
    user_service_types: &HashSet<String>,
    sort_field: SortField,
    sort_direction: SortDirection,
) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.6f").to_string();
    let filename = format!("{}-state-dump.json", timestamp);
    let json_content = dump_state_to_json(
        services,
        service_types,
        metrics,
        filter_query,
        user_service_types,
        sort_field,
        sort_direction,
    )?;

    tokio::fs::write(&filename, json_content).await?;
    Ok(filename)
}

#[allow(clippy::too_many_arguments)]
pub fn load_from_state(
    services: &mut Vec<ServiceEntry>,
    service_types: &mut Vec<String>,
    metrics: &mut BTreeMap<String, u64>,
    filter_query: &mut String,
    user_service_types: &mut HashSet<String>,
    sort_field: &mut SortField,
    sort_direction: &mut SortDirection,
    dump: AppStateSnapshot,
) {
    *services = dump.services.iter().map(|s| s.into()).collect();
    *service_types = dump.service_types;
    *metrics = dump.metrics;
    *filter_query = dump.filters.query;
    *user_service_types = dump.filters.active_service_types.into_iter().collect();

    *sort_field = match dump.sorting.field.as_str() {
        "Host" => SortField::Host,
        "ServiceType" => SortField::ServiceType,
        "Fullname" => SortField::Fullname,
        "Port" => SortField::Port,
        "Address" => SortField::Address,
        "Timestamp" => SortField::Timestamp,
        _ => SortField::Host,
    };

    *sort_direction = match dump.sorting.direction.as_str() {
        "Ascending" => SortDirection::Ascending,
        "Descending" => SortDirection::Descending,
        _ => SortDirection::Ascending,
    };
}

pub fn parse_state_dump(
    json_content: &str,
) -> Result<AppStateSnapshot, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(json_content)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn create_test_service(name: &str, service_type: &str, port: u16) -> ServiceEntry {
        ServiceEntry {
            fullname: format!("{}.{}", name, service_type),
            host: format!("{}.local.", name),
            service_type: service_type.to_string(),
            subtype: None,
            addrs: vec![format!("192.168.1.{}", (port % 254) + 1)],
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

    #[test]
    fn test_state_dump_creation() {
        let services = vec![create_test_service("test", "_http._tcp.local.", 8080)];
        let service_types = vec!["_http._tcp.local.".to_string()];
        let metrics = BTreeMap::new();
        let filter_query = "".to_string();
        let user_service_types = HashSet::new();

        let snapshot = create_state_dump(
            &services,
            &service_types,
            &metrics,
            &filter_query,
            &user_service_types,
            SortField::Host,
            SortDirection::Ascending,
        );

        assert_eq!(snapshot.services.len(), 1);
        assert_eq!(snapshot.service_types.len(), 1);
        assert!(!snapshot.metadata.version.is_empty());
    }

    #[test]
    fn test_json_serialization_roundtrip() {
        let services = vec![create_test_service("test", "_http._tcp.local.", 8080)];
        let service_types = vec!["_http._tcp.local.".to_string()];
        let metrics = BTreeMap::new();
        let filter_query = "test".to_string();
        let user_service_types: HashSet<String> =
            vec!["_http._tcp.local.".to_string()].into_iter().collect();

        let json = dump_state_to_json(
            &services,
            &service_types,
            &metrics,
            &filter_query,
            &user_service_types,
            SortField::Host,
            SortDirection::Ascending,
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("services").is_some());
        assert!(parsed.get("metadata").is_some());
    }

    #[test]
    fn test_load_from_state() {
        let services = vec![create_test_service("test", "_http._tcp.local.", 8080)];
        let service_types = vec!["_http._tcp.local.".to_string()];
        let metrics = BTreeMap::new();
        let filter_query = "test".to_string();
        let user_service_types: HashSet<String> =
            vec!["_http._tcp.local.".to_string()].into_iter().collect();

        let snapshot = create_state_dump(
            &services,
            &service_types,
            &metrics,
            &filter_query,
            &user_service_types,
            SortField::Host,
            SortDirection::Ascending,
        );

        let mut loaded_services = Vec::new();
        let mut loaded_service_types = Vec::new();
        let mut loaded_metrics = BTreeMap::new();
        let mut loaded_filter_query = String::new();
        let mut loaded_user_types = HashSet::new();
        let mut loaded_sort_field = SortField::Timestamp;
        let mut loaded_sort_direction = SortDirection::Descending;

        load_from_state(
            &mut loaded_services,
            &mut loaded_service_types,
            &mut loaded_metrics,
            &mut loaded_filter_query,
            &mut loaded_user_types,
            &mut loaded_sort_field,
            &mut loaded_sort_direction,
            snapshot,
        );

        assert_eq!(loaded_services.len(), 1);
        assert_eq!(loaded_service_types.len(), 1);
        assert_eq!(loaded_filter_query, "test");
        assert!(loaded_user_types.contains("_http._tcp.local."));
    }
}

// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceSession {
    pub start_time: u64,
    pub end_time: Option<u64>,
}

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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableServiceSession {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub dump_timestamp: String,
    pub application_name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDump {
    pub metadata: Metadata,
    pub services: Vec<SerializableServiceEntry>,
    pub service_types: Vec<String>,
    pub metrics: BTreeMap<String, u64>,
    pub filters: FilterInfo,
    pub sorting: SortInfo,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterInfo {
    pub query: String,
    pub active_service_types: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortInfo {
    pub field: String,
    pub direction: String,
}

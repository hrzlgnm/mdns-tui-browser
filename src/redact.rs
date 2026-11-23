// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Heuristic redaction policy for JSON state dumps, so they can be shared
//! after review.
//!
//! Device fingerprints (tokens, keys, hashes), publicly reachable URLs and
//! public IPv6 addresses are masked; LAN data stays visible because it is
//! what a dump is meant to help debug. Markers match mex so dumps from both
//! tools read alike.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

use crate::models::SerializableServiceEntry;

/// Shannon entropy in bits per character.
fn shannon_entropy(s: &str) -> f64 {
    let mut counts = HashMap::new();
    let mut n = 0u64;
    for c in s.chars() {
        *counts.entry(c).or_insert(0u64) += 1;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    counts
        .values()
        .map(|&c| {
            let p = c as f64 / n as f64;
            -p * p.log2()
        })
        .sum()
}

/// Whether a TXT value looks like a token/key/hash: long, single-token
/// charset (hex/base64/UUID-style, no spaces), and dense. Short values
/// and human-readable text never match. The 3.2-bit floor sits low
/// enough to include repetitive hex device IDs, which carry less entropy
/// than random tokens but are fingerprints all the same.
fn is_high_entropy_value(v: &str) -> bool {
    v.len() >= 20
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-' | '.'))
        && shannon_entropy(v) >= 3.2
}

/// Whether an IP address is publicly routable (i.e. not private,
/// loopback, link-local, unique-local, multicast, or documentation).
/// IPv4-mapped IPv6 addresses are judged by their IPv4 part.
fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let first = v.octets()[0];
            // 0/8 ("this network") and 240/4 (reserved, broadcast) have
            // no stable std predicate.
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_multicast()
                || first == 0
                || first >= 240)
        }
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                return is_public_ip(&IpAddr::V4(v4));
            }
            let s = v.segments();
            // 2001:db8::/32 (documentation) has no stable std predicate.
            let is_documentation = s[0] == 0x2001 && s[1] == 0x0db8;
            !(is_documentation
                || v.is_loopback()
                || v.is_unspecified()
                || v.is_unicast_link_local()
                || v.is_unique_local()
                || v.is_multicast())
        }
    }
}

/// Whether a URL host is publicly reachable. Single-label names and
/// home-network suffixes (mDNS, homenet, router pseudo-TLDs) count as
/// private; anything else (e.g. a DynDNS hostname) counts as public.
fn is_public_host(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() {
        return false;
    }
    // `url` keeps IPv6 hosts bracketed in `host_str`.
    if let Ok(ip) = h
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
    {
        return is_public_ip(&ip);
    }
    if !h.contains('.') {
        return false;
    }
    ![
        ".local",
        ".localhost",
        ".invalid",
        ".internal",
        ".lan",
        ".home",
        ".home.arpa",
        ".box",
        ".repeater",
    ]
    .iter()
    .any(|suffix| h.ends_with(suffix))
}

/// Whether a TXT value is a URL (`scheme://...`) pointing at a publicly
/// reachable host. Parsing follows the WHATWG URL standard via the `url`
/// crate (userinfo, ports, bracketed IPv6); the public/private verdict is
/// local policy.
fn is_public_url(v: &str) -> bool {
    url::Url::parse(v)
        .ok()
        .and_then(|url| url.host_str().map(is_public_host))
        .unwrap_or(false)
}

/// The masked form of a public IPv6 literal, keeping the first and last
/// hextet (`2003:*REDACTED*:7927`) so prefix and host tail stay useful
/// for debugging; `None` for anything else. Global unicast addresses are
/// stable device fingerprints; link-local, unique-local, loopback,
/// multicast, documentation and plain IPv4 stay visible. An IPv4-mapped
/// address is masked when its IPv4 part is public. A `%scope` suffix is
/// ignored.
fn masked_public_ipv6(addr: &str) -> Option<String> {
    let unscoped = addr.trim().split('%').next().unwrap_or("");
    match unscoped.parse::<IpAddr>() {
        Ok(ip @ IpAddr::V6(v6)) if is_public_ip(&ip) => {
            let s = v6.segments();
            Some(format!("{:x}:*REDACTED*:{:x}", s[0], s[7]))
        }
        _ => None,
    }
}

/// Redact a bare public IPv6 literal (see `masked_public_ipv6`);
/// anything else passes through unchanged.
fn redact_ip_literal(addr: &str) -> String {
    masked_public_ipv6(addr).unwrap_or_else(|| addr.to_string())
}

/// Redact a TXT `key=value` pair, keeping the key so dumps stay
/// comparable: high-entropy tokens and publicly reachable URLs become
/// `key=*REDACTED*`, public IPv6 literals keep their first and last
/// hextet. Pairs without `=` or with ordinary values pass through.
fn redact_txt_value(pair: &str) -> String {
    let Some((k, v)) = pair.split_once('=') else {
        return pair.to_string();
    };
    if is_high_entropy_value(v) || is_public_url(v) {
        format!("{k}=*REDACTED*")
    } else if let Some(masked) = masked_public_ipv6(v) {
        format!("{k}={masked}")
    } else {
        pair.to_string()
    }
}

/// Redact a dumped service in place before it is shared in bug reports:
/// TXT pairs go through `redact_txt_value`, addresses through
/// `redact_ip_literal`. Values that become identical once masked are kept
/// once, in first-seen order. Everything else is left untouched.
pub fn redact_service_entry(entry: &mut SerializableServiceEntry) {
    entry.txt_records = redact_unique(&entry.txt_records, redact_txt_value);
    entry.addresses = redact_unique(&entry.addresses, redact_ip_literal);
}

fn redact_unique(values: &[String], redact: fn(&str) -> String) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|v| redact(v))
        .filter(|v| seen.insert(v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_txt_value_fingerprints_redacted_keys_kept() {
        assert_eq!(
            redact_txt_value("id=328FEBD442638273B38340E9AFE1E9CB"),
            "id=*REDACTED*"
        );
        // Repetitive hex: low entropy, but a fingerprint all the same.
        assert_eq!(
            redact_txt_value("id=8CED8754FF25F0C2D5A6E25CCD5E8A5F"),
            "id=*REDACTED*"
        );
        assert_eq!(
            redact_txt_value("pk=23d94723d5f4df84cfa99ae4e131ec1d6fe081864b7456de66a09ab037021af8"),
            "pk=*REDACTED*"
        );
        assert_eq!(
            redact_txt_value("gid=01518384-aa7a-4445-b1f2-1b0438a16d5a"),
            "gid=*REDACTED*"
        );
    }

    #[test]
    fn test_redact_txt_value_ordinary_values_unchanged() {
        assert_eq!(redact_txt_value("fn=Schreibtisch"), "fn=Schreibtisch");
        assert_eq!(redact_txt_value("rm="), "rm=");
        assert_eq!(redact_txt_value("bare-flag"), "bare-flag");
        assert_eq!(
            redact_txt_value("model=SHIELD Android TV"),
            "model=SHIELD Android TV"
        );
    }

    #[test]
    fn test_redact_txt_value_public_urls_redacted() {
        assert_eq!(
            redact_txt_value("external_url=https://ha.knulp.home64.de"),
            "external_url=*REDACTED*"
        );
        assert_eq!(
            redact_txt_value("base_url=https://example.com:443/a?b=c"),
            "base_url=*REDACTED*"
        );
        assert_eq!(
            redact_txt_value("x=http://user:pass@public.example.com:8080/a"),
            "x=*REDACTED*"
        );
        assert_eq!(
            redact_txt_value("x=rtsp://camera.example.com/stream"),
            "x=*REDACTED*"
        );
        assert_eq!(
            redact_txt_value("x=http://[2003:e8::1]:8123/"),
            "x=*REDACTED*"
        );
        assert_eq!(redact_txt_value("x=http://8.8.8.8/"), "x=*REDACTED*");
        assert_eq!(redact_txt_value("x=http://172.32.0.1/"), "x=*REDACTED*");
    }

    #[test]
    fn test_redact_txt_value_lan_urls_unchanged() {
        for pair in [
            "internal_url=http://192.168.178.27:8095",
            "path=http://fritz.box:49000/avmnexusdesc.xml",
            "x=http://[fe80::1]:8123/",
            "path=http://printer.local.:9100/x",
            "x=http://router.home.arpa/",
            "x=http://10.0.0.1/",
            "x=http://172.31.255.1/",
            "x=http://169.254.1.1/",
            "x=http://127.0.0.1:8080/",
            "x=http://0.0.0.0/",
            "x=http://255.255.255.255/",
            "x=http://[fd00::1]/",
            "x=http://[::1]/",
        ] {
            assert_eq!(redact_txt_value(pair), pair);
        }
    }

    #[test]
    fn test_redact_txt_value_public_ipv6_masked() {
        assert_eq!(
            redact_txt_value("ipv6=2003:e8:bf10:9500:38e1:a9ff:fe2c:7927"),
            "ipv6=2003:*REDACTED*:7927"
        );
        assert_eq!(
            redact_txt_value("ipv6=2003:e8:bf10:9500:38e1:a9ff:fe2c:7927%eth0"),
            "ipv6=2003:*REDACTED*:7927"
        );
        assert_eq!(
            redact_txt_value("ipv6=fe80::38e1:a9ff:fe2c:7927"),
            "ipv6=fe80::38e1:a9ff:fe2c:7927"
        );
        assert_eq!(redact_txt_value("ip6=fd00::1"), "ip6=fd00::1");
    }

    #[test]
    fn test_redact_ip_literal_public_ipv6_keeps_outer_hextets() {
        assert_eq!(
            redact_ip_literal("2003:e8:bf10:9500:38e1:a9ff:fe2c:7927"),
            "2003:*REDACTED*:7927"
        );
        assert_eq!(
            redact_ip_literal("2003:e8:bf10:9500:38e1:a9ff:fe2c:7927%eth0"),
            "2003:*REDACTED*:7927"
        );
    }

    #[test]
    fn test_redact_ip_literal_mapped_public_ipv4_masked() {
        assert_eq!(redact_ip_literal("::ffff:8.8.8.8"), "0:*REDACTED*:808");
    }

    #[test]
    fn test_redact_ip_literal_non_public_unchanged() {
        for addr in [
            "::1",
            "::ffff:192.168.1.1",
            "2001:db8::1",
            "fe80::1%eth0",
            "fd00::1",
            "ff02::fb",
            "192.168.178.30",
            "8.8.8.8",
        ] {
            assert_eq!(redact_ip_literal(addr), addr);
        }
    }
}

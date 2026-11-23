// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Redaction policy that makes JSON state dumps safe to share.
//!
//! Device fingerprints (tokens, keys, hashes), publicly reachable URLs and
//! public IPv6 addresses are masked; LAN data stays visible because it is
//! what a dump is meant to help debug. Markers match mex so dumps from both
//! tools read alike.

use std::collections::HashMap;
use std::net::IpAddr;

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
/// and human-readable text never match. The 3.2 floor also catches
/// repetitive hex device IDs, which carry less entropy but are
/// fingerprints all the same.
fn is_high_entropy_value(v: &str) -> bool {
    v.len() >= 20
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-' | '.'))
        && shannon_entropy(v) >= 3.2
}

/// Whether an IP address is publicly routable (i.e. not private,
/// loopback, link-local, unique-local, or multicast).
fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(o[0] == 10
                || (o[0] == 172 && (16..32).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || o[0] == 127
                || (o[0] == 169 && o[1] == 254)
                || o[0] == 0
                || o[0] >= 224)
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            !(v.is_loopback()
                || v.is_unspecified()
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xff00) == 0xff00)
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

/// Whether a string is a public IPv6 literal. Global unicast device
/// addresses are stable fingerprints; link-local, unique-local, loopback
/// and multicast stay visible.
fn is_public_ipv6_literal(v: &str) -> bool {
    matches!(v.trim().parse::<IpAddr>(), Ok(ip @ IpAddr::V6(_)) if is_public_ip(&ip))
}

/// Redact a bare public IPv6 literal, keeping the first and last hextet
/// (`2003:*REDACTED*:7927`) so prefix and host tail stay useful for
/// debugging. A `%scope` suffix is ignored for the decision. Anything
/// else (including IPv4 and non-public IPv6) passes through unchanged.
pub fn redact_ip_literal(addr: &str) -> String {
    let unscoped = addr.trim().split('%').next().unwrap_or("");
    match unscoped.parse::<IpAddr>() {
        Ok(IpAddr::V6(ip)) if is_public_ip(&IpAddr::V6(ip)) => {
            let s = ip.segments();
            format!("{:x}:*REDACTED*:{:x}", s[0], s[7])
        }
        _ => addr.to_string(),
    }
}

/// Redact a TXT `key=value` pair, keeping the key so dumps stay
/// comparable: high-entropy tokens and publicly reachable URLs become
/// `key=*REDACTED*`, public IPv6 literals keep their first and last
/// hextet. Pairs without `=` or with ordinary values pass through.
pub fn redact_txt_value(pair: &str) -> String {
    match pair.split_once('=') {
        Some((k, v)) if is_high_entropy_value(v) || is_public_url(v) => {
            format!("{k}=*REDACTED*")
        }
        Some((k, v)) if is_public_ipv6_literal(v) => {
            format!("{k}={}", redact_ip_literal(v))
        }
        _ => pair.to_string(),
    }
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
    }

    #[test]
    fn test_redact_txt_value_lan_urls_unchanged() {
        for pair in [
            "internal_url=http://192.168.178.27:8095",
            "path=http://fritz.box:49000/avmnexusdesc.xml",
            "x=http://[fe80::1]:8123/",
            "path=http://printer.local.:9100/x",
            "x=http://router.home.arpa/",
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
    fn test_redact_ip_literal_non_public_unchanged() {
        for addr in [
            "::1",
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

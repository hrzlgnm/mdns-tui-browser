// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Parsing and typo-tolerant matching of the `/` quick-filter query.

use std::cmp::Ordering;

use frizbee::{CaseMatching, Config, Matcher, Pattern, PatternConfig};

use crate::models::{ServiceEntry, format_service_addrs};

/// Shortest text term that may contain a typo; shorter terms would match
/// too broadly with one.
const MIN_TYPO_TOLERANT_TERM_CHARS: usize = 4;

/// How well a service matched the text terms of a [`QuickFilter`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Relevance {
    /// Every text term occurs contiguously in the searchable text.
    contiguous: bool,
    score: u16,
}

impl Relevance {
    /// Orders more relevant matches first.
    pub fn cmp_best_first(&self, other: &Self) -> Ordering {
        other
            .contiguous
            .cmp(&self.contiguous)
            .then_with(|| other.score.cmp(&self.score))
    }
}

/// Reserved status words; each one present must hold for a service.
#[derive(Debug, Default)]
struct StatusConstraints {
    online: bool,
    offline: bool,
    flapping: bool,
}

impl StatusConstraints {
    /// Records `term` and returns true if it is a reserved status word.
    fn add(&mut self, term: &str) -> bool {
        let flag = match term {
            "online" => &mut self.online,
            "offline" => &mut self.offline,
            "flapping" => &mut self.flapping,
            _ => return false,
        };
        *flag = true;
        true
    }

    fn admits(&self, service: &ServiceEntry) -> bool {
        (!self.online || service.online)
            && (!self.offline || !service.online)
            && (!self.flapping || service.is_flapping)
    }
}

/// A parsed quick-filter query: status constraints plus fuzzy text terms,
/// all of which must match.
pub struct QuickFilter {
    status: StatusConstraints,
    terms: Vec<String>,
    /// `None` when the query has no text terms.
    matcher: Option<Matcher>,
}

impl QuickFilter {
    /// Parses whitespace-separated terms case-insensitively. Terms are
    /// matched literally: frizbee's query operators are not exposed.
    pub fn parse(query: &str) -> Self {
        let mut status = StatusConstraints::default();
        let terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .filter(|term| !status.add(term))
            .map(str::to_string)
            .collect();

        let matcher = (!terms.is_empty()).then(|| {
            let patterns: Vec<Pattern> = terms
                .iter()
                .map(|term| {
                    let max_typos = u16::from(term.chars().count() >= MIN_TYPO_TOLERANT_TERM_CHARS);
                    Pattern::new(term, PatternConfig::default().max_typos(Some(max_typos)))
                })
                .collect();
            Matcher::from_patterns(&patterns, &Config::default().casing(CaseMatching::Ignore))
        });

        Self {
            status,
            terms,
            matcher,
        }
    }

    /// Returns how well `service` matches, or `None` if it does not.
    pub fn relevance(&mut self, service: &ServiceEntry) -> Option<Relevance> {
        if !self.status.admits(service) {
            return None;
        }
        let Some(matcher) = self.matcher.as_mut() else {
            return Some(Relevance::default());
        };
        let text = searchable_text(service);
        let found = matcher.match_one(&text, 0)?;
        Some(Relevance {
            contiguous: self.terms.iter().all(|term| text.contains(term.as_str())),
            score: found.score,
        })
    }
}

/// All searchable service fields, lowercased and space-separated.
fn searchable_text(service: &ServiceEntry) -> String {
    [
        service.fullname.as_str(),
        service.host.as_str(),
        service.service_type.as_str(),
        &format_service_addrs(&service.addrs, " "),
        &service.port.to_string(),
        &service.txt.join(" "),
        service.subtype.as_deref().unwrap_or_default(),
    ]
    .join(" ")
    .to_lowercase()
}

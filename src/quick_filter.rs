// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Parsing and typo-tolerant matching of the `/` quick-filter query.

use std::cmp::Ordering;

use frizbee::{CaseMatching, Config, Matcher, Pattern, PatternConfig, SortStrategy};

use crate::models::{ServiceEntry, format_scoped_ip_with_context};

/// Shortest text term that may contain a typo; shorter terms would match
/// too broadly with one.
const MIN_TYPO_TOLERANT_TERM_CHARS: usize = 4;

/// How well a service matched the text terms of a [`QuickFilter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Relevance {
    /// Every text term occurs contiguously in one of the service's fields.
    contiguous: bool,
    /// Sum of each term's best fuzzy score over the service's fields.
    score: u16,
}

impl Default for Relevance {
    /// Relevance of a service when the query has no text terms.
    fn default() -> Self {
        Self {
            contiguous: true,
            score: 0,
        }
    }
}

impl Relevance {
    /// Orders contiguous matches first, then fuzzy-only matches by
    /// descending score. Contiguous matches compare equal so the caller's
    /// sort decides their order.
    pub fn cmp_best_first(&self, other: &Self) -> Ordering {
        match (self.contiguous, other.contiguous) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => other.score.cmp(&self.score),
        }
    }

    /// Combines the relevance of two terms that must both match.
    fn and(self, other: Self) -> Self {
        Self {
            contiguous: self.contiguous && other.contiguous,
            score: self.score.saturating_add(other.score),
        }
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

/// One text term of a quick-filter query and its compiled matcher.
struct Term {
    text: String,
    matcher: Matcher,
}

impl Term {
    /// Terms are matched literally: frizbee's query operators are not
    /// exposed.
    fn new(text: &str) -> Self {
        let max_typos = u16::from(text.chars().count() >= MIN_TYPO_TOLERANT_TERM_CHARS);
        let pattern = Pattern::new(text, PatternConfig::default().max_typos(Some(max_typos)));
        let config = Config::default()
            .casing(CaseMatching::Ignore)
            .sort(SortStrategy::IndexAsc);
        Self {
            text: text.to_string(),
            matcher: Matcher::new(pattern, &config),
        }
    }

    /// Best relevance per owner, where `owners[i]` owns `fields[i]`.
    fn best_per_owner(
        &mut self,
        fields: &[String],
        owners: &[usize],
        owner_count: usize,
    ) -> Vec<Option<Relevance>> {
        let mut best: Vec<Option<Relevance>> = vec![None; owner_count];
        for found in self.matcher.match_list(fields) {
            let field = found.index as usize;
            let candidate = Relevance {
                contiguous: fields[field].contains(&self.text),
                score: found.score,
            };
            let slot = &mut best[owners[field]];
            *slot = Some(match *slot {
                Some(current) => Relevance {
                    contiguous: current.contiguous || candidate.contiguous,
                    score: current.score.max(candidate.score),
                },
                None => candidate,
            });
        }
        best
    }
}

/// A parsed quick-filter query: status constraints plus fuzzy text terms,
/// all of which must match.
pub struct QuickFilter {
    status: StatusConstraints,
    terms: Vec<Term>,
}

impl QuickFilter {
    /// Parses whitespace-separated terms case-insensitively.
    pub fn parse(query: &str) -> Self {
        let mut status = StatusConstraints::default();
        let terms = query
            .to_lowercase()
            .split_whitespace()
            .filter(|term| !status.add(term))
            .map(Term::new)
            .collect();
        Self { status, terms }
    }

    /// Returns the ids of the matching candidates with their relevance.
    ///
    /// Each term is matched against every field separately, so a fuzzy
    /// match or typo never spans two fields.
    pub fn rank<'a>(
        &mut self,
        candidates: impl IntoIterator<Item = (usize, &'a ServiceEntry)>,
    ) -> Vec<(usize, Relevance)> {
        let admitted: Vec<(usize, &ServiceEntry)> = candidates
            .into_iter()
            .filter(|(_, service)| self.status.admits(service))
            .collect();

        let mut fields = Vec::new();
        let mut owners = Vec::new();
        for (owner, (_, service)) in admitted.iter().enumerate() {
            for field in searchable_fields(service) {
                fields.push(field);
                owners.push(owner);
            }
        }

        let mut relevance = vec![Some(Relevance::default()); admitted.len()];
        for term in &mut self.terms {
            let best = term.best_per_owner(&fields, &owners, admitted.len());
            for (combined, term_best) in relevance.iter_mut().zip(best) {
                *combined = combined.zip(term_best).map(|(a, b)| a.and(b));
            }
        }

        admitted
            .into_iter()
            .zip(relevance)
            .filter_map(|((id, _), relevance)| Some((id, relevance?)))
            .collect()
    }
}

/// Every searchable service field, lowercased.
fn searchable_fields(service: &ServiceEntry) -> Vec<String> {
    let mut fields = vec![
        service.fullname.to_lowercase(),
        service.host.to_lowercase(),
        service.service_type.to_lowercase(),
        service.port.to_string(),
    ];
    fields.extend(service.subtype.iter().map(|subtype| subtype.to_lowercase()));
    fields.extend(
        service
            .addrs
            .iter()
            .map(|addr| format_scoped_ip_with_context(addr, &service.addrs).to_lowercase()),
    );
    fields.extend(service.txt.iter().map(|entry| entry.to_lowercase()));
    fields
}

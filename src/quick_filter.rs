// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Parsing and typo-tolerant matching of the `/` quick-filter query.

use std::cmp::Ordering;

use frizbee::{CaseMatching, Config, Matcher, Pattern, PatternConfig, SortStrategy};

use crate::models::{ServiceEntry, formatted_service_addrs};

/// Shortest text term that may contain a typo; shorter terms would match
/// too broadly with one.
const MIN_TYPO_TOLERANT_TERM_CHARS: usize = 4;

/// How well a service matched the text terms of a [`QuickFilter`].
#[derive(Clone, Copy, Debug)]
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
#[derive(Clone, Debug, Default)]
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
#[derive(Clone)]
struct Term {
    text: String,
    typo_tolerant: bool,
    matcher: Matcher,
}

impl Term {
    /// Terms are matched literally: frizbee's query operators are not
    /// exposed.
    fn new(text: &str) -> Self {
        let typo_tolerant = text.chars().count() >= MIN_TYPO_TOLERANT_TERM_CHARS;
        let pattern = Pattern::new(
            text,
            PatternConfig::default().max_typos(Some(u16::from(typo_tolerant))),
        );
        let config = Config::default()
            .casing(CaseMatching::Ignore)
            .sort(SortStrategy::IndexAsc);
        Self {
            text: text.to_string(),
            typo_tolerant,
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
        for (field, score) in self.field_scores(fields) {
            let candidate = Relevance {
                contiguous: fields[field].contains(&self.text),
                score,
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

    /// Score of every matching field, as `(field index, score)`.
    fn field_scores(&mut self, fields: &[String]) -> Vec<(usize, u16)> {
        if !self.typo_tolerant {
            return self
                .matcher
                .match_list(fields)
                .into_iter()
                .map(|found| (found.index as usize, found.score))
                .collect();
        }
        let mut scores = Vec::new();
        for found in self.matcher.match_list_indices(fields) {
            let field = found.index as usize;
            if self.is_compact_or_typo_free(&found.indices) {
                scores.push((field, found.score));
            } else if let Some(score) = self.best_compact_score(&fields[field]) {
                scores.push((field, score));
            }
        }
        scores
    }

    /// Rejects a typo match whose matched characters are spread out:
    /// frizbee counts a typo as a missing term byte, so a spread-out match
    /// only shares a few scattered characters with the term (`http` against
    /// `printer._ipp._tcp`). Matches without a typo may spread out, so `prn`
    /// still finds `printer`.
    fn is_compact_or_typo_free(&self, matched_byte_offsets: &[u32]) -> bool {
        let has_typo = matched_byte_offsets.len() < self.text.len();
        let (Some(&first), Some(&last)) = (
            matched_byte_offsets.iter().min(),
            matched_byte_offsets.iter().max(),
        ) else {
            return false;
        };
        let span = (last - first) as usize + 1;
        !has_typo || span <= self.max_typo_span()
    }

    /// Widest byte span a typo match may cover.
    fn max_typo_span(&self) -> usize {
        self.text.len() + 1
    }

    /// Best score of a compact match anywhere in `field`. frizbee reports
    /// only the highest-scoring alignment per field, which may be a
    /// spread-out typo match hiding a compact one elsewhere, so match every
    /// window of the widest allowed span instead.
    fn best_compact_score(&mut self, field: &str) -> Option<u16> {
        let windows: Vec<&str> = field
            .char_indices()
            .map(|(start, _)| {
                let mut end = (start + self.max_typo_span()).min(field.len());
                while !field.is_char_boundary(end) {
                    end -= 1;
                }
                &field[start..end]
            })
            .collect();
        self.matcher
            .match_list(&windows)
            .into_iter()
            .map(|found| found.score)
            .max()
    }
}

/// A parsed quick-filter query: status constraints plus fuzzy text terms,
/// all of which must match.
#[derive(Clone)]
pub struct QuickFilter {
    query: String,
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
        Self {
            query: query.to_string(),
            status,
            terms,
        }
    }

    /// The query text this filter was parsed from.
    pub fn query(&self) -> &str {
        &self.query
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
        if self.terms.is_empty() {
            return admitted
                .into_iter()
                .map(|(id, _)| (id, Relevance::default()))
                .collect();
        }

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
    fields.extend(formatted_service_addrs(&service.addrs).map(|addr| addr.to_lowercase()));
    fields.extend(service.txt.iter().map(|entry| entry.to_lowercase()));
    fields
}

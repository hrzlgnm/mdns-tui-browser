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

/// How closely a term matched, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MatchTier {
    /// The term occurs contiguously.
    Contiguous,
    /// The term's characters occur in order, possibly spread out.
    Fuzzy,
    /// All but one of the term's characters occur close together.
    Typo,
}

/// How well a service matched the text terms of a [`QuickFilter`].
#[derive(Clone, Copy, Debug)]
pub struct Relevance {
    tier: MatchTier,
    /// Sum of each term's best score; scores are only comparable within a
    /// tier because typo matches are scored on short windows.
    score: u16,
}

impl Default for Relevance {
    /// Relevance of a service when the query has no text terms.
    fn default() -> Self {
        Self {
            tier: MatchTier::Contiguous,
            score: 0,
        }
    }
}

impl Relevance {
    /// Orders by tier, then fuzzy and typo matches by descending score.
    /// Contiguous matches compare equal so the caller's sort decides their
    /// order.
    pub fn cmp_best_first(&self, other: &Self) -> Ordering {
        self.tier.cmp(&other.tier).then_with(|| match self.tier {
            MatchTier::Contiguous => Ordering::Equal,
            MatchTier::Fuzzy | MatchTier::Typo => other.score.cmp(&self.score),
        })
    }

    /// The better of two matches of the same term.
    fn best_of(self, other: Self) -> Self {
        match self.tier.cmp(&other.tier) {
            Ordering::Less => self,
            Ordering::Greater => other,
            Ordering::Equal => Self {
                tier: self.tier,
                score: self.score.max(other.score),
            },
        }
    }

    /// Combines the relevance of two terms that must both match; the
    /// weaker tier decides.
    fn and(self, other: Self) -> Self {
        Self {
            tier: self.tier.max(other.tier),
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

/// One text term of a quick-filter query and its compiled matchers.
#[derive(Clone)]
struct Term {
    text: String,
    exact: Matcher,
    /// Allows one typo; `None` for terms too short to tolerate one.
    typo: Option<Matcher>,
}

impl Term {
    /// Terms are matched literally: frizbee's query operators are not
    /// exposed.
    fn new(text: &str) -> Self {
        let config = Config::default()
            .casing(CaseMatching::Ignore)
            .sort(SortStrategy::IndexAsc);
        let matcher = |max_typos| {
            let pattern = Pattern::new(text, PatternConfig::default().max_typos(Some(max_typos)));
            Matcher::new(pattern, &config)
        };
        Self {
            text: text.to_string(),
            exact: matcher(0),
            typo: (text.chars().count() >= MIN_TYPO_TOLERANT_TERM_CHARS).then(|| matcher(1)),
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
        for (field, relevance) in self.field_matches(fields) {
            let slot = &mut best[owners[field]];
            *slot = Some(slot.map_or(relevance, |current| current.best_of(relevance)));
        }
        best
    }

    /// Relevance of every matching field, as `(field index, relevance)`.
    fn field_matches(&mut self, fields: &[String]) -> Vec<(usize, Relevance)> {
        let mut matches: Vec<(usize, Relevance)> = self
            .exact
            .match_list(fields)
            .into_iter()
            .map(|found| {
                let field = found.index as usize;
                let tier = if fields[field].contains(&self.text) {
                    MatchTier::Contiguous
                } else {
                    MatchTier::Fuzzy
                };
                (
                    field,
                    Relevance {
                        tier,
                        score: found.score,
                    },
                )
            })
            .collect();

        let Some(typo) = self.typo.as_mut() else {
            return matches;
        };
        let mut matched = vec![false; fields.len()];
        for &(field, _) in &matches {
            matched[field] = true;
        }
        let unmatched: Vec<usize> = (0..fields.len()).filter(|&field| !matched[field]).collect();
        let unmatched_fields: Vec<&str> = unmatched
            .iter()
            .map(|&field| fields[field].as_str())
            .collect();
        let max_span = self.text.len() + 1;
        for found in typo.match_list(&unmatched_fields) {
            let field = unmatched[found.index as usize];
            if let Some(score) = best_window_score(typo, &fields[field], max_span) {
                matches.push((
                    field,
                    Relevance {
                        tier: MatchTier::Typo,
                        score,
                    },
                ));
            }
        }
        matches
    }
}

/// Best score of `matcher` over every window of `field` at most
/// `max_span` bytes long, or `None` if no window matches.
///
/// frizbee counts a typo as a missing term character and lets the rest
/// spread over the whole field, so one typo would turn `http` into any
/// `t..t..p`. Matching windows one byte longer than the term keeps typo
/// matches close together and scores them all the same way.
fn best_window_score(matcher: &mut Matcher, field: &str, max_span: usize) -> Option<u16> {
    let windows: Vec<&str> = field
        .char_indices()
        .map(|(start, _)| {
            let mut end = (start + max_span).min(field.len());
            while !field.is_char_boundary(end) {
                end -= 1;
            }
            &field[start..end]
        })
        .collect();
    matcher
        .match_list(&windows)
        .into_iter()
        .map(|found| found.score)
        .max()
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

// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Parsing and typo-tolerant matching of the `/` quick-filter query.

use std::cmp::Ordering;

use frizbee::{CaseMatching, Config, Matcher, Pattern, PatternConfig, SortStrategy};

use crate::models::{ServiceEntry, formatted_service_addrs};

/// Shortest term that may contain a typo.
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
    /// Sum of each term's best score. Typo matches are scored on short
    /// windows, which earn frizbee's start-of-text bonus, so typo-tier
    /// sums are only roughly comparable with each other.
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
    /// The constraint `term` names, or `None` if it is not a status word.
    fn flag_mut(&mut self, term: &str) -> Option<&mut bool> {
        match term {
            "online" => Some(&mut self.online),
            "offline" => Some(&mut self.offline),
            "flapping" => Some(&mut self.flapping),
            _ => None,
        }
    }

    fn admits(&self, service: &ServiceEntry) -> bool {
        (!self.online || service.online)
            && (!self.offline || !service.online)
            && (!self.flapping || service.is_flapping)
    }
}

/// Searchable fields of candidate services, each tagged with its owner.
struct Haystack {
    fields: Vec<String>,
    /// Owner index of each field in `fields`.
    owners: Vec<usize>,
    owner_count: usize,
}

impl Haystack {
    fn new(services: &[&ServiceEntry]) -> Self {
        let mut fields = Vec::new();
        let mut owners = Vec::new();
        for (owner, service) in services.iter().enumerate() {
            for field in searchable_fields(service) {
                fields.push(field);
                owners.push(owner);
            }
        }
        Self {
            fields,
            owners,
            owner_count: services.len(),
        }
    }

    /// Drops the fields of owners that `keep` rejects.
    fn retain_owners(&mut self, keep: impl Fn(usize) -> bool) {
        let (fields, owners) = std::mem::take(&mut self.fields)
            .into_iter()
            .zip(std::mem::take(&mut self.owners))
            .filter(|&(_, owner)| keep(owner))
            .unzip();
        self.fields = fields;
        self.owners = owners;
    }
}

/// Whether `term` may match with one typo. Short terms would match too
/// broadly, and in a port or address one digit off names a different
/// service, so numeric and IP-like terms must match without a typo.
fn tolerates_typo(term: &str) -> bool {
    // Addresses are shown with a `%scope` suffix, e.g. `fe80::1%eth0`
    let address = term.split_once('%').map_or(term, |(address, _)| address);
    let is_numeric = address
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | ':'));
    let is_ipv6_like = address.contains(':')
        && address
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, '.' | ':'));
    term.chars().count() >= MIN_TYPO_TOLERANT_TERM_CHARS && !is_numeric && !is_ipv6_like
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
    /// exposed. Terms and fields are lowercased up front, so matching can
    /// respect case.
    fn new(text: &str) -> Self {
        let config = Config::default()
            .casing(CaseMatching::Respect)
            .sort(SortStrategy::IndexAsc);
        let matcher = |max_typos| {
            let pattern = Pattern::new(text, PatternConfig::default().max_typos(Some(max_typos)));
            Matcher::new(pattern, &config)
        };
        Self {
            text: text.to_string(),
            exact: matcher(0),
            typo: tolerates_typo(text).then(|| matcher(1)),
        }
    }

    /// Best relevance per owner of `haystack`.
    fn best_per_owner(&mut self, haystack: &Haystack) -> Vec<Option<Relevance>> {
        let mut best: Vec<Option<Relevance>> = vec![None; haystack.owner_count];
        let keep_best = |best: &mut [Option<Relevance>], field: usize, relevance: Relevance| {
            let slot = &mut best[haystack.owners[field]];
            *slot = Some(slot.map_or(relevance, |current| current.best_of(relevance)));
        };

        for (field, relevance) in self.typo_free_matches(&haystack.fields) {
            keep_best(&mut best, field, relevance);
        }
        // A typo match never beats a typo-free one, so only owners without
        // any match need the typo pass.
        let unmatched: Vec<usize> = (0..haystack.fields.len())
            .filter(|&field| best[haystack.owners[field]].is_none())
            .collect();
        for (field, relevance) in self.typo_matches(&haystack.fields, &unmatched) {
            keep_best(&mut best, field, relevance);
        }
        best
    }

    /// Fields the term matches without a typo, as `(field, relevance)`.
    fn typo_free_matches(&mut self, fields: &[String]) -> Vec<(usize, Relevance)> {
        self.exact
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
            .collect()
    }

    /// Fields among `candidates` the term matches with one typo, as
    /// `(field, relevance)`.
    ///
    /// frizbee counts a typo as a missing term character and lets the rest
    /// spread over the whole field, so one typo would turn `http` into any
    /// `t..t..p`. Matching windows one byte longer than the term keeps typo
    /// matches close together and scores them all the same way.
    fn typo_matches(&mut self, fields: &[String], candidates: &[usize]) -> Vec<(usize, Relevance)> {
        let Some(typo) = self.typo.as_mut() else {
            return Vec::new();
        };
        let candidate_fields: Vec<&str> = candidates
            .iter()
            .map(|&field| fields[field].as_str())
            .collect();
        let max_span = self.text.len() + 1;
        let mut windows = Vec::new();
        let mut window_fields = Vec::new();
        for found in typo.match_list(&candidate_fields) {
            let field = candidates[found.index as usize];
            for window in windows_of(&fields[field], &self.text, max_span) {
                windows.push(window);
                window_fields.push(field);
            }
        }

        let mut best: Vec<Option<u16>> = vec![None; fields.len()];
        for found in typo.match_list(&windows) {
            let slot = &mut best[window_fields[found.index as usize]];
            *slot = Some(slot.map_or(found.score, |score| score.max(found.score)));
        }
        best.into_iter()
            .enumerate()
            .filter_map(|(field, score)| {
                let relevance = Relevance {
                    tier: MatchTier::Typo,
                    score: score?,
                };
                Some((field, relevance))
            })
            .collect()
    }
}

/// Every substring of `field` at most `max_span` bytes long that starts
/// with the first or second character of `term`. A one-typo alignment
/// begins with one of them, so other start positions only repeat work.
fn windows_of<'a>(field: &'a str, term: &str, max_span: usize) -> impl Iterator<Item = &'a str> {
    let mut leading = term.chars();
    let (first, second) = (leading.next(), leading.next());
    field
        .char_indices()
        .filter(move |&(_, c)| Some(c) == first || Some(c) == second)
        .map(move |(start, _)| {
            let mut end = (start + max_span).min(field.len());
            while !field.is_char_boundary(end) {
                end -= 1;
            }
            &field[start..end]
        })
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
        let mut terms = Vec::new();
        for term in query.to_lowercase().split_whitespace() {
            match status.flag_mut(term) {
                Some(flag) => *flag = true,
                None => terms.push(Term::new(term)),
            }
        }
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

        let services: Vec<&ServiceEntry> = admitted.iter().map(|&(_, service)| service).collect();
        let mut haystack = Haystack::new(&services);
        let mut relevance = vec![Some(Relevance::default()); admitted.len()];
        for term in &mut self.terms {
            let best = term.best_per_owner(&haystack);
            for (combined, term_best) in relevance.iter_mut().zip(best) {
                *combined = combined.zip(term_best).map(|(a, b)| a.and(b));
            }
            haystack.retain_owners(|owner| relevance[owner].is_some());
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

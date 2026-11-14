// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Parsing and typo-tolerant matching of the `/` quick-filter query.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use frizbee::{CaseMatching, Config, Matcher, Pattern, PatternConfig, SortStrategy};
use mdns_sd::ScopedIp;

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

/// Everything the quick filter searches in a service.
#[derive(Clone, Debug)]
struct SearchSource {
    fullname: String,
    host: String,
    service_type: String,
    subtype: Option<String>,
    addrs: Vec<ScopedIp>,
    port: u16,
    txt: Vec<String>,
}

impl SearchSource {
    fn of(service: &ServiceEntry) -> Self {
        Self {
            fullname: service.fullname.clone(),
            host: service.host.clone(),
            service_type: service.service_type.clone(),
            subtype: service.subtype.clone(),
            addrs: service.addrs.clone(),
            port: service.port,
            txt: service.txt.clone(),
        }
    }

    /// Whether `service` still has this searchable content.
    fn matches(&self, service: &ServiceEntry) -> bool {
        // Exhaustive so a new searchable field cannot be left out
        let Self {
            fullname,
            host,
            service_type,
            subtype,
            addrs,
            port,
            txt,
        } = self;
        *fullname == service.fullname
            && *host == service.host
            && *service_type == service.service_type
            && *subtype == service.subtype
            && *addrs == service.addrs
            && *port == service.port
            && *txt == service.txt
    }

    /// Every searchable field, lowercased.
    fn fields(&self) -> Vec<String> {
        let mut fields = vec![
            self.fullname.to_lowercase(),
            self.host.to_lowercase(),
            self.service_type.to_lowercase(),
            self.port.to_string(),
        ];
        fields.extend(self.subtype.iter().map(|subtype| subtype.to_lowercase()));
        fields.extend(formatted_service_addrs(&self.addrs).map(|addr| addr.to_lowercase()));
        fields.extend(self.txt.iter().map(|entry| entry.to_lowercase()));
        fields
    }
}

#[derive(Clone, Debug)]
struct CachedFields {
    source: SearchSource,
    fields: Vec<String>,
}

/// Lowercased searchable fields of services by fullname, reused while a
/// service's searchable content is unchanged.
#[derive(Clone, Debug, Default)]
pub struct SearchFieldCache {
    by_fullname: HashMap<String, CachedFields>,
}

impl SearchFieldCache {
    /// Searchable fields of each of `services`, refreshing stale entries.
    fn fields_of<'c>(&'c mut self, services: &[&ServiceEntry]) -> Vec<Cow<'c, [String]>> {
        for service in services {
            let fresh = self
                .by_fullname
                .get(&service.fullname)
                .is_some_and(|cached| cached.source.matches(service));
            if !fresh {
                let source = SearchSource::of(service);
                let fields = source.fields();
                self.by_fullname
                    .insert(service.fullname.clone(), CachedFields { source, fields });
            }
        }
        let cache = &*self;
        services
            .iter()
            .map(|service| match cache.by_fullname.get(&service.fullname) {
                Some(cached) if cached.source.matches(service) => {
                    Cow::Borrowed(cached.fields.as_slice())
                }
                // Another service with the same fullname replaced the entry
                _ => Cow::Owned(SearchSource::of(service).fields()),
            })
            .collect()
    }

    /// Drops entries of services that are gone, once the cache holds more
    /// entries than there are services.
    pub fn prune(&mut self, services: &[ServiceEntry]) {
        if self.by_fullname.len() <= services.len() {
            return;
        }
        let live: HashSet<&str> = services
            .iter()
            .map(|service| service.fullname.as_str())
            .collect();
        self.by_fullname
            .retain(|fullname, _| live.contains(fullname.as_str()));
    }

    /// Number of cached services.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_fullname.len()
    }
}

/// Searchable fields of candidate services, each tagged with its owner.
struct Haystack<'a> {
    fields: Vec<&'a str>,
    /// Owner index of each field in `fields`.
    owners: Vec<usize>,
    owner_count: usize,
}

impl<'a> Haystack<'a> {
    fn new(field_lists: &'a [Cow<'a, [String]>]) -> Self {
        let mut fields = Vec::new();
        let mut owners = Vec::new();
        for (owner, list) in field_lists.iter().enumerate() {
            for field in list.iter() {
                fields.push(field.as_str());
                owners.push(owner);
            }
        }
        Self {
            fields,
            owners,
            owner_count: field_lists.len(),
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
    let is_numeric = !address.is_empty()
        && address
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
    fn typo_free_matches(&mut self, fields: &[&str]) -> Vec<(usize, Relevance)> {
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
    fn typo_matches(&mut self, fields: &[&str], candidates: &[usize]) -> Vec<(usize, Relevance)> {
        let Some(typo) = self.typo.as_mut() else {
            return Vec::new();
        };
        let candidate_fields: Vec<&str> = candidates.iter().map(|&field| fields[field]).collect();
        let max_span = self.text.len() + 1;
        let mut windows = Vec::new();
        let mut window_fields = Vec::new();
        for found in typo.match_list(&candidate_fields) {
            let field = candidates[found.index as usize];
            for window in windows_of(fields[field], &self.text, max_span) {
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

/// From each occurrence of the first or second character of `term`, the
/// substring of `field` up to `max_span` bytes long. A one-typo alignment
/// begins with one of those characters, so other start positions only
/// repeat work.
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

    /// Whether the query has text terms, i.e. whether results are ranked
    /// by relevance rather than only by the selected sort.
    pub fn has_text_terms(&self) -> bool {
        !self.terms.is_empty()
    }

    /// Returns the ids of the matching candidates with their relevance.
    ///
    /// Each term is matched against every field separately, so a fuzzy
    /// match or typo never spans two fields.
    pub fn rank<'a>(
        &mut self,
        candidates: impl IntoIterator<Item = (usize, &'a ServiceEntry)>,
        cache: &mut SearchFieldCache,
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
        let field_lists = cache.fields_of(&services);
        let mut haystack = Haystack::new(&field_lists);
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

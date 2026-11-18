//! Select disjoint source contributions without treating source claims as provider attestation.
use super::normalize::{
    Attribution, Contribution, Counters, Observation, Quality, Scope, add, coverage_error,
};
use super::{Result, conflict};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize)]
pub(super) struct Summary {
    pub observation_count: usize,
    pub contribution_count: usize,
    pub counters: Counters,
    pub exclusive_counters: Counters,
    pub shared_counters: Counters,
    pub unattributed_counters: Counters,
    pub known_lower_bounds: Counters,
    pub total_tokens: Option<u64>,
    pub known_total_lower_bound: u64,
    pub assurance: &'static str,
    pub coverage: &'static str,
    pub contributing_event_ids: Vec<Uuid>,
    pub noncontributing_event_ids: Vec<Uuid>,
    pub declared_quality_counts: BTreeMap<String, usize>,
    pub normalized_quality_counts: BTreeMap<String, usize>,
    pub counter_quality_counts: BTreeMap<String, usize>,
}
fn unique(observations: &[Observation]) -> Result<Vec<&Observation>> {
    let mut selected = Vec::new();
    let mut keys: BTreeMap<String, &Observation> = BTreeMap::new();
    let mut ids = BTreeMap::new();
    for event in observations {
        event.normalize()?;
        if let Some(prior) = ids.insert(event.event_id, event) {
            if prior != event {
                return Err(conflict());
            }
            continue;
        }
        if let Some(prior) = keys.get(&event.source_key()) {
            let mut expected = (*prior).clone();
            expected.event_id = event.event_id;
            expected.observed_at.clone_from(&event.observed_at);
            if &expected != event {
                return Err(conflict());
            }
            continue;
        }
        keys.insert(event.source_key(), event);
        selected.push(event);
    }
    Ok(selected)
}
fn namespace(event: &Observation) -> String {
    format!(
        "{}\0{}",
        event.source.namespace,
        event.host_session_id.as_deref().unwrap_or("")
    )
}
fn owner(event: &Observation) -> (Uuid, Uuid, &str, &str) {
    (
        event.run_id,
        event.segment_id,
        &event.source.adapter,
        &event.source.adapter_version,
    )
}
fn validate_owners(events: &[&Observation]) -> Result<()> {
    let mut requests = BTreeMap::new();
    let mut sessions = BTreeMap::new();
    let mut cumulative = BTreeMap::new();
    for event in events {
        if event.contribution == Contribution::Authoritative
            && sessions
                .insert(event.run_id, event.host_session_id.as_deref())
                .is_some_and(|previous| previous != event.host_session_id.as_deref())
        {
            return Err(coverage_error());
        }
        if let Some(request) = &event.source.request_id
            && requests
                .insert((namespace(event), request), owner(event))
                .is_some_and(|old| old != owner(event))
        {
            return Err(conflict());
        }
        if event.source.scope == Scope::SessionCumulative
            && cumulative
                .insert(namespace(event), owner(event))
                .is_some_and(|old| old != owner(event))
        {
            return Err(conflict());
        }
    }
    for event in events {
        if cumulative
            .get(&namespace(event))
            .is_some_and(|old| *old != owner(event))
        {
            return Err(coverage_error());
        }
    }
    Ok(())
}
fn validate_group(group: &[&Observation]) -> Result<()> {
    let mut sequences = BTreeSet::new();
    for event in group {
        if !sequences.insert(event.sequence) {
            return Err(conflict());
        }
    }
    Ok(())
}
// Validate the whole segment before partitioning its independently namespaced sources.
fn validate_identities(events: &[&Observation]) -> Result<()> {
    let mut identities: BTreeMap<(Uuid, Uuid), [Option<&str>; 5]> = BTreeMap::new();
    for event in events {
        let known = identities
            .entry((event.run_id, event.segment_id))
            .or_default();
        let identity = &event.identity;
        for (prior, observed) in known.iter_mut().zip([
            identity.effective_provider.as_deref(),
            identity.effective_model.as_deref(),
            identity.model_vendor.as_deref(),
            identity.service_tier.as_deref(),
            identity.provider_region.as_deref(),
        ]) {
            if let Some(value) = observed {
                if prior.is_some_and(|old| old != value) {
                    return Err(conflict());
                }
                *prior = Some(value);
            }
        }
    }
    Ok(())
}
// Each snapshot replaces the previous covered interval, so compare against the
// previous snapshot floor PLUS deltas since it, rather than their separate maxima.
fn cumulative_floor(group: &[&Observation]) -> Result<[u64; 5]> {
    let mut floor = [0; 5];
    let mut interval = Vec::new();
    let mut covered_requests = BTreeSet::new();
    for event in group {
        if event.source.scope != Scope::SessionCumulative {
            if event.source.scope == Scope::TurnTotal
                && covered_requests.contains(&event.source.request_id)
            {
                return Err(coverage_error());
            }
            interval.push(*event);
            continue;
        }
        if event.source.request_id.is_some() {
            return Err(coverage_error());
        }
        let lower = sum(&request_contributions(&interval)?)?.1.values();
        for (index, value) in event.counters.values().into_iter().enumerate() {
            let minimum = add(floor[index], lower[index].unwrap_or(0))?;
            if value.is_some_and(|actual| actual < minimum) {
                return Err(coverage_error());
            }
            floor[index] = value.unwrap_or(minimum);
        }
        covered_requests.extend(interval.iter().map(|event| event.source.request_id.clone()));
        interval.clear();
    }
    Ok(floor)
}
fn select_group<'a>(group: &mut [&'a Observation]) -> Result<(Vec<&'a Observation>, [u64; 5])> {
    group.sort_by_key(|event| event.sequence);
    validate_group(group)?;
    let floor = cumulative_floor(group)?;
    let mut active = Vec::new();
    let mut history = [0; 5];
    let cumulative: Vec<_> = group
        .iter()
        .copied()
        .filter(|e| e.source.scope == Scope::SessionCumulative)
        .collect();
    let base = cumulative.last().copied();
    if let Some(base) = base {
        if base.source.request_id.is_some() {
            return Err(coverage_error());
        }
        for (index, value) in base.counters.values().into_iter().enumerate() {
            if value.is_none() {
                history[index] = floor[index];
            }
        }
        active.push(base);
    }
    let later: Vec<_> = group
        .iter()
        .copied()
        .filter(|e| {
            e.source.scope != Scope::SessionCumulative
                && base.is_none_or(|b| e.sequence > b.sequence)
        })
        .collect();
    if let Some(base) = base {
        for event in &later {
            if event.source.scope == Scope::TurnTotal
                && group.iter().any(|old| {
                    old.sequence <= base.sequence
                        && old.source.request_id == event.source.request_id
                        && old.source.scope != Scope::SessionCumulative
                })
            {
                return Err(coverage_error());
            }
        }
    }
    active.extend(request_contributions(&later)?);
    Ok((active, history))
}
fn quality_name(quality: Quality) -> &'static str {
    match quality {
        Quality::Complete => "complete",
        Quality::Partial => "partial",
        Quality::Imported => "imported",
        Quality::Missing => "missing",
    }
}
pub(super) fn summarize(observations: &[Observation]) -> Result<Summary> {
    let unique = unique(observations)?;
    validate_owners(&unique)?;
    validate_identities(&unique)?;
    let mut groups: BTreeMap<String, Vec<&Observation>> = BTreeMap::new();
    for event in &unique {
        if event.contribution == Contribution::Authoritative {
            groups.entry(event.group()).or_default().push(event);
        }
    }
    let mut active = Vec::new();
    let mut history = [0; 5];
    for group in groups.values_mut() {
        let (selected, lower) = select_group(group)?;
        active.extend(selected);
        for (i, value) in lower.into_iter().enumerate() {
            history[i] = add(history[i], value)?;
        }
    }
    let (counters, lower) = sum(&active)?;
    let mut values = lower.values();
    for (index, extra) in history.into_iter().enumerate() {
        if extra > 0 {
            values[index] = Some(add(values[index].unwrap_or(0), extra)?);
        }
    }
    let lower = Counters::from_values(values);
    let lower_total = add(
        lower.input_total.unwrap_or(0),
        lower.output_total.unwrap_or(0),
    )?;
    let contributing_event_ids: Vec<_> = active.iter().map(|e| e.event_id).collect();
    let noncontributing_event_ids = unique
        .iter()
        .filter(|e| !contributing_event_ids.contains(&e.event_id))
        .map(|e| e.event_id)
        .collect();
    let mut declared_quality_counts = BTreeMap::new();
    let mut normalized_quality_counts = BTreeMap::new();
    let mut counter_quality_counts = BTreeMap::new();
    for event in &unique {
        *declared_quality_counts
            .entry(quality_name(event.quality).to_owned())
            .or_default() += 1;
        *normalized_quality_counts
            .entry(quality_name(event.normalize()?.quality).to_owned())
            .or_default() += 1;
        *counter_quality_counts
            .entry(quality_name(event.normalize()?.counter_quality).to_owned())
            .or_default() += 1;
    }
    let partition = |kind| {
        sum(&active
            .iter()
            .copied()
            .filter(|event| event.attribution == kind)
            .collect::<Vec<_>>())
        .map(|pair| pair.0)
    };
    Ok(Summary {
        exclusive_counters: partition(Attribution::Exclusive)?,
        shared_counters: partition(Attribution::Shared)?,
        unattributed_counters: partition(Attribution::Unattributed)?,
        observation_count: unique.len(),
        contribution_count: active.len(),
        counters,
        known_lower_bounds: lower,
        total_tokens: counters.total()?,
        known_total_lower_bound: lower_total,
        assurance: "local_unverified",
        coverage: "unclosed",
        contributing_event_ids,
        noncontributing_event_ids,
        declared_quality_counts,
        normalized_quality_counts,
        counter_quality_counts,
    })
}
fn request_contributions<'a>(events: &[&'a Observation]) -> Result<Vec<&'a Observation>> {
    let mut requests: BTreeMap<&str, Vec<&Observation>> = BTreeMap::new();
    for event in events {
        let id = event
            .source
            .request_id
            .as_deref()
            .ok_or_else(coverage_error)?;
        requests.entry(id).or_default().push(event);
    }
    let mut selected = Vec::new();
    for group in requests.values() {
        let summaries: Vec<_> = group
            .iter()
            .copied()
            .filter(|e| e.source.scope == Scope::TurnTotal)
            .collect();
        let deltas: Vec<_> = group
            .iter()
            .copied()
            .filter(|e| e.source.scope == Scope::RequestDelta)
            .collect();
        if summaries.windows(2).any(|pair| {
            pair[0].counters != pair[1].counters
                || pair[0].source.semantics != pair[1].source.semantics
        }) {
            return Err(conflict());
        }
        if let Some(summary) = summaries.first() {
            if deltas.iter().any(|d| d.sequence >= summary.sequence) {
                return Err(coverage_error());
            }
            let (_, lower) = sum(&deltas)?;
            for (value, seen) in summary.counters.values().into_iter().zip(lower.values()) {
                if matches!((value,seen),(Some(a),Some(b)) if a<b)
                    || (value.is_none() && seen.is_some_and(|known| known > 0))
                {
                    // A terminal replacement cannot erase a known delta bound.
                    // Reject this ambiguous replacement rather than guessing a total.
                    return Err(coverage_error());
                }
            }
            selected.push(*summary);
        } else {
            selected.extend(deltas);
        }
    }
    Ok(selected)
}
fn sum(events: &[&Observation]) -> Result<(Counters, Counters)> {
    if events.is_empty() {
        return Ok((Counters::default(), Counters::default()));
    }
    let mut exact = [Some(0); 5];
    let mut lower = [Some(0); 5];
    for event in events {
        for (i, value) in event.counters.values().into_iter().enumerate() {
            match value {
                Some(value) => {
                    lower[i] = Some(add(lower[i].unwrap_or(0), value)?);
                    exact[i] = exact[i].map(|old| add(old, value)).transpose()?;
                }
                None => exact[i] = None,
            }
        }
    }
    let lower = Counters::from_values(lower);
    // Check the minimum possible total even when a parent counter is unavailable.
    add(
        lower.input_total.unwrap_or(0).max(add(
            lower.cached_input.unwrap_or(0),
            lower.cache_creation_input.unwrap_or(0),
        )?),
        lower
            .output_total
            .unwrap_or(0)
            .max(lower.reasoning_output.unwrap_or(0)),
    )?;
    Ok((Counters::from_values(exact), lower))
}

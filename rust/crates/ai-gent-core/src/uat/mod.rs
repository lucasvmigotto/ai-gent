//! `ai-gent uat`: the deterministic half of live acceptance testing (`qa:uat`, spec 055).
//!
//! The agent writes and judges; this module checks the roadmap source (`lint`) and compares two
//! versions of it (`diff`). It reads files and writes nothing.
pub mod diff;
pub mod lint;
pub mod model;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The feature directory for `NNN` or a path, under `root/specs`.
///
/// # Errors
/// When nothing, or more than one directory, fits.
pub fn feature_dir(root: &Path, feature: &str) -> Result<PathBuf, String> {
    let direct = root.join(feature);
    if feature.contains('-') && direct.is_dir() {
        return Ok(direct);
    }
    let mut hits: Vec<PathBuf> = std::fs::read_dir(root.join("specs"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{feature}-")))
        })
        .collect();
    hits.sort();
    if hits.len() == 1 {
        Ok(hits.remove(0))
    } else {
        Err(format!("no single feature `{feature}` under specs/"))
    }
}

/// The acceptance scenarios of a feature's `spec.md` as anchors (`specs/NNN-x/spec.md#US1-AS2`).
///
/// A scenario is a numbered item under a story's `**Acceptance Scenarios**` line; stories are read
/// with the Spec Kit reader the pipeline already uses.
#[must_use]
pub fn scenarios(spec_text: &str, dir_name: &str) -> Vec<String> {
    let (_, stories) = crate::spec::parse::parse_spec(Some(spec_text));
    let mut out = Vec::new();
    for story in stories {
        let mut inside = false;
        let mut count = 0;
        for line in story.text.lines() {
            let t = line.trim();
            if t.starts_with("**Acceptance Scenarios") {
                inside = true;
                continue;
            }
            if inside && t.starts_with("**") && !t.starts_with("**Given") {
                inside = false;
            }
            let numbered = t
                .split_once(". ")
                .is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
            if inside && numbered {
                count += 1;
                out.push(format!("specs/{dir_name}/spec.md#US{}-AS{count}", story.n));
            }
        }
    }
    out
}

/// Read each mode B feature's scenarios into a lint context.
///
/// # Errors
/// Never; a feature that cannot be read is left out and `lint` reports it.
#[must_use]
pub fn context_for(r: &model::Roadmap, root: &Path, budget: Option<u64>) -> lint::Context {
    let mut scenarios_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if r.mode == "B" {
        for f in &r.features {
            let read = feature_dir(root, f).and_then(|d| {
                let name = d
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().to_string());
                std::fs::read_to_string(d.join("spec.md"))
                    .map(|t| scenarios(&t, &name))
                    .map_err(|e| e.to_string())
            });
            if let Ok(list) = read {
                scenarios_by.insert(f.clone(), list);
            }
        }
    }
    lint::Context {
        scenarios: scenarios_by,
        budget,
    }
}

#[cfg(test)]
mod tests;

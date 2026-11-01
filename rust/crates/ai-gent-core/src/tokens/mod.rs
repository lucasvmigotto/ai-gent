//! Token tracking: a port of `plugins/agents/scripts/token_report.py` (spec 035).
//!
//! Parity with the Python tool is the contract: same outputs, byte for byte, until the 3.0.0
//! cutover (decision D7). Pure helpers live in `util`, JSON that prints like Python's in `pyjson`.
// A line-for-line port of a Python tool whose arithmetic (counts as i64, floats for weights) and
// short names are part of the parity contract; these pedantic lints would only obscure the mapping.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::manual_assert,
    clippy::unreadable_literal,
    clippy::float_cmp,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::assigning_clones,
    clippy::manual_let_else,
    clippy::doc_markdown,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::implicit_hasher,
    clippy::missing_panics_doc,
    clippy::type_complexity
)]

pub mod attribute;
pub mod budget;
pub mod estimate;
pub mod history;
pub mod pyjson;
pub mod report;
pub mod specs;
pub mod tags;
pub mod transcripts;
pub mod util;
pub mod writers;

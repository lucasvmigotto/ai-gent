//! Database runner core of ai-gent (spec 038): the driver-free safety core and the connectors.
//!
//! - [`core`]: statement splitting and classification, cost rules, masking, redaction. No driver.
//! - [`connector`]: the [`connector::Connector`] trait (the runner's `_exec` JSON contract), the
//!   contract cases, and one module per engine behind its Cargo feature (`sqlite` today).
//!
//! Built for ai-gent's own policy, not a general SQL validator: see the README.
#![forbid(unsafe_code)]
// The classifier is a port of a rule-heavy Python tool: one long function per rule family, short names
// for scanner positions, a local helper next to the loop that uses it, and `unwrap` only on values the
// preceding line guarantees (a regex literal, a split that always yields one item).
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::missing_panics_doc
)]

pub mod connector;
pub mod core;

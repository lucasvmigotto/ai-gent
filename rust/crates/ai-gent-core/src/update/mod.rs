//! Update notice, refresh and apply (spec 034): `update --check` is notify-only; `update` runs the
//! target release's own installer.
pub mod apply;
pub mod cache;
pub mod check;
pub mod source;
pub mod version;

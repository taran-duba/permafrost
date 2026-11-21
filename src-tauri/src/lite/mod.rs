//! Permafrost Lite: CacheCAD's upload/download workflow on Permafrost's sync
//! core. Nothing is locked or checked out. The app lists what's ready to
//! upload and what's ready to download (`plan`), and the user ticks rows and
//! applies them (`download`, `upload`).
//!
//! This module is always compiled (so its tests run in every build); only the
//! app wiring is behind the `lite` Cargo feature.

pub mod download;
pub mod plan;
pub mod upload;

#[cfg(test)]
mod tests;

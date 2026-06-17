//! Effect re-exports and helpers.
//!
//! The `Effect` enum lives in `git-vacuum-core::event` (because it crosses
//! the service/app/binary boundary). This module just re-exports it and
//! provides construction helpers for clarity at call sites.

pub use git_vacuum_core::Effect;

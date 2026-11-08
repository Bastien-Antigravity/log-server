//! ESSENTIAL PROCESS:
//! Utilities module namespace for logging formatters, helpers, and terminal UI.
//!
//! DATA FLOW:
//! 1. Exposes utility helpers and terminal formatters to log-server crate.
//!
//! KEY PARAMETERS:
//! - helpers: OS and string manipulation routines.
//! - terminal_ui: Terminal output and level color palettes.

pub mod helpers;
pub mod terminal_ui;

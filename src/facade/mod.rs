//! ESSENTIAL PROCESS:
//! Facade module namespace for high-level log server orchestration and
//! file writer components.
//!
//! DATA FLOW:
//! 1. Exposes LogServer and LogWriter facades to the crate root.
//!
//! KEY PARAMETERS:
//! - log_server: System orchestrator.
//! - log_writer: Ordered async disk persistence.

pub mod log_server;
pub mod log_writer;

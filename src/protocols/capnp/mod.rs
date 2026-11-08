//! ESSENTIAL PROCESS:
//! Cap'n Proto messaging schema module namespace.
//!
//! DATA FLOW:
//! 1. Exposes generated LoggerMsg serialization structures.
//!
//! KEY PARAMETERS:
//! - logger_msg: Generated Cap'n Proto Rust bindings for logger.capnp.

#[allow(clippy::all)]
#[rustfmt::skip]
pub mod logger_msg;

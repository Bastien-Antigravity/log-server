//! ESSENTIAL PROCESS:
//! Core server components and business logic module namespace.
//!
//! DATA FLOW:
//! 1. Aggregates log formatters, protocol decoders, ring buffer, and reordering tests.
//!
//! KEY PARAMETERS:
//! - log_formatter, protocol_handlers, reorder_test, ring_buffer.

pub mod log_formatter;
pub mod protocol_handlers;
pub mod reorder_test;
pub mod ring_buffer;
